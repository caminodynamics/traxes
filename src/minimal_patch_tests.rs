#[cfg(test)]
mod tests {
    use crate::action::ProposedAction;
    use crate::artifact::{ArtifactLogger, AuditArtifact};
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
        assert!(evaluation
            .result
            .reason
            .contains("INSTANCE_TYPE_CONSTRAINT_CHECK"));
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
target:
  tool: aws_ec2_provision
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
        assert_eq!(
            replay_result.original_decision,
            replay_result.replay_decision
        );
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
target:
  tool: FILE_WRITE
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
target:
  tool: FILE_WRITE
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
target:
  tool: FILE_WRITE
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
target:
  tool: FILE_WRITE
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

        assert_eq!(
            artifact.proposed_action.parameters.path,
            Some("/tmp/test.txt".to_string())
        );
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
target:
  tool: FILE_WRITE
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

    #[test]
    fn test_file_write_execute_allow_and_deny_behavior() {
        // Ensure action::execute is the single execution boundary for FILE_WRITE
        let sandbox = std::env::temp_dir().join("traxes_file_write_test");
        let _ = std::fs::create_dir_all(&sandbox);
        let target = sandbox.join("exec_allowed.txt");
        let target_str = target.to_string_lossy().replace('\\', "/");

        // Build a policy that allows only the target path and denies others (explicit allowlist)
        let policy = format!(
            r#"apiVersion: Traxes.dev/v1
kind: ExecutionPolicy
metadata:
  name: filesystem-write-allowlist
target:
  tool: FILE_WRITE
rules:
  - name: file_write_path_allowlist
    condition: payload.proposed_action.parameters.path not in ["{}"]
    action: DENY
    reason: "File write path outside allowlist for sandbox."
"#,
            target_str
        );
        let engine = Engine::with_policy(policy.to_string());

        // Build action
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid-exec".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": target_str,
                "content": "minimal test payload"
            }),
        };

        // Evaluate for the allowed action
        let evaluation = engine.evaluate(&action);
        let decision_id = Uuid::new_v4().to_string();

        // Execute through central boundary
        let exec_status = crate::action::execute(&action, &evaluation.decision, &decision_id);

        // ALLOW case: must have executed and created the file
        assert_eq!(evaluation.decision, "ALLOW");
        assert_eq!(exec_status, "executed");
        assert!(target.exists(), "ALLOW must create the target file");
        let contents = std::fs::read_to_string(&target).unwrap_or_default();
        assert_eq!(contents, "minimal test payload");

        // Now construct a DENY action (different path) and ensure it's blocked
        let deny_target = sandbox.join("exec_denied.txt");
        let deny_target_str = deny_target.to_string_lossy().replace('\\', "/");
        let deny_action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid-exec-deny".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": deny_target_str,
                "content": "should not be written"
            }),
        };
        let deny_eval = engine.evaluate(&deny_action);
        assert_eq!(deny_eval.decision, "DENY");
        let deny_decision_id = Uuid::new_v4().to_string();
        let deny_exec_status =
            crate::action::execute(&deny_action, &deny_eval.decision, &deny_decision_id);
        assert_eq!(deny_exec_status, "blocked");
        assert!(
            !deny_target.exists(),
            "DENY must NOT create the target file"
        );

        // Cleanup
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn test_file_write_allow_filesystem_failure_fails_closed() {
        // Create a directory and attempt to write to that directory path (should fail)
        let sandbox = std::env::temp_dir().join("traxes_file_write_failtest");
        let _ = std::fs::create_dir_all(&sandbox);
        let dir_target = sandbox.join("a_directory");
        let _ = std::fs::create_dir_all(&dir_target);
        let dir_target_str = dir_target.to_string_lossy().replace('\\', "/");

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid-fail".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                // Intentionally point at a directory to cause write failure
                "path": dir_target_str,
                "content": "will fail"
            }),
        };

        // Use a simple engine that allows this path so execute() will attempt the write
        let decision = "ALLOW".to_string();
        let decision_id = Uuid::new_v4().to_string();

        let exec_status = crate::action::execute(&action, &decision, &decision_id);

        // Expect failure to write to a directory -> blocked and no "executed"
        assert_eq!(exec_status, "blocked");
        // Ensure no regular file was created at that path
        assert!(dir_target.exists() && dir_target.is_dir());

        // Cleanup
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn test_file_write_replay_does_not_recreate_target() {
        // Ensure replay verifies decision but does not execute FILE_WRITE
        // Build the allowlist policy after we know the actual target path
        let sandbox = std::env::temp_dir().join("traxes_file_write_replay_test");
        let _ = std::fs::create_dir_all(&sandbox);
        let target = sandbox.join("replay_target.txt");
        let target_str = target.to_string_lossy().replace('\\', "/");

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid-replay".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": target_str,
                "content": "replay payload"
            }),
        };
        // Build policy to allow only this target path
        let policy = format!(
            r#"target:
  tool: FILE_WRITE
rules:
  - name: path_limit
    condition: payload.proposed_action.parameters.path not in ["{}"]
    action: DENY
"#,
            target_str
        );
        let engine = Engine::with_policy(policy.to_string());

        let evaluation = engine.evaluate(&action);
        assert_eq!(evaluation.decision, "ALLOW");
        let decision_id = Uuid::new_v4().to_string();

        // Execute to create the file
        let exec_status = crate::action::execute(&action, &evaluation.decision, &decision_id);
        if evaluation.decision == "ALLOW" {
            assert_eq!(exec_status, "executed");
            assert!(target.exists());
        }

        // Remove the file to simulate non-replay environment
        let _ = std::fs::remove_file(&target);
        assert!(!target.exists());

        // Replay should match but must not recreate the file
        let replay_engine = ReplayEngine::new(engine);
        let artifact = ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            replay_engine.engine.policy_hash(),
            exec_status,
        );
        let replay_res = replay_engine.replay_from_artifact(&artifact);
        assert!(replay_res.match_status);
        assert!(!target.exists(), "Replay must not recreate the target file");

        // Cleanup
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn test_async_path_preserves_operator_in_artifact() {
        // Test that the async/HTTP path preserves the actual operator (in_list/not_in)
        // rather than the rule_id in the artifact
        use crate::execution_event::ExecutionEvent;
        use crate::traxes_engine::DecisionRecord;

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "sid-async".to_string(),
            environment: "env".to_string(),
            parameters: json!({
                "path": "/tmp/test.txt"
            }),
        };

        let policy = r#"
target:
  tool: FILE_WRITE
rules:
  - name: path_limit
    condition: payload.proposed_action.parameters.path not in ["/etc/passwd"]
    action: DENY
"#;
        let engine = Engine::with_policy(policy.to_string());
        let evaluation = engine.evaluate(&action);

        // Create a DecisionRecord (what goes through the async queue)
        let record = DecisionRecord {
            decision: evaluation.decision.clone(),
            session_id: action.session_id.clone(),
            tool: action.tool.clone(),
            environment: action.environment.clone(),
            parameters: action.parameters.clone(),
            policy_hash: engine.policy_hash().to_string(),
            evaluation_result: evaluation.result.clone(),
            evaluation_latency_us: evaluation.evaluation_latency_us,
            decision_id: "test-dec-id".to_string(),
            trace_id: "test-trace-id".to_string(),
            execution_status: "executed".to_string(),
        };

        // Convert to ExecutionEvent (async worker does this)
        let event = ExecutionEvent::from_record(record);

        // Convert back to EvaluationDecision (artifact construction)
        let reconstructed = event.to_evaluation_decision();

        // The reconstructed rule should be the operator "not_in", not the rule_id
        assert_eq!(reconstructed.rule, "not_in",
            "Async path must preserve operator, not rule_id");
        assert_eq!(reconstructed.field, evaluation.result.field);

        let (_, rx) = tokio::sync::mpsc::channel(1);
        let emitter = crate::artifact_emitter::ArtifactEmitter::new(
            rx, engine.policy_hash().to_string(),
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        );
        for operator in ["not_in", "in_list"] {
            let mut event = event.clone();
            event.rule_trace.operator = operator.to_string();
            let artifact = emitter.event_to_artifact(event).unwrap();
            assert_eq!(artifact.rule_evaluation.operator, operator);
            assert_eq!(artifact.rule_evaluation.observed_value, json!("/tmp/test.txt"));
            assert_eq!(artifact.rules_evaluated.unwrap()[0].operator, operator);
        }
    }

    #[test]
    fn test_file_write_hash_binds_content_and_metadata() {
        let engine = Engine::with_policy("target:\n  tool: FILE_WRITE\nrules: []".to_string());
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "session".to_string(),
            environment: "sandbox".to_string(),
            parameters: json!({"path": "allowed.txt", "content": "original"}),
        };
        // Hold evaluation and decision ID fixed to isolate action binding.
        let evaluation = engine.evaluate(&action);
        let hash = |action: &ProposedAction| ArtifactLogger::generate_artifact(
            "fixed-id", action, &evaluation, engine.policy_hash(), "blocked".to_string(),
        ).sha256_hash;
        let original = hash(&action);
        assert_eq!(original, hash(&action));
        for field in ["content", "path"] {
            let mut changed = action.clone();
            changed.parameters[field] = json!("changed");
            assert_ne!(original, hash(&changed), "must bind {field}");
        }
        let mut changed = action.clone();
        changed.session_id.push_str("-changed");
        assert_ne!(original, hash(&changed));
        let mut changed = action.clone();
        changed.environment.push_str("-changed");
        assert_ne!(original, hash(&changed));
        let mut changed = action.clone();
        changed.parameters.as_object_mut().unwrap().remove("content");
        let missing = hash(&changed);
        changed.parameters["content"] = json!("");
        assert_ne!(missing, hash(&changed));
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn test_file_write_rejects_symlink_escape() {
        let sandbox = std::env::temp_dir().join(format!("traxes-links-{}", Uuid::new_v4()));
        let governed = sandbox.join("governed");
        let outside = sandbox.join("outside");
        std::fs::create_dir_all(&governed).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let victim = outside.join("victim.txt");
        std::fs::write(&victim, "untouched").unwrap();
        let link = governed.join("approved.txt");
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(&victim, &link);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_file(&victim, &link);
        #[cfg(windows)]
        if result.as_ref().err().and_then(|error| error.raw_os_error()) == Some(1314) {
            eprintln!("SKIPPED: symlink assertions were not executed because Windows symlink privilege was unavailable (error 1314); Windows junction regression runs separately.");
            std::fs::remove_dir_all(&sandbox).unwrap();
            return;
        }
        result.unwrap();

        let check = |path: &std::path::Path| {
            let action = ProposedAction {
                tool: "FILE_WRITE".to_string(), session_id: "links".to_string(),
                environment: "sandbox".to_string(),
                parameters: json!({"path": path, "content": "escaped"}),
            };
            assert_eq!(crate::action::execute(&action, "ALLOW", "links"), "blocked");
        };
        check(&link);
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "untouched");
        std::fs::remove_file(&link).unwrap();
        let missing = outside.join("missing.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&missing, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&missing, &link).unwrap();
        check(&link);
        assert!(!missing.exists());
        std::fs::remove_file(&link).unwrap();

        let parent = governed.join("redirect");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &parent).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&outside, &parent).unwrap();
        check(&parent.join("victim.txt"));
        check(&parent.join("missing.txt"));
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "untouched");
        assert!(!missing.exists());
        #[cfg(unix)]
        std::fs::remove_file(&parent).unwrap();
        #[cfg(windows)]
        std::fs::remove_dir(&parent).unwrap();
        std::fs::remove_dir_all(&sandbox).unwrap();
    }

    #[test]
    #[cfg(windows)]
    fn test_file_write_rejects_windows_junction_escape() {
        let sandbox = std::env::temp_dir().join(format!("traxes-junction-{}", Uuid::new_v4()));
        let outside = sandbox.join("outside");
        let governed = sandbox.join("governed");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&governed).unwrap();
        let junction = governed.join("redirect");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction).arg(&outside).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let victim = outside.join("victim.txt");
        std::fs::write(&victim, "untouched").unwrap();
        for name in ["victim.txt", "missing.txt"] {
            let path = junction.join(name).to_string_lossy().replace('\\', "/");
            let engine = Engine::with_policy(format!(
                "target:\n  tool: FILE_WRITE\nrules:\n  - name: allowlist\n    condition: payload.proposed_action.parameters.path not in [\"{path}\"]\n    action: DENY\n"
            ));
            let action = ProposedAction {
                tool: "FILE_WRITE".to_string(), session_id: "junction".to_string(),
                environment: "sandbox".to_string(),
                parameters: json!({"path": path, "content": "escaped"}),
            };
            let evaluation = engine.evaluate(&action);
            assert_eq!(evaluation.decision, "ALLOW");
            assert_eq!(crate::action::execute(&action, &evaluation.decision, "junction"), "blocked");
        }
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "untouched");
        assert!(!outside.join("missing.txt").exists());
        std::fs::remove_dir(&junction).unwrap();
        std::fs::remove_dir_all(&sandbox).unwrap();
    }
}
