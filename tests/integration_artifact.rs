//! Integration tests for decision → artifact flow

use traxes_demo::artifact::{AuditArtifact, ArtifactLogger};
use traxes_demo::traxes_engine::EvaluationDecision;
use traxes_demo::action::ProposedAction;
use traxes_demo::server_policy::EvaluationResult;
use traxes_demo::execution_event::{ExecutionEvent, RuleTrace, ReplayMetadata, PerformanceMetrics};
use traxes_demo::artifact_emitter::ArtifactEmitter;
use std::fs;
use std::sync::{Arc, atomic::AtomicU64};

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
    assert_eq!(artifact.artifact_version, "2.0.0");
    
    // For ALLOW decisions, reason should be empty
    assert!(artifact.reason.is_empty());
    
    // Verify engine info
    assert_eq!(artifact.engine.name, "Traxes");
    assert_eq!(artifact.engine.engine_version, "1.0.0");
    
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

#[test]
fn test_artifact_construction_identity() {
    // Verify that async and sync artifact construction paths produce identical artifacts
    // This is critical for the architectural invariant: single canonical artifact builder
    
    // Create evaluation context
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
        result: evaluation_result.clone(),
        decision: "DENY".to_string(),
        evaluation_latency_us: 150.0,
    };
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "test-session-identity".to_string(),
        environment: "test".to_string(),
        parameters: serde_json::json!({
            "instance_cost_per_hour": 1.50,
            "instance_type": "t3.medium"
        }),
    };
    
    let policy_hash = "test-policy-hash-12345";
    let decision_id = "test-decision-identity";
    
    // Generate artifact via sync path (canonical builder)
    let sync_artifact = ArtifactLogger::generate_artifact(
        decision_id,
        &action,
        &decision,
        policy_hash,
        "blocked".to_string(),
    );
    
    // Generate artifact via async path (simulate EventEmitter::event_to_artifact)
    let rule_trace = RuleTrace {
        rule_id: "infra-cost-limit".to_string(),
        field: "instance_cost_per_hour".to_string(),
        observed_value: "1.50".to_string(),
        operator: "numeric_lte".to_string(),
        violation: true,
        evaluation_result: true,
    };
    
    let replay_metadata = ReplayMetadata {
        policy_hash: policy_hash.to_string(),
        action_hash: "test-action-hash".to_string(),
        engine_version: "1.0.0".to_string(),
        sequence: 0,
    };
    
    let performance = PerformanceMetrics {
        evaluation_latency_us: 150,
        decision_latency_us: 4,
    };
    
    let event = ExecutionEvent::new(
        decision_id.to_string(),
        action.session_id.clone(),
        decision_id.to_string(), // trace_id same as decision_id for test
        action.tool.clone(),
        action.environment.clone(),
        decision.decision.clone(),
        rule_trace,
        replay_metadata,
        performance,
        "blocked".to_string(),
        action.parameters.clone(),
        decision.result.policy_value,
        decision.result.reason.clone(),
    );
    
    // Create artifact emitter and convert event to artifact
    let (_tx, rx) = tokio::sync::mpsc::channel(100);
    let emitter = ArtifactEmitter::new(rx, policy_hash.to_string(), Arc::new(AtomicU64::new(0)));
    let async_artifact = emitter.event_to_artifact(event)
        .expect("Failed to convert event to artifact");
    
    // Verify artifacts are identical in all critical fields
    assert_eq!(sync_artifact.decision, async_artifact.decision, "Decision mismatch");
    assert_eq!(sync_artifact.sha256_hash, async_artifact.sha256_hash, "Hash mismatch");
    assert_eq!(sync_artifact.policy_bundle, async_artifact.policy_bundle, "Policy bundle mismatch");
    assert_eq!(sync_artifact.policy_hash, async_artifact.policy_hash, "Policy hash mismatch");
    assert_eq!(sync_artifact.reason, async_artifact.reason, "Reason mismatch");
    assert_eq!(sync_artifact.tool, async_artifact.tool, "Tool mismatch");
    assert_eq!(sync_artifact.environment, async_artifact.environment, "Environment mismatch");
    
    // Verify parameters are identical
    assert_eq!(
        sync_artifact.proposed_action.parameters.instance_type,
        async_artifact.proposed_action.parameters.instance_type,
        "Instance type mismatch"
    );
    assert_eq!(
        sync_artifact.proposed_action.parameters.instance_cost_per_hour,
        async_artifact.proposed_action.parameters.instance_cost_per_hour,
        "Instance cost mismatch"
    );
    
    // Verify rule evaluation info
    assert_eq!(
        sync_artifact.rule_evaluation.rule_id,
        async_artifact.rule_evaluation.rule_id,
        "Rule ID mismatch"
    );
    assert_eq!(
        sync_artifact.rule_evaluation.field,
        async_artifact.rule_evaluation.field,
        "Field mismatch"
    );
    assert_eq!(
        sync_artifact.rule_evaluation.observed_value,
        async_artifact.rule_evaluation.observed_value,
        "Observed value mismatch"
    );
    
    // Verify execution context
    assert_eq!(
        sync_artifact.execution_context.session_id,
        async_artifact.execution_context.session_id,
        "Session ID mismatch"
    );
    assert_eq!(
        sync_artifact.execution_context.trace_id,
        async_artifact.execution_context.trace_id,
        "Trace ID mismatch"
    );
    
    // Verify performance metrics (allow small variance in artifact_write_latency_us)
    assert_eq!(
        sync_artifact.performance.evaluation_latency_us,
        async_artifact.performance.evaluation_latency_us,
        "Evaluation latency mismatch"
    );
}
