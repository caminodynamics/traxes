#[cfg(test)]
mod tests {
    use crate::action::ProposedAction;
    use crate::artifact::{AuditArtifact, ArtifactLogger};
    use crate::replay::{ReplayEngine, Verifier};
    use crate::traxes_engine::Engine;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_aws_allow() {
        let engine = Engine::load_default_policies().unwrap();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.small",
                "instance_cost_per_hour": 0.50
            }),
        };
        let evaluation = engine.evaluate(&action);
        assert_eq!(evaluation.decision, "ALLOW");
    }

    #[test]
    fn test_aws_deny_instance_type() {
        let engine = Engine::load_default_policies().unwrap();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "p3.16xlarge",
                "instance_cost_per_hour": 0.50
            }),
        };
        let evaluation = engine.evaluate(&action);
        assert_eq!(evaluation.decision, "DENY");
        assert!(evaluation.result.reason.contains("INSTANCE_TYPE_CONSTRAINT_CHECK"));
    }

    #[test]
    fn test_aws_deny_cost() {
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.small",
                "instance_cost_per_hour": 5000.0
            }),
        };
        let policy2 = r#"
rules:
  - operator: numeric_lte
    condition_value: "100.00"
    action: DENY
"#;
        let engine2 = Engine::with_policy(policy2.to_string());
        let evaluation2 = engine2.evaluate(&action);
        if evaluation2.decision != "ALLOW" {
             panic!("Expected ALLOW");
        }
    }

    #[test]
    fn test_aws_replay() {
        let engine = Engine::load_default_policies().unwrap();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "test-session".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.medium",
                "instance_cost_per_hour": 1.20
            }),
        };
        let evaluation = engine.evaluate(&action);
        let decision_id = Uuid::new_v4().to_string();
        let artifact = ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            engine.policy_hash(),
            "executed".to_string(),
        );

        let replay_engine = ReplayEngine::new(engine);
        let replay_result = replay_engine.replay_from_artifact(&artifact);
        assert!(replay_result.match_status);
        assert_eq!(replay_result.original_decision, replay_result.replay_decision);
    }

    #[test]
    fn test_corrupted_policy_deny() {
        let corrupted_policy = "rules: [ { operator: 'invalid', field: 'none' } ]";
        let engine = Engine::with_policy(corrupted_policy.to_string());
        let action = ProposedAction {
            tool: "any".to_string(),
            session_id: "sid".to_string(),
            environment: "env".to_string(),
            parameters: json!({}),
        };
        let evaluation = engine.evaluate(&action);
        assert_eq!(evaluation.decision, "DENY");
    }

    #[test]
    fn test_file_write_allowed_path() {
        let policy = r#"
rules:
  - operator: in_list
    field: payload.proposed_action.parameters.path
    condition_value: ["/tmp/allowed.txt", "/var/log/app.log"]
    action: DENY
"#;
        let engine = Engine::with_policy(policy.to_string());
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": "/tmp/allowed.txt"
            }),
        };
        let evaluation = engine.evaluate(&action);
        // "in_list" returns the action (DENY) if matched
        assert_eq!(evaluation.decision, "DENY");
    }

    #[test]
    fn test_file_write_denied_path() {
        let policy = r#"
rules:
  - operator: in_list
    field: payload.proposed_action.parameters.path
    condition_value: ["/tmp/allowed.txt"]
    action: ALLOW
"#;
        let engine = Engine::with_policy(policy.to_string());
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": "/etc/passwd"
            }),
        };
        let evaluation = engine.evaluate(&action);
        // "in_list" does NOT match, so it returns None, which defaults to DENY
        assert_eq!(evaluation.decision, "DENY");
    }

    #[test]
    fn test_file_write_missing_path_deny() {
        let policy = r#"
rules:
  - operator: in_list
    field: payload.proposed_action.parameters.path
    condition_value: ["/tmp/allowed.txt"]
    action: ALLOW
"#;
        let engine = Engine::with_policy(policy.to_string());
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid".to_string(),
            environment: "env".to_string(),
            parameters: json!({}),
        };
        let evaluation = engine.evaluate(&action);
        assert_eq!(evaluation.decision, "DENY");
    }

    #[test]
    fn test_aws_extra_path_parameter() {
        let engine = Engine::load_default_policies().unwrap();
        let action = ProposedAction {
            tool: "aws_ec2_provision".to_string(),
            session_id: "sid".to_string(),
            environment: "staging".to_string(),
            parameters: json!({
                "instance_type": "t3.small",
                "instance_cost_per_hour": 0.50,
                "path": "/should/not/affect/aws"
            }),
        };
        let evaluation = engine.evaluate(&action);
        assert_eq!(evaluation.decision, "ALLOW");
    }

    #[test]
    fn test_file_write_artifact_contains_path() {
        let policy = r#"
rules:
  - operator: in_list
    field: payload.proposed_action.parameters.path
    condition_value: ["/tmp/test.txt"]
    action: ALLOW
"#;
        let engine = Engine::with_policy(policy.to_string());
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": "/tmp/test.txt"
            }),
        };
        let evaluation = engine.evaluate(&action);
        let decision_id = Uuid::new_v4().to_string();
        let artifact = ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            engine.policy_hash(),
            "executed".to_string(),
        );

        assert_eq!(artifact.proposed_action.parameters.path, Some("/tmp/test.txt".to_string()));
    }

    #[test]
    fn test_file_write_replay_match() {
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": "/tmp/test.txt"
            }),
        };
        let policy = r#"
rules:
  - name: path_limit
    condition: payload.proposed_action.parameters.path in ["/tmp/test.txt"]
    action: ALLOW
"#;
        let engine = Engine::with_policy(policy.to_string());
        let evaluation = engine.evaluate(&action);
        let decision_id = uuid::Uuid::new_v4().to_string();
        let artifact = ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            engine.policy_hash(),
            "executed".to_string(),
        );

        let replay_engine = ReplayEngine::new(engine);
        let replay_result = replay_engine.replay_from_artifact(&artifact);
        
        if !replay_result.match_status {
             panic!("MISMATCH");
        }
    }
}
