//! Shared policy YAML loading and rule-detection logic for server + CLI evaluate paths.

use crate::action::ProposedAction;
use crate::cli_utils;
use crate::server_policy::{EvaluationResult, PolicyEvaluator, Rule};
use crate::traxes_engine::ParsedRule;
use serde_yaml::Value as YamlValue;
use std::io;

pub fn read_policy_yaml(path: &str) -> io::Result<String> {
    let policy_content = std::fs::read_to_string(path)?;
    Ok(normalize_policy_encoding(&policy_content, path))
}

pub fn normalize_policy_encoding(policy_content: &str, path: &str) -> String {
    if policy_content.contains('\u{0}') {
        if let Ok(bytes) = std::fs::read(path) {
            let u16_iter: Vec<u16> = bytes
                .chunks(2)
                .filter_map(|c| {
                    if c.len() == 2 {
                        Some(u16::from_le_bytes([c[0], c[1]]))
                    } else {
                        None
                    }
                })
                .collect();
            return String::from_utf16(&u16_iter).unwrap_or_else(|_| policy_content.to_string());
        }
    }
    policy_content.to_string()
}

pub fn evaluate_action_policy(action: &ProposedAction, policy_yaml: &str) -> EvaluationResult {
    let target_tool = parse_policy_target_tool(policy_yaml);
    if let Some(denial) = validate_policy_target(action, target_tool.as_deref()) {
        return denial;
    }
    let evaluator = PolicyEvaluator::new();
    let threshold_str = find_numeric_lte_threshold(policy_yaml);

    if let Some(th) = threshold_str {
        return evaluate_numeric_lte(action, &evaluator, &th);
    }

    if let Some((op, values, field)) = find_list_rule_from_yaml(policy_yaml) {
        return evaluate_list_rule(
            action,
            &evaluator,
            &op,
            &values,
            &field,
            "instance_type_constraint",
        );
    }

    if let Some(result) =
        evaluate_deny_expensive_instances_fallback(action, &evaluator, policy_yaml)
    {
        return result;
    }

    cli_utils::debug_log("[Traxes] No numeric_lte threshold found in policy; failing closed.");
    missing_threshold_result(action)
}

/// Missing, malformed, or empty targets cannot authorize any tool.
pub(crate) fn parse_policy_target_tool(policy_yaml: &str) -> Option<String> {
    let doc: YamlValue = serde_yaml::from_str(policy_yaml).ok()?;
    let tool = doc.get("target")?.get("tool")?.as_str()?;
    if tool.trim().is_empty() {
        return None;
    }
    Some(tool.to_string())
}

fn validate_policy_target(
    action: &ProposedAction,
    target_tool: Option<&str>,
) -> Option<EvaluationResult> {
    if target_tool.is_some_and(|tool| !tool.trim().is_empty() && tool == action.tool) {
        return None;
    }
    Some(EvaluationResult {
        action: Some("DENY".to_string()),
        field: "proposed_action.tool".to_string(),
        rule: "target_tool_match".to_string(),
        observed_value: 0.0,
        observed_value_str: action.tool.clone(),
        policy_value: 0.0,
        evaluation_expression: "proposed_action.tool == policy.target.tool".to_string(),
        reason: "POLICY_TARGET_MISSING_OR_MISMATCH".to_string(),
    })
}

/// Parse policy rules from YAML during engine initialization
pub fn parse_policy_rules(policy_yaml: &str) -> Vec<ParsedRule> {
    let mut rules = Vec::new();

    // Try to find list rules from YAML
    if let Some((op, values, field)) = find_list_rule_from_yaml(policy_yaml) {
        rules.push(ParsedRule {
            operator: op,
            field,
            allowed_values: values,
            rule_id: "instance_type_constraint".to_string(),
        });
    }

    // Try to find numeric threshold rules
    if let Some(threshold) = find_numeric_lte_threshold(policy_yaml) {
        // Convert numeric threshold to a rule representation
        // For numeric rules, we store the threshold in allowed_values as a single-element vector
        rules.push(ParsedRule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            allowed_values: vec![threshold],
            rule_id: "numeric_lte".to_string(),
        });
    }

    rules
}

/// Evaluate action using pre-parsed rules and their policy target (hot path).
/// Target matching is mandatory and precedes every rule evaluation.
pub fn evaluate_action_policy_with_rules(
    action: &ProposedAction,
    parsed_rules: &[ParsedRule],
    target_tool: Option<&str>,
) -> EvaluationResult {
    if let Some(denial) = validate_policy_target(action, target_tool) {
        return denial;
    }
    let evaluator = PolicyEvaluator::new();

    // Try numeric_lte rules first
    for rule in parsed_rules {
        if rule.operator == "numeric_lte" {
            if let Some(threshold) = rule.allowed_values.first() {
                return evaluate_numeric_lte(action, &evaluator, threshold);
            }
        }
    }

    // Try list rules (not_in, in_list)
    for rule in parsed_rules {
        if rule.operator == "not_in" || rule.operator == "in_list" {
            return evaluate_list_rule(
                action,
                &evaluator,
                &rule.operator,
                &rule.allowed_values,
                &rule.field,
                &rule.rule_id,
            );
        }
    }

    cli_utils::debug_log("[Traxes] No matching rules found; failing closed.");
    missing_threshold_result(action)
}

// Operator-based classification: list operators are explicit in parsed rules.
// The previous field-name heuristic was brittle and is removed. Use the
// rule operator (in_list / not_in) rather than inspecting field names.

fn evaluate_numeric_lte(
    action: &ProposedAction,
    evaluator: &PolicyEvaluator,
    threshold: &str,
) -> EvaluationResult {
    let rule = Rule {
        operator: "numeric_lte".to_string(),
        field: "instance_cost_per_hour".to_string(),
        condition_value: Some(threshold.to_string()),
        allowed_values: None,
        action: "DENY".to_string(),
    };

    let action_result = evaluator.evaluate(action, &rule);
    let cost_per_hour = action
        .parameters
        .get("instance_cost_per_hour")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    EvaluationResult {
        action: action_result,
        field: rule.field.clone(),
        rule: rule.operator.clone(),
        observed_value: cost_per_hour,
        observed_value_str: cost_per_hour.to_string(),
        policy_value: threshold.parse::<f64>().unwrap_or(0.0),
        evaluation_expression: cli_utils::compact_evaluation_expression(
            &rule.field,
            &rule.operator,
        ),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    }
}

fn evaluate_list_rule(
    action: &ProposedAction,
    evaluator: &PolicyEvaluator,
    op: &str,
    values: &[String],
    field: &str,
    _rule_id: &str,
) -> EvaluationResult {
    let rule = Rule {
        operator: op.to_string(),
        field: field.to_string(),
        condition_value: None,
        allowed_values: Some(values.to_vec()),
        action: "DENY".to_string(),
    };

    let action_result = evaluator.evaluate(action, &rule);
    let param_key = crate::server_policy::parameter_key(field);
    let observed_value_raw = action
        .parameters
        .get(param_key)
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Return raw observed values - hashing moved to artifact generation phase
    // Classify by operator (op) rather than field name. If this is a list
    // operator, return string observed value; otherwise assume numeric (cost).
    let (observed_value, observed_value_str) = if op == "in_list" || op == "not_in" {
        (0.0, observed_value_raw.to_string())
    } else {
        let cost_per_hour = action
            .parameters
            .get("instance_cost_per_hour")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        (cost_per_hour, cost_per_hour.to_string())
    };

    EvaluationResult {
        action: action_result,
        field: rule.field.clone(),
        rule: rule.operator.clone(),
        observed_value,
        observed_value_str,
        policy_value: 0.0,
        evaluation_expression: cli_utils::compact_evaluation_expression(field, op),
        reason: "INSTANCE_TYPE_CONSTRAINT_CHECK".to_string(),
    }
}

fn evaluate_deny_expensive_instances_fallback(
    action: &ProposedAction,
    evaluator: &PolicyEvaluator,
    policy_yaml: &str,
) -> Option<EvaluationResult> {
    let pos = policy_yaml.find("deny_expensive_instances")?;

    let mut start_idx: Option<usize> = None;
    for (i, c) in policy_yaml.char_indices().skip(pos) {
        if c == '[' {
            start_idx = Some(i);
            break;
        }
    }
    let Some(s) = start_idx else {
        cli_utils::debug_log(
            "[Traxes] No opening bracket found after deny_expensive_instances; failing closed.",
        );
        return Some(missing_threshold_result(action));
    };

    let mut depth: i32 = 0;
    let mut end_idx: Option<usize> = None;
    for (i, c) in policy_yaml.char_indices().skip(s) {
        if c == '[' {
            depth += 1;
        } else if c == ']' {
            depth -= 1;
            if depth == 0 {
                end_idx = Some(i);
                break;
            }
        }
    }
    let Some(e) = end_idx else {
        cli_utils::debug_log(
            "[Traxes] No closing bracket found after deny_expensive_instances; failing closed.",
        );
        return Some(missing_threshold_result(action));
    };

    let slice = &policy_yaml[s..=e];
    match serde_yaml::from_str::<YamlValue>(slice) {
        Ok(YamlValue::Sequence(arr)) => {
            let mut v = vec![];
            for el in arr.iter() {
                if let YamlValue::String(s) = el {
                    v.push(s.clone());
                }
            }
            if v.is_empty() {
                cli_utils::debug_log("[Traxes] No usable allowlist found; failing closed.");
                Some(missing_threshold_result(action))
            } else {
                Some(evaluate_list_rule(
                    action,
                    evaluator,
                    "not_in",
                    &v,
                    "instance_type",
                    "deny_expensive_instances",
                ))
            }
        }
        _ => {
            cli_utils::debug_log(
                "[Traxes] No usable allowlist found in raw slice; failing closed.",
            );
            Some(missing_threshold_result(action))
        }
    }
}

fn missing_threshold_result(action: &ProposedAction) -> EvaluationResult {
    let cost_per_hour = action
        .parameters
        .get("instance_cost_per_hour")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    EvaluationResult {
        action: Some("DENY".to_string()),
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: cost_per_hour,
        observed_value_str: cost_per_hour.to_string(),
        policy_value: 0.0,
        evaluation_expression: "NO_THRESHOLD".to_string(),
        reason: "POLICY_MISSING_THRESHOLD".to_string(),
    }
}

fn find_numeric_lte_threshold(policy_yaml: &str) -> Option<String> {
    match serde_yaml::from_str::<YamlValue>(policy_yaml) {
        Ok(doc) => find_threshold(&doc),
        Err(e) => {
            cli_utils::debug_log(format!("[Traxes] Failed to parse policy YAML: {}", e));
            None
        }
    }
}

fn find_threshold(v: &YamlValue) -> Option<String> {
    match v {
        YamlValue::Mapping(map) => {
            if let Some(op) = map.get(&YamlValue::String("operator".to_string())) {
                if op == &YamlValue::String("numeric_lte".to_string()) {
                    for key in &["condition_value", "value", "threshold"] {
                        if let Some(val) = map.get(&YamlValue::String(key.to_string())) {
                            if let YamlValue::String(s) = val {
                                return Some(s.clone());
                            }
                            if let YamlValue::Number(n) = val {
                                return Some(n.to_string());
                            }
                        }
                    }
                }
            }
            for (_k, v) in map.iter() {
                if let Some(found) = find_threshold(v) {
                    return Some(found);
                }
            }
            None
        }
        YamlValue::Sequence(seq) => {
            for item in seq.iter() {
                if let Some(found) = find_threshold(item) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn find_list_rule_from_yaml(policy_yaml: &str) -> Option<(String, Vec<String>, String)> {
    let doc: YamlValue = serde_yaml::from_str(policy_yaml).ok()?;
    let YamlValue::Mapping(map) = doc else {
        return None;
    };
    let rules_val = map.get(&YamlValue::String("rules".to_string()))?;
    let YamlValue::Sequence(seq) = rules_val else {
        return None;
    };

    for item in seq.iter() {
        let YamlValue::Mapping(m) = item else {
            continue;
        };
        let cond_val = m.get(&YamlValue::String("condition".to_string()))?;
        let cond_raw = match cond_val {
            YamlValue::String(s) => s.clone(),
            other => serde_yaml::to_string(other).unwrap_or_default(),
        };
        let cond_clean = cond_raw
            .replace('\n', " ")
            .replace('\r', " ")
            .replace('\t', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        let operator = if cond_clean.contains(" not in ") {
            "not_in"
        } else if cond_clean.contains(" in ") {
            "in_list"
        } else {
            continue;
        };

        let field = if let Some(idx) = cond_clean.find("payload.") {
            let rest = &cond_clean[idx + 8..];
            if let Some(space_idx) = rest.find(' ') {
                rest[..space_idx].to_string()
            } else {
                rest.to_string()
            }
        } else {
            continue;
        };

        let start = cond_clean.find('[')?;
        let end = cond_clean[start..].find(']')?;
        let end_idx = start + end;
        let slice = &cond_clean[start..=end_idx];
        let vals: YamlValue = serde_yaml::from_str(slice).ok()?;
        let YamlValue::Sequence(arr) = vals else {
            continue;
        };
        let mut v = vec![];
        for el in arr.iter() {
            if let YamlValue::String(s) = el {
                v.push(s.trim().to_string());
            }
        }
        if !v.is_empty() {
            return Some((operator.to_string(), v, field));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traxes_engine::Engine;
    use serde_json::json;
    use std::path::Path;

    fn action(tool: &str) -> ProposedAction {
        ProposedAction {
            tool: tool.to_string(),
            session_id: "target-binding".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.small",
                "path": "allowed.txt",
                "content": "must be governed"
            }),
        }
    }

    // Exercise all policy-level evaluation entry points, including the raw path.
    fn assert_decision(policy: &str, action: &ProposedAction, expected: &str) {
        let engine = Engine::with_policy(policy.to_string());
        assert_eq!(engine.evaluate(action).decision, expected);
        let rules = parse_policy_rules(policy);
        let target = parse_policy_target_tool(policy);
        for result in [
            engine.evaluate_raw(action),
            evaluate_action_policy(action, policy),
            evaluate_action_policy_with_rules(action, &rules, target.as_deref()),
        ] {
            assert_eq!(cli_utils::normalize_decision(&result).0, expected);
        }
    }

    #[test]
    fn matching_aws_target_preserves_allow() {
        assert_decision(
            crate::traxes_engine::DEFAULT_POLICY_YAML,
            &action("aws_ec2_provision"),
            "ALLOW",
        );
    }

    #[test]
    fn aws_target_rejects_file_write_with_allowed_aws_parameters() {
        let policy = crate::traxes_engine::DEFAULT_POLICY_YAML;
        let action = action("FILE_WRITE");
        assert_decision(policy, &action, "DENY");
        assert_eq!(
            Engine::with_policy(policy.to_string())
                .evaluate(&action)
                .result
                .reason,
            "POLICY_TARGET_MISSING_OR_MISMATCH"
        );
    }

    #[test]
    fn matching_file_write_target_preserves_path_allowlist() {
        let policy = r#"target:
  tool: FILE_WRITE
rules:
  - name: path_allowlist
    condition: payload.proposed_action.parameters.path not in ["allowed.txt"]
    action: DENY
"#;
        let mut action = action("FILE_WRITE");
        assert_decision(policy, &action, "ALLOW");
        action.parameters["path"] = json!("forbidden.txt");
        assert_decision(policy, &action, "DENY");
        action.parameters["path"] = json!("allowed.txt");
        action.tool = "aws_ec2_provision".to_string();
        assert_decision(policy, &action, "DENY");
    }

    #[test]
    fn missing_invalid_or_mismatched_targets_fail_closed() {
        let rules = r#"rules:
  - name: allowlist
    condition: payload.proposed_action.parameters.instance_type not in ["t3.small"]
    action: DENY
"#;
        for target in [
            "",
            "target: null\n",
            "target: {}\n",
            "target: FILE_WRITE\n",
            "target: {tool: null}\n",
            "target: {tool: 42}\n",
            "target: {tool: []}\n",
            "target: {tool: ''}\n",
            "target: {tool: '   '}\n",
            "target: {tool: unrelated}\n",
        ] {
            let policy = format!("{target}{rules}");
            for tool in ["FILE_WRITE", "aws_ec2_provision"] {
                assert_decision(&policy, &action(tool), "DENY");
            }
        }
        // Tool matching is exact, with no case folding or whitespace aliases.
        for tool in ["", "AWS_EC2_PROVISION", "aws_ec2_provision "] {
            assert_decision(
                crate::traxes_engine::DEFAULT_POLICY_YAML,
                &action(tool),
                "DENY",
            );
        }
        assert_decision("target: [invalid yaml", &action("FILE_WRITE"), "DENY");
    }

    #[test]
    fn tool_mismatch_cannot_create_or_overwrite_file() {
        let sandbox = std::env::temp_dir().join(format!("traxes-target-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&sandbox).unwrap();
        let existing = sandbox.join("existing.txt");
        let missing = sandbox.join("missing.txt");
        std::fs::write(&existing, "untouched").unwrap();
        let engine = Engine::load_default_policies().unwrap();
        for path in [&existing, &missing] {
            let mut action = action("FILE_WRITE");
            action.parameters["path"] = json!(path);
            let decision = engine.evaluate(&action);
            assert_eq!(decision.decision, "DENY");
            // DENY decisions mean no permit is created by the engine
            // Simulate this by passing None to execute
            let execution_outcome = crate::action::execute(&action, None);
            assert_eq!(
                execution_outcome,
                crate::action::ExecutionOutcome::Unauthorized
            );
        }
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "untouched");
        assert!(!missing.exists());
        std::fs::remove_dir_all(&sandbox).unwrap();
    }

    #[test]
    fn loads_policy_from_repo_relative_path() {
        let path = "policies/aws_staging_guardrails.yaml";
        if !Path::new(path).exists() {
            return;
        }
        let yaml = read_policy_yaml(path).expect("policy readable");
        assert!(!yaml.is_empty());
    }
}
