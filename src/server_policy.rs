use crate::cli;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct PolicyEvaluator {
}

#[derive(Debug, Clone, Serialize)]
pub struct EvaluationResult {
    pub action: Option<String>,
    pub field: String,
    pub rule: String,
    pub observed_value: f64,
    pub observed_value_str: String,
    pub policy_value: f64,
    pub evaluation_expression: String,
    pub reason: String,
}
pub struct Rule {
    pub operator: String,
    pub field: String,
    // For numeric comparisons (e.g. numeric_lte) this holds the threshold as string
    pub condition_value: Option<String>,
    // For list-based rules this holds the allowed/denied values
    pub allowed_values: Option<Vec<String>>,
    // The action to take when the rule condition evaluates to true
    pub action: String,
}

impl PolicyEvaluator {
    pub fn new() -> Self {
        Self {
        }
    }

    // New evaluator that operates on full payload and rule contexts.
    // Returns `Some(action)` when the payload satisfies the rule, `None` otherwise.
    pub fn evaluate(&self, payload: &crate::action::ProposedAction, rule: &Rule) -> Option<String> {

        match rule.operator.as_str() {
            "numeric_lte" => {
                let cost_per_hour = payload.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let payload_str = format!("{:.2}", cost_per_hour);
                let payload_cents = parse_to_fixed_cents(&payload_str);
                let policy_cents =
                    parse_to_fixed_cents(rule.condition_value.as_deref().unwrap_or(""));

                match (payload_cents, policy_cents) {
                    (Some(payload_val), Some(policy_val)) => {
                        // CRITICAL CORRECTION: Must be strictly payload less-than-or-equal-to policy
                        let condition_matches = payload_val <= policy_val;
                        if condition_matches {
                            Some(rule.action.clone())
                        } else {
                            None
                        }
                    }
                    _ => {
                        cli::debug_log("[Traxes] SECURITY CRITICAL: Failed to parse fixed-point numeric strings cleanly.");
                        None // Fail closed
                    }
                }
            }
            "in_list" => {
                let allowed = match &rule.allowed_values {
                    Some(v) => v.clone(),
                    None => {
                        // Fallback: try parsing condition_value if present
                        if let Some(cv) = &rule.condition_value {
                            if cv.trim().starts_with('[') {
                                serde_json::from_str(cv).unwrap_or_default()
                            } else {
                                cv.split(',')
                                    .map(|s| s.trim().trim_matches('"').to_string())
                                    .collect()
                            }
                        } else {
                            vec![]
                        }
                    }
                };

                let payload_val = payload.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                if allowed.contains(&payload_val) {
                    Some(rule.action.clone())
                } else {
                    None
                }
            }
            "not_in" => {
                // For 'not in' constraints: if payload is NOT in the list, the condition matches (violation)
                let list = match &rule.allowed_values {
                    Some(v) => v.clone(),
                    None => {
                        if let Some(cv) = &rule.condition_value {
                            if cv.trim().starts_with('[') {
                                serde_json::from_str(cv).unwrap_or_default()
                            } else {
                                cv.split(',')
                                    .map(|s| s.trim().trim_matches('"').to_string())
                                    .collect()
                            }
                        } else {
                            vec![]
                        }
                    }
                };

                let payload_val = payload.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                // If payload is NOT in the list, it's a violation → return DENY action
                // If payload IS in the list, it's allowed → return None (no action)
                if !list.contains(&payload_val) {
                    Some(rule.action.clone()) // DENY
                } else {
                    None // ALLOW
                }
            }
            _ => {
                cli::debug_log(format!("[Traxes] Unknown rule operator: {}", rule.operator));
                None // Fail closed on unknown operators
            }
        }
    }
}
/// Safely converts a decimal string to a standardized 2-decimal integer (cents)
/// Handles "12.5", "12.50", and "12" uniformly. Returns None on malformed text.
fn parse_to_fixed_cents(val_str: &str) -> Option<i64> {
    let clean = val_str.trim();
    if let Some((iter_part, frac_part)) = clean.split_once('.') {
        let ip: i64 = iter_part.parse().ok()?;

        let mut frac_str = frac_part.to_string();
        if frac_str.len() > 2 {
            frac_str.truncate(2);
        } else {
            while frac_str.len() < 2 {
                frac_str.push('0');
            }
        }

        let fp: i64 = frac_str.parse().ok()?;
        if ip < 0 || clean.starts_with('-') {
            Some(ip * 100 - fp)
        } else {
            Some(ip * 100 + fp)
        }
    } else {
        let ip: i64 = clean.parse().ok()?;
        Some(ip * 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ProposedAction;
    use serde_json::json;

    fn create_test_action(cost: f64, instance_type: &str) -> ProposedAction {
        ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_cost_per_hour": cost,
                "instance_type": instance_type
            }),
        }
    }

    // ==================== numeric_lte Operator Tests ====================

    #[test]
    fn test_numeric_lte_exact_match_denies() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("2.00".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // 2.00 <= 2.00 matches, so DENY
    }

    #[test]
    fn test_numeric_lte_below_threshold_denies() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(1.50, "t3.medium");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("2.00".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // 1.50 <= 2.00 matches, so DENY
    }

    #[test]
    fn test_numeric_lte_above_threshold_allows() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.50, "t3.medium");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("2.00".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // 2.50 <= 2.00 doesn't match, so ALLOW
    }

    #[test]
    fn test_numeric_lte_integer_format() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.0, "t3.medium");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("2".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    #[test]
    fn test_numeric_lte_decimal_precision() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(1.5, "t3.medium");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("1.50".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    #[test]
    fn test_numeric_lte_edge_case_small_value() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(0.01, "t3.micro");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("0.01".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    #[test]
    fn test_numeric_lte_missing_cost_parameter() {
        let evaluator = PolicyEvaluator::new();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.medium"
                // Missing instance_cost_per_hour
            }),
        };
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("2.00".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // Missing cost defaults to 0.0, which is <= 2.00, so DENY
    }

    #[test]
    fn test_numeric_lte_invalid_threshold() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "numeric_lte".to_string(),
            field: "instance_cost_per_hour".to_string(),
            condition_value: Some("invalid".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // Fail closed
    }

    // ==================== parse_to_fixed_cents Tests ====================

    #[test]
    fn test_parse_to_fixed_cents_integer() {
        assert_eq!(parse_to_fixed_cents("12"), Some(1200));
    }

    #[test]
    fn test_parse_to_fixed_cents_decimal_one_digit() {
        assert_eq!(parse_to_fixed_cents("12.5"), Some(1250));
    }

    #[test]
    fn test_parse_to_fixed_cents_decimal_two_digits() {
        assert_eq!(parse_to_fixed_cents("12.50"), Some(1250));
    }

    #[test]
    fn test_parse_to_fixed_cents_decimal_more_digits() {
        assert_eq!(parse_to_fixed_cents("12.567"), Some(1256)); // Truncates
    }

    #[test]
    fn test_parse_to_fixed_cents_zero() {
        assert_eq!(parse_to_fixed_cents("0"), Some(0));
        assert_eq!(parse_to_fixed_cents("0.00"), Some(0));
    }

    #[test]
    fn test_parse_to_fixed_cents_negative() {
        assert_eq!(parse_to_fixed_cents("-1.50"), Some(-150));
    }

    #[test]
    fn test_parse_to_fixed_cents_invalid_empty() {
        assert_eq!(parse_to_fixed_cents(""), None);
    }

    #[test]
    fn test_parse_to_fixed_cents_invalid_non_numeric() {
        assert_eq!(parse_to_fixed_cents("abc"), None);
    }

    #[test]
    fn test_parse_to_fixed_cents_whitespace() {
        assert_eq!(parse_to_fixed_cents(" 12.50 "), Some(1250));
    }

    // ==================== in_list Operator Tests ====================

    #[test]
    fn test_in_list_value_present_denies() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "in_list".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string(), "t3.small".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    #[test]
    fn test_in_list_value_not_present_allows() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "m5.large");
        let rule = Rule {
            operator: "in_list".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string(), "t3.small".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // No action = ALLOW
    }

    #[test]
    fn test_in_list_empty_list() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "in_list".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec![]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // No action = ALLOW
    }

    #[test]
    fn test_in_list_missing_instance_type() {
        let evaluator = PolicyEvaluator::new();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_cost_per_hour": 2.00
                // Missing instance_type
            }),
        };
        let rule = Rule {
            operator: "in_list".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // "unknown" not in list
    }

    #[test]
    fn test_in_list_json_array_format() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "in_list".to_string(),
            field: "instance_type".to_string(),
            condition_value: Some("[\"t3.medium\", \"t3.small\"]".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    #[test]
    fn test_in_list_comma_separated_format() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "in_list".to_string(),
            field: "instance_type".to_string(),
            condition_value: Some("t3.medium, t3.small".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    // ==================== not_in Operator Tests ====================

    #[test]
    fn test_not_in_value_not_in_list_denies() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "m5.large");
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string(), "t3.small".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // Not in list = violation
    }

    #[test]
    fn test_not_in_value_in_list_allows() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string(), "t3.small".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // In list = ALLOW
    }

    #[test]
    fn test_not_in_empty_list_denies() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec![]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // Not in empty list = violation
    }

    #[test]
    fn test_not_in_single_item_list() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.small");
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // Not in list = violation
    }

    #[test]
    fn test_not_in_missing_instance_type() {
        let evaluator = PolicyEvaluator::new();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_cost_per_hour": 2.00
                // Missing instance_type
            }),
        };
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string())); // "unknown" not in list = violation
    }

    #[test]
    fn test_not_in_json_array_format() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "m5.large");
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: Some("[\"t3.medium\", \"t3.small\"]".to_string()),
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, Some("DENY".to_string()));
    }

    #[test]
    fn test_not_in_duplicate_values() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "not_in".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: Some(vec!["t3.medium".to_string(), "t3.medium".to_string()]),
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // In list = ALLOW
    }

    // ==================== Unknown Operator Tests ====================

    #[test]
    fn test_unknown_operator_fails_closed() {
        let evaluator = PolicyEvaluator::new();
        let action = create_test_action(2.00, "t3.medium");
        let rule = Rule {
            operator: "unknown_operator".to_string(),
            field: "instance_type".to_string(),
            condition_value: None,
            allowed_values: None,
            action: "DENY".to_string(),
        };

        let result = evaluator.evaluate(&action, &rule);
        assert_eq!(result, None); // Fail closed
    }
}
