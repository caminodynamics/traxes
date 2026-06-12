//! Integration tests for complete evaluation flow: Payload → Engine → Decision

use traxes_demo::traxes_engine::Engine;
use traxes_demo::action::ProposedAction;
use std::fs;

#[test]
fn test_numeric_lte_complete_flow_allow() {
    // Load engine with numeric_lte policy
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_numeric_lte.yaml")
        .expect("Failed to load policy");
    
    // Load payload with cost above threshold (should ALLOW)
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/numeric_lte_allow.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate through Engine
    let decision = engine.evaluate(&action);
    
    // Verify decision is ALLOW
    assert_eq!(decision.decision, "ALLOW");
    assert!(!decision.is_deny());
    
    // Verify evaluation result fields
    assert_eq!(decision.result.field, "instance_cost_per_hour");
    assert_eq!(decision.result.rule, "numeric_lte");
    assert_eq!(decision.result.observed_value, 2.50);
}

#[test]
fn test_numeric_lte_complete_flow_deny() {
    // Load engine with numeric_lte policy
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_numeric_lte.yaml")
        .expect("Failed to load policy");
    
    // Load payload with cost below threshold (should DENY)
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/numeric_lte_deny.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate through Engine
    let decision = engine.evaluate(&action);
    
    // Verify decision is DENY
    assert_eq!(decision.decision, "DENY");
    assert!(decision.is_deny());
    
    // Verify action is set
    assert_eq!(decision.result.action, Some("DENY".to_string()));
}

#[test]
fn test_in_list_complete_flow_deny() {
    // Load engine with in_list policy
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_in_list.yaml")
        .expect("Failed to load policy");
    
    // Load payload with instance_type in list (should DENY)
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/in_list_deny.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate through Engine
    let decision = engine.evaluate(&action);
    
    // Verify decision is DENY
    assert_eq!(decision.decision, "DENY");
    assert!(decision.is_deny());
    
    // Verify field and operator
    assert!(decision.result.field.contains("instance_type"));
    assert_eq!(decision.result.rule, "in_list");
}

#[test]
fn test_in_list_complete_flow_allow() {
    // Load engine with in_list policy
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_in_list.yaml")
        .expect("Failed to load policy");
    
    // Load payload with instance_type not in list (should ALLOW)
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/in_list_allow.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate through Engine
    let decision = engine.evaluate(&action);
    
    // Verify decision is ALLOW
    assert_eq!(decision.decision, "ALLOW");
    assert!(!decision.is_deny());
    
    // Verify no action taken
    assert_eq!(decision.result.action, None);
}

#[test]
fn test_not_in_complete_flow_deny() {
    // Load engine with not_in policy
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_not_in.yaml")
        .expect("Failed to load policy");
    
    // Load payload with instance_type not in allowed list (should DENY)
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/not_in_deny.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate through Engine
    let decision = engine.evaluate(&action);
    
    // Verify decision is DENY
    assert_eq!(decision.decision, "DENY");
    assert!(decision.is_deny());
    
    // Verify operator
    assert_eq!(decision.result.rule, "not_in");
}

#[test]
fn test_not_in_complete_flow_allow() {
    // Load engine with not_in policy
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_not_in.yaml")
        .expect("Failed to load policy");
    
    // Load payload with instance_type in allowed list (should ALLOW)
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/not_in_allow.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate through Engine
    let decision = engine.evaluate(&action);
    
    // Verify decision is ALLOW
    assert_eq!(decision.decision, "ALLOW");
    assert!(!decision.is_deny());
    
    // Verify no action taken
    assert_eq!(decision.result.action, None);
}

#[test]
fn test_policy_hash_consistency() {
    // Load same policy twice and verify hash consistency
    let engine1 = Engine::load("../tests/fixtures/test_fixtures/policies/test_numeric_lte.yaml")
        .expect("Failed to load policy");
    let engine2 = Engine::load("../tests/fixtures/test_fixtures/policies/test_numeric_lte.yaml")
        .expect("Failed to load policy");
    
    assert_eq!(engine1.policy_hash(), engine2.policy_hash());
    
    // Verify hash is non-empty
    assert!(!engine1.policy_hash().is_empty());
}

#[test]
fn test_evaluation_latency_measurement() {
    // Load engine
    let engine = Engine::load("../tests/fixtures/test_fixtures/policies/test_numeric_lte.yaml")
        .expect("Failed to load policy");
    
    // Load payload
    let payload_str = fs::read_to_string("../tests/fixtures/test_fixtures/payloads/numeric_lte_allow.json")
        .expect("Failed to load payload");
    let action: ProposedAction = serde_json::from_str(&payload_str)
        .expect("Failed to parse payload");
    
    // Evaluate
    let decision = engine.evaluate(&action);
    
    // Verify latency is measured
    assert!(decision.evaluation_latency_us > 0.0);
    assert!(decision.evaluation_latency_us < 1000000.0); // Should be less than 1 second
}
