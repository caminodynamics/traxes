use crate::action::ProposedAction;
use crate::artifact::{AuditArtifact, PolicyInfo, GovernanceInfo, EvaluationTrace, EvaluationStep, RuleEvaluationInfo};
use crate::traxes_engine::Engine;
use serde_json::json;

#[cfg(test)]
mod artifact_schema_tests {
    use super::*;

    #[test]
    fn test_policy_info_serialization() {
        let policy_info = PolicyInfo {
            policy_id: "infra-cost-limit-v1".to_string(),
            policy_version: "1.0.0".to_string(),
            policy_hash: "abc123".to_string(),
        };

        let serialized = serde_json::to_string(&policy_info).unwrap();
        let deserialized: PolicyInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(policy_info.policy_id, deserialized.policy_id);
        assert_eq!(policy_info.policy_version, deserialized.policy_version);
        assert_eq!(policy_info.policy_hash, deserialized.policy_hash);
    }

    #[test]
    fn test_governance_info_serialization() {
        let governance_info = GovernanceInfo {
            coverage_status: "GOVERNED".to_string(),
            endpoint: "aws_ec2_provision::staging".to_string(),
            enforcement_hit: true,
            coverage_event_type: Some("EnforcedPath".to_string()),
        };

        let serialized = serde_json::to_string(&governance_info).unwrap();
        let deserialized: GovernanceInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(governance_info.coverage_status, deserialized.coverage_status);
        assert_eq!(governance_info.endpoint, deserialized.endpoint);
        assert_eq!(governance_info.enforcement_hit, deserialized.enforcement_hit);
    }

    #[test]
    fn test_evaluation_trace_serialization() {
        let trace = EvaluationTrace {
            steps: vec![
                EvaluationStep {
                    step_name: "policy_loaded".to_string(),
                    step_order: 0,
                    step_result: "SUCCESS".to_string(),
                    step_duration_us: 100.0,
                    step_details: Some("Policy loaded successfully".to_string()),
                },
                EvaluationStep {
                    step_name: "rule_evaluated".to_string(),
                    step_order: 1,
                    step_result: "SUCCESS".to_string(),
                    step_duration_us: 50.0,
                    step_details: None,
                },
            ],
            total_evaluation_time_us: 150.0,
        };

        let serialized = serde_json::to_string(&trace).unwrap();
        let deserialized: EvaluationTrace = serde_json::from_str(&serialized).unwrap();

        assert_eq!(trace.steps.len(), deserialized.steps.len());
        assert_eq!(trace.total_evaluation_time_us, deserialized.total_evaluation_time_us);
    }

    #[test]
    fn test_rule_evaluation_with_order() {
        let rule_eval = RuleEvaluationInfo {
            rule_id: "test-rule".to_string(),
            field: "instance_cost_per_hour".to_string(),
            observed_value: json!(1.50),
            operator: "numeric_lte".to_string(),
            policy_value: 2.00,
            evaluation_expression: "1.50 <= 2.00".to_string(),
            evaluation_result: true,
            rule_order: Some(0),
        };

        let serialized = serde_json::to_string(&rule_eval).unwrap();
        let deserialized: RuleEvaluationInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(rule_eval.rule_order, deserialized.rule_order);
        assert_eq!(rule_eval.rule_id, deserialized.rule_id);
    }

    #[test]
    fn test_artifact_v2_schema() {
        let engine = Engine::load_default_policies().unwrap();
        
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.medium",
                "instance_cost_per_hour": 1.50
            }),
        };

        let evaluation = engine.evaluate(&action);
        let artifact = crate::artifact::ArtifactLogger::generate_artifact(
            &uuid::Uuid::new_v4().to_string(),
            &action,
            &evaluation,
            engine.policy_hash(),
            "executed".to_string(),
        );

        // Verify artifact version is 2.0.0
        assert_eq!(artifact.artifact_version, "2.0.0");

        // Verify policy_info is present
        assert!(artifact.policy_info.is_some());
        let policy_info = artifact.policy_info.as_ref().unwrap();
        assert_eq!(policy_info.policy_version, "1.0.0");

        // Verify governance_info is present
        assert!(artifact.governance_info.is_some());
        let governance_info = artifact.governance_info.as_ref().unwrap();
        assert!(!governance_info.coverage_status.is_empty());

        // Verify rules_evaluated is present
        assert!(artifact.rules_evaluated.is_some());
        let rules_evaluated = artifact.rules_evaluated.as_ref().unwrap();
        assert_eq!(rules_evaluated.len(), 1);

        // Verify evaluation_trace is None (disabled by default)
        assert!(artifact.evaluation_trace.is_none());

        // Verify rule_order is present
        assert!(artifact.rule_evaluation.rule_order.is_some());
    }

    #[test]
    fn test_artifact_serialization_roundtrip() {
        let engine = Engine::load_default_policies().unwrap();
        
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.micro",
                "instance_cost_per_hour": 0.20
            }),
        };

        let evaluation = engine.evaluate(&action);
        let artifact = crate::artifact::ArtifactLogger::generate_artifact(
            &uuid::Uuid::new_v4().to_string(),
            &action,
            &evaluation,
            engine.policy_hash(),
            "executed".to_string(),
        );

        // Serialize
        let serialized = serde_json::to_string(&artifact).unwrap();
        
        // Deserialize
        let deserialized: AuditArtifact = serde_json::from_str(&serialized).unwrap();

        // Verify all fields match
        assert_eq!(artifact.artifact_version, deserialized.artifact_version);
        assert_eq!(artifact.decision_id, deserialized.decision_id);
        assert_eq!(artifact.decision, deserialized.decision);
        assert_eq!(artifact.policy_info, deserialized.policy_info);
        assert_eq!(artifact.governance_info, deserialized.governance_info);
        assert_eq!(artifact.rules_evaluated, deserialized.rules_evaluated);
        assert_eq!(artifact.evaluation_trace, deserialized.evaluation_trace);
    }
}

#[cfg(test)]
mod backward_compatibility_tests {
    use super::*;

    #[test]
    fn test_load_v1_artifact_with_optional_fields() {
        // Simulate a v1 artifact JSON (without new fields)
        let v1_json = r#"
        {
            "artifact_version": "1.0.0",
            "artifact_type": "pre_execution_decision",
            "decision_id": "test-123",
            "timestamp": "2024-01-01T00:00:00Z",
            "decision": "ALLOW",
            "tool": "aws_ec2_provision",
            "environment": "staging",
            "reason": "",
            "policy_bundle": "infra-cost-limit-v1",
            "policy_hash": "test-hash",
            "sha256_hash": "test-sha256",
            "engine": {
                "name": "Traxes",
                "engine_version": "0.3.2",
                "policy_bundle_id": "infra-cost-limit-v1"
            },
            "execution_context": {
                "session_id": "session-123",
                "trace_id": "trace-123"
            },
            "proposed_action": {
                "tool": "aws_ec2_provision",
                "environment": "staging",
                "parameters": {
                    "instance_type": "t3.medium",
                    "instance_cost_per_hour": 1.50
                }
            },
            "rule_evaluation": {
                "rule_id": "infra-cost-limit",
                "field": "instance_cost_per_hour",
                "observed_value": 1.50,
                "operator": "numeric_lte",
                "policy_value": 2.00,
                "evaluation_expression": "1.50 <= 2.00",
                "evaluation_result": false
            },
            "performance": {
                "evaluation_latency_us": 100.0,
                "decision_latency_us": 50.0,
                "artifact_write_latency_us": 25.0
            },
            "side_effect_prevention": {
                "decision_effect": "ALLOW"
            },
            "execution_status": "executed"
        }
        "#;

        // Should deserialize successfully with None for optional fields
        let artifact: AuditArtifact = serde_json::from_str(v1_json).unwrap();

        assert_eq!(artifact.artifact_version, "1.0.0");
        assert!(artifact.policy_info.is_none());
        assert!(artifact.governance_info.is_none());
        assert!(artifact.rules_evaluated.is_none());
        assert!(artifact.evaluation_trace.is_none());
    }

    #[test]
    fn test_v1_artifact_rule_order_default() {
        // v1 artifact without rule_order should deserialize with None
        let v1_json = r#"
        {
            "artifact_version": "1.0.0",
            "artifact_type": "pre_execution_decision",
            "decision_id": "test-123",
            "timestamp": "2024-01-01T00:00:00Z",
            "decision": "ALLOW",
            "tool": "aws_ec2_provision",
            "environment": "staging",
            "reason": "",
            "policy_bundle": "infra-cost-limit-v1",
            "policy_hash": "test-hash",
            "sha256_hash": "test-sha256",
            "engine": {
                "name": "Traxes",
                "engine_version": "0.3.2",
                "policy_bundle_id": "infra-cost-limit-v1"
            },
            "execution_context": {
                "session_id": "session-123",
                "trace_id": "trace-123"
            },
            "proposed_action": {
                "tool": "aws_ec2_provision",
                "environment": "staging",
                "parameters": {
                    "instance_type": "t3.medium",
                    "instance_cost_per_hour": 1.50
                }
            },
            "rule_evaluation": {
                "rule_id": "infra-cost-limit",
                "field": "instance_cost_per_hour",
                "observed_value": 1.50,
                "operator": "numeric_lte",
                "policy_value": 2.00,
                "evaluation_expression": "1.50 <= 2.00",
                "evaluation_result": false
            },
            "performance": {
                "evaluation_latency_us": 100.0,
                "decision_latency_us": 50.0,
                "artifact_write_latency_us": 25.0
            },
            "side_effect_prevention": {
                "decision_effect": "ALLOW"
            },
            "execution_status": "executed"
        }
        "#;

        let artifact: AuditArtifact = serde_json::from_str(v1_json).unwrap();
        assert!(artifact.rule_evaluation.rule_order.is_none());
    }
}
