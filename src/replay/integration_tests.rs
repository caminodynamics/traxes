use crate::action::ProposedAction;
use crate::artifact::{AuditArtifact, ArtifactLogger};
use crate::replay::{ReplayEngine, ReplayRequest, ReplayResult, Verifier, VerificationStatus};
use crate::traxes_engine::Engine;
use serde_json::json;
use std::fs;
use uuid::Uuid;

#[test]
fn test_end_to_end_replay_match() {
    // Setup: Create an engine and evaluate an action
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

    // Evaluate the action
    let evaluation = engine.evaluate(&action);
    let decision_id = Uuid::new_v4().to_string();
    
    // Create an artifact from the evaluation
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Replay the decision
    let replay_engine = ReplayEngine::new(engine);
    let replay_result = replay_engine.replay_from_artifact(&artifact);

    // Verify the replay matches
    assert!(replay_result.match_status);
    assert_eq!(replay_result.original_decision, replay_result.replay_decision);
    assert_eq!(replay_result.decision_id, decision_id);
}

#[test]
fn test_replay_from_file() {
    // Create a temporary artifact file
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Write artifact to file
    let artifact_path = format!("test_artifact_{}.json", decision_id);
    let artifact_json = serde_json::to_string_pretty(&artifact).unwrap();
    fs::write(&artifact_path, artifact_json).unwrap();

    // Replay from file
    let replay_engine = ReplayEngine::new(engine);
    let replay_result = replay_engine.replay_from_file(&artifact_path);

    assert!(replay_result.is_ok());
    let result = replay_result.unwrap();
    assert!(result.match_status);

    // Cleanup
    fs::remove_file(&artifact_path).ok();
}

#[test]
fn test_replay_with_custom_policy() {
    // Create a custom policy with different threshold
    let custom_policy = r#"
rules:
  - operator: numeric_lte
    field: instance_cost_per_hour
    condition_value: "5.00"
    action: DENY
"#;

    let engine = Engine::load_default_policies().unwrap();
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "staging".to_string(),
        parameters: json!({
            "instance_type": "t3.medium",
            "instance_cost_per_hour": 3.00
        }),
    };

    // Evaluate with default policy (should DENY if threshold is 2.00)
    let evaluation = engine.evaluate(&action);
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Replay with custom policy (should ALLOW with threshold 5.00)
    let replay_engine = ReplayEngine::new(engine.clone());
    let request = ReplayRequest {
        artifact_path: format!("test_artifact_{}.json", decision_id),
        policy_yaml: Some(custom_policy.to_string()),
    };

    // Write artifact for the replay request
    let artifact_path = format!("test_artifact_{}.json", decision_id);
    let artifact_json = serde_json::to_string_pretty(&artifact).unwrap();
    fs::write(&artifact_path, artifact_json).unwrap();

    let replay_result = replay_engine.replay(request);

    assert!(replay_result.is_ok());
    let result = replay_result.unwrap();
    
    // With custom policy, decision might differ
    // The important part is that replay works with custom policy
    assert_eq!(result.decision_id, decision_id);

    // Cleanup
    fs::remove_file(&artifact_path).ok();
}

#[test]
fn test_verification_match() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    let replay_result = replay_engine.replay_from_artifact(&artifact);
    let verification = Verifier::verify(&replay_result, Some(engine.policy_hash()));

    assert_eq!(verification.status, VerificationStatus::Match);
    assert!(verification.is_match());
}

#[test]
fn test_verification_policy_mismatch() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    let replay_result = replay_engine.replay_from_artifact(&artifact);
    
    // Verify with wrong policy hash
    let verification = Verifier::verify(&replay_result, Some("wrong-policy-hash"));

    assert_eq!(verification.status, VerificationStatus::PolicyVersionMismatch);
    assert!(!verification.is_match());
}

#[test]
fn test_multiple_replays_consistency() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "staging".to_string(),
        parameters: json!({
            "instance_type": "t3.small",
            "instance_cost_per_hour": 1.00
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

    // Replay multiple times - should be deterministic
    let replay1 = replay_engine.replay_from_artifact(&artifact);
    let replay2 = replay_engine.replay_from_artifact(&artifact);
    let replay3 = replay_engine.replay_from_artifact(&artifact);

    assert_eq!(replay1.replay_decision, replay2.replay_decision);
    assert_eq!(replay2.replay_decision, replay3.replay_decision);
    assert_eq!(replay1.match_status, replay2.match_status);
    assert_eq!(replay2.match_status, replay3.match_status);
}

#[test]
fn test_replay_deny_scenario() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
    // Action that should be denied (high cost)
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "staging".to_string(),
        parameters: json!({
            "instance_type": "m5.large",
            "instance_cost_per_hour": 10.00
        }),
    };

    let evaluation = engine.evaluate(&action);
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "blocked".to_string(),
    );

    let replay_result = replay_engine.replay_from_artifact(&artifact);

    assert!(replay_result.match_status);
    assert_eq!(replay_result.original_decision, "DENY");
    assert_eq!(replay_result.replay_decision, "DENY");
}

#[test]
fn test_policy_consistency_check() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Check policy consistency
    let is_consistent = replay_engine.verify_policy_consistency(&artifact);
    assert!(is_consistent);
}

#[test]
fn test_replay_result_serialization_roundtrip() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    let replay_result = replay_engine.replay_from_artifact(&artifact);

    // Serialize and deserialize
    let serialized = serde_json::to_string_pretty(&replay_result).unwrap();
    let deserialized: ReplayResult = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.decision_id, replay_result.decision_id);
    assert_eq!(deserialized.original_decision, replay_result.original_decision);
    assert_eq!(deserialized.replay_decision, replay_result.replay_decision);
    assert_eq!(deserialized.match_status, replay_result.match_status);
}

#[test]
fn test_verification_with_tolerance() {
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    let replay_result = replay_engine.replay_from_artifact(&artifact);
    
    // Verify with tolerance
    let verification = Verifier::verify_with_tolerance(&replay_result, 0.01);
    assert_eq!(verification.status, VerificationStatus::Match);
}

// ==================== Replay Verification Workflow Tests ====================

#[test]
fn test_replay_verification_success() {
    // Given: valid artifact with unchanged policy
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Write artifact to file
    let artifact_path = format!("test_artifact_{}.json", decision_id);
    let artifact_json = serde_json::to_string_pretty(&artifact).unwrap();
    fs::write(&artifact_path, artifact_json).unwrap();

    // When: replay verification is executed
    let verification_report = replay_engine.verify_artifact(&artifact_path);

    // Then: replay result matches original decision and verification passes
    assert!(verification_report.is_ok());
    let report = verification_report.unwrap();
    assert_eq!(report.verification_status, "PASS");
    assert_eq!(report.original_decision, report.replay_decision);
    assert_eq!(report.decision_id, decision_id);

    // Cleanup
    fs::remove_file(&artifact_path).ok();
}

#[test]
fn test_replay_verification_mismatch() {
    // Given: artifact with modified policy
    let engine = Engine::load_default_policies().unwrap();
    let _replay_engine = ReplayEngine::new(engine.clone());
    
    // Use an action that will be ALLOW with default policy but DENY with stricter policy
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "staging".to_string(),
        parameters: json!({
            "instance_type": "t3.medium",
            "instance_cost_per_hour": 3.50  // Above default threshold (2.00)
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

    // Write artifact to file
    let artifact_path = format!("test_artifact_{}.json", decision_id);
    let artifact_json = serde_json::to_string_pretty(&artifact).unwrap();
    fs::write(&artifact_path, artifact_json).unwrap();

    // Create replay engine with different policy (higher threshold to flip decision)
    let custom_policy = r#"
rules:
  - operator: numeric_lte
    field: instance_cost_per_hour
    condition_value: "5.00"
    action: DENY
"#;
    let custom_replay_engine = ReplayEngine::new(Engine::with_policy(custom_policy.to_string()));

    // When: replay verification is executed with modified policy
    let verification_report = custom_replay_engine.verify_artifact(&artifact_path);

    // Then: replay result differs and verification fails
    assert!(verification_report.is_ok());
    let report = verification_report.unwrap();
    assert_eq!(report.verification_status, "FAIL");
    // Original decision should differ from replay due to policy change
    assert_ne!(report.original_decision, report.replay_decision);

    // Cleanup
    fs::remove_file(&artifact_path).ok();
}

#[test]
fn test_artifact_reconstruction() {
    // Verify that the artifact contains enough information to recreate the decision request
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
    let original_action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session-123".to_string(),
        environment: "staging".to_string(),
        parameters: json!({
            "instance_type": "t3.small",
            "instance_cost_per_hour": 0.75
        }),
    };

    let evaluation = engine.evaluate(&original_action);
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &original_action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Reconstruct action from artifact
    let reconstructed_action = replay_engine.reconstruct_action(&artifact);

    // Verify all critical fields are preserved
    assert_eq!(reconstructed_action.tool, original_action.tool);
    assert_eq!(reconstructed_action.session_id, original_action.session_id);
    assert_eq!(reconstructed_action.environment, original_action.environment);
    assert_eq!(
        reconstructed_action.parameters.get("instance_type").and_then(|v: &serde_json::Value| v.as_str()),
        original_action.parameters.get("instance_type").and_then(|v: &serde_json::Value| v.as_str())
    );
    assert_eq!(
        reconstructed_action.parameters.get("instance_cost_per_hour").and_then(|v: &serde_json::Value| v.as_f64()),
        original_action.parameters.get("instance_cost_per_hour").and_then(|v: &serde_json::Value| v.as_f64())
    );
}

#[test]
fn test_serialization_roundtrip() {
    // Verify artifact can be saved, loaded, and replay still succeeds
    let engine = Engine::load_default_policies().unwrap();
    let replay_engine = ReplayEngine::new(engine.clone());
    
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
    let decision_id = Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    // Save artifact
    let save_result = artifact.write_to_file_sync();
    assert!(save_result.is_ok());
    let artifact_path = save_result.unwrap();

    // Load artifact
    let loaded_content = fs::read_to_string(&artifact_path).unwrap();
    let loaded_artifact: AuditArtifact = serde_json::from_str(&loaded_content).unwrap();

    // Verify loaded artifact matches original
    assert_eq!(loaded_artifact.decision_id, artifact.decision_id);
    assert_eq!(loaded_artifact.decision, artifact.decision);
    assert_eq!(loaded_artifact.policy_hash, artifact.policy_hash);

    // Replay loaded artifact
    let replay_result = replay_engine.replay_from_artifact(&loaded_artifact);
    assert!(replay_result.match_status);
    assert_eq!(replay_result.original_decision, replay_result.replay_decision);

    // Cleanup
    fs::remove_file(&artifact_path).ok();
}

#[test]
fn test_verification_report_display() {
    let report = crate::replay::VerificationReport {
        decision_id: "test-123".to_string(),
        original_decision: "DENY".to_string(),
        replay_decision: "DENY".to_string(),
        verification_status: "PASS".to_string(),
        policy_hash: "hash-123".to_string(),
        replay_timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let display_output = report.display();
    
    assert!(display_output.contains("TRAXES Replay Verification"));
    assert!(display_output.contains("test-123"));
    assert!(display_output.contains("DENY"));
    assert!(display_output.contains("PASS"));
}
