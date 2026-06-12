//! Integration tests for decision → artifact flow

use traxes_demo::artifact::{AuditArtifact, ArtifactLogger};
use traxes_demo::traxes_engine::{EvaluationDecision, Engine};
use traxes_demo::action::ProposedAction;
use traxes_demo::server_policy::EvaluationResult;
use std::fs;

#[test]
fn test_allow_decision_artifact_generation() {
    // Create evaluation decision with ALLOW
    let evaluation_result = EvaluationResult {
        action: None,
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 2.50,
        observed_value_str: "2.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "ALLOW".to_string(),
        evaluation_latency_us: 100.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session-001".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 2.50,
            "instance_type": "t3.medium"
        }),
    };
    
    // Generate artifact
    let artifact = ArtifactLogger::generate_artifact("test-decision-001", &action, &decision, "test-policy-hash", "success".to_string());
    
    // Verify artifact fields
    assert_eq!(artifact.decision, "ALLOW");
    assert_eq!(artifact.tool, "aws_ec2_provision");
    assert_eq!(artifact.environment, "test");
    assert_eq!(artifact.artifact_type, "pre_execution_decision");
    assert_eq!(artifact.artifact_version, "1.0.0");
    
    // For ALLOW decisions, reason should be empty
    assert!(artifact.reason.is_empty());
    
    // Verify engine info
    assert_eq!(artifact.engine.name, "Traxes");
    assert_eq!(artifact.engine.engine_version, "0.3.2");
    
    // Verify execution context
    assert_eq!(artifact.execution_context.session_id, "test-session-001");
    assert_eq!(artifact.execution_context.trace_id, "test-decision-001");
}

#[test]
fn test_deny_decision_artifact_generation() {
    // Create evaluation decision with DENY
    let evaluation_result = EvaluationResult {
        action: Some("DENY".to_string()),
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 1.50,
        observed_value_str: "1.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "DENY".to_string(),
        evaluation_latency_us: 150.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session-002".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 1.50,
            "instance_type": "t3.medium"
        }),
    };
    
    // Generate artifact
    let artifact = ArtifactLogger::generate_artifact("test-decision-002", &action, &decision, "test-policy-hash", "success".to_string());
    
    // Verify artifact fields
    assert_eq!(artifact.decision, "DENY");
    assert_eq!(artifact.tool, "aws_ec2_provision");
    
    // For DENY decisions, reason should be populated
    assert!(!artifact.reason.is_empty());
    
    // Verify rule evaluation info
    assert_eq!(artifact.rule_evaluation.rule_id, "infra-cost-limit");
    assert_eq!(artifact.rule_evaluation.field, "instance_cost_per_hour");
    assert_eq!(artifact.rule_evaluation.operator, "numeric_lte");
    assert_eq!(artifact.rule_evaluation.policy_value, 2.00);
    
    // Verify evaluation result is true (rule was violated)
    assert!(artifact.rule_evaluation.evaluation_result);
}

#[test]
fn test_artifact_serialization() {
    // Create a simple artifact
    let evaluation_result = EvaluationResult {
        action: None,
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 2.50,
        observed_value_str: "2.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "ALLOW".to_string(),
        evaluation_latency_us: 100.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 2.50,
            "instance_type": "t3.medium"
        }),
    };
    
    let artifact = ArtifactLogger::generate_artifact("test-decision", &action, &decision, "test-hash", "success".to_string());
    
    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&artifact)
        .expect("Failed to serialize artifact");
    
    // Verify JSON is valid
    assert!(!json_str.is_empty());
    assert!(json_str.contains("\"decision\": \"ALLOW\""));
    assert!(json_str.contains("\"tool\": \"aws_ec2_provision\""));
    
    // Verify can deserialize back
    let deserialized: AuditArtifact = serde_json::from_str(&json_str)
        .expect("Failed to deserialize artifact");
    
    assert_eq!(deserialized.decision, artifact.decision);
    assert_eq!(deserialized.tool, artifact.tool);
}

#[test]
fn test_artifact_sha256_hash() {
    // Create artifact
    let evaluation_result = EvaluationResult {
        action: None,
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 2.50,
        observed_value_str: "2.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "ALLOW".to_string(),
        evaluation_latency_us: 100.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 2.50,
            "instance_type": "t3.medium"
        }),
    };
    
    let artifact1 = ArtifactLogger::generate_artifact("test-decision-001", &action, &decision, "test-hash", "success".to_string());
    let artifact2 = ArtifactLogger::generate_artifact("test-decision-001", &action, &decision, "test-hash", "success".to_string());
    
    // Same inputs should produce same hash
    assert_eq!(artifact1.sha256_hash, artifact2.sha256_hash);
    
    // Hash should be non-empty and valid hex
    assert!(!artifact1.sha256_hash.is_empty());
    assert!(artifact1.sha256_hash.len() == 64); // SHA256 produces 64 hex chars
}

#[test]
fn test_artifact_file_write() {
    // Create artifact
    let evaluation_result = EvaluationResult {
        action: None,
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 2.50,
        observed_value_str: "2.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "ALLOW".to_string(),
        evaluation_latency_us: 100.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 2.50,
            "instance_type": "t3.medium"
        }),
    };
    
    let artifact = ArtifactLogger::generate_artifact("test-decision-write", &action, &decision, "test-hash", "success".to_string());
    
    // Write to file
    let file_path = ArtifactLogger::write_sync(&artifact)
        .expect("Failed to write artifact");
    
    // Verify file was created
    assert!(fs::metadata(&file_path).is_ok());
    
    // Verify file content
    let content = fs::read_to_string(&file_path)
        .expect("Failed to read artifact file");
    
    assert!(!content.is_empty());
    assert!(content.contains("\"decision\": \"ALLOW\""));
    
    // Cleanup
    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_dir_all("artifacts");
}

#[test]
fn test_artifact_performance_metrics() {
    // Create artifact
    let evaluation_result = EvaluationResult {
        action: None,
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 2.50,
        observed_value_str: "2.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "ALLOW".to_string(),
        evaluation_latency_us: 123.45,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 2.50,
            "instance_type": "t3.medium"
        }),
    };
    
    let artifact = ArtifactLogger::generate_artifact("test-decision-perf", &action, &decision, "test-hash", "success".to_string());
    
    // Verify performance metrics
    assert_eq!(artifact.performance.evaluation_latency_us, 123.45);
    assert_eq!(artifact.performance.decision_latency_us, 4.8);
    assert_eq!(artifact.performance.artifact_write_latency_us, 41.7);
}

#[test]
fn test_artifact_side_effect_prevention() {
    // Create DENY artifact
    let evaluation_result = EvaluationResult {
        action: Some("DENY".to_string()),
        field: "instance_cost_per_hour".to_string(),
        rule: "numeric_lte".to_string(),
        observed_value: 1.50,
        observed_value_str: "1.50".to_string(),
        policy_value: 2.00,
        evaluation_expression: "instance_cost_per_hour numeric_lte 2.00".to_string(),
        reason: "INFRA_COST_LIMIT_CHECK".to_string(),
    };
    
    let decision = EvaluationDecision {
        result: evaluation_result,
        decision: "DENY".to_string(),
        evaluation_latency_us: 100.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 1.50,
            "instance_type": "t3.medium"
        }),
    };
    
    let artifact = ArtifactLogger::generate_artifact("test-decision-sep", &action, &decision, "test-hash", "success".to_string());
    
    // Verify side effect prevention info
    assert_eq!(artifact.side_effect_prevention.decision_effect, "DENY");
}
