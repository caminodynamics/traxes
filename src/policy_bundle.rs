//! Shared policy YAML loading and rule-detection logic for server + CLI evaluate paths.

use crate::action::ProposedAction;
use crate::cli;
use crate::server_policy::{EvaluationResult, PolicyEvaluator, Rule};
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
    let evaluator = PolicyEvaluator::new();
    let threshold_str = find_numeric_lte_threshold(policy_yaml);

    if let Some(th) = threshold_str {
        return evaluate_numeric_lte(action, &evaluator, &th);
    }

    if let Some((op, values, field)) = find_list_rule_from_yaml(policy_yaml) {
        return evaluate_list_rule(action, &evaluator, &op, &values, &field, "instance_type_constraint");
    }

    if let Some(result) = evaluate_deny_expensive_instances_fallback(action, &evaluator, policy_yaml) {
        return result;
    }

    cli::debug_log("[Traxes] No numeric_lte threshold found in policy; failing closed.");
    missing_threshold_result(action)
}

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
    let cost_per_hour = action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
    EvaluationResult {
        action: action_result,
        field: rule.field.clone(),
        rule: rule.operator.clone(),
        observed_value: cost_per_hour,
        observed_value_str: cost_per_hour.to_string(),
        policy_value: threshold.parse::<f64>().unwrap_or(0.0),
        evaluation_expression: cli::compact_evaluation_expression(&rule.field, &rule.operator),
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
    // Fix: Extract the actual field value being evaluated (instance_type), not cost_per_hour
    let instance_type = action.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let cost_per_hour = action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
    
    // Use appropriate value based on field type - CRITICAL FIX
    let (observed_value, observed_value_str) = if field == "instance_type" || field.contains("instance_type") {
        // For instance_type field, use a hash of the string value for observed_value (f64)
        // This prevents cross-field contamination while maintaining type compatibility
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        instance_type.hash(&mut hasher);
        let hash = hasher.finish();
        (hash as f64, instance_type.to_string())
    } else {
        (cost_per_hour, cost_per_hour.to_string())
    };
    
    EvaluationResult {
        action: action_result,
        field: rule.field.clone(),
        rule: rule.operator.clone(),
        observed_value,
        observed_value_str,
        policy_value: 0.0,
        evaluation_expression: cli::compact_evaluation_expression(field, op),
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
        cli::debug_log(
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
        cli::debug_log(
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
                cli::debug_log("[Traxes] No usable allowlist found; failing closed.");
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
            cli::debug_log("[Traxes] No usable allowlist found in raw slice; failing closed.");
            Some(missing_threshold_result(action))
        }
    }
}

fn missing_threshold_result(action: &ProposedAction) -> EvaluationResult {
    let cost_per_hour = action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
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
            cli::debug_log(format!("[Traxes] Failed to parse policy YAML: {}", e));
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
    use std::path::Path;

    #[test]
    fn loads_policy_from_repo_relative_path() {
        let path = "../demo/policies/default_policy.yaml";
        if !Path::new(path).exists() {
            return;
        }
        let yaml = read_policy_yaml(path).expect("policy readable");
        assert!(!yaml.is_empty());
    }
}
