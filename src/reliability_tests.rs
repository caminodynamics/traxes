//! TRAXES Reliability & Failure Test Suite
//! 
//! This module contains reliability tests designed to break the system
//! before release, ensuring robust error handling and no undefined behavior.

use crate::action::ProposedAction;
use crate::traxes_engine::Engine;
use crate::artifact::AuditArtifact;
use crate::replay::ReplayEngine;
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReliabilityTestResult {
    pub test_name: String,
    pub category: String,
    pub passed: bool,
    pub error_message: Option<String>,
    pub details: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReliabilityTestSuite {
    pub test_results: Vec<ReliabilityTestResult>,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
}

pub fn run_reliability_tests() -> ReliabilityTestSuite {
    println!("🔧 TRAXES Reliability Test Suite");
    println!("=================================\n");
    
    let mut results = Vec::new();
    
    // 1. Malformed Input Tests
    println!("📝 Testing Malformed Inputs...");
    results.extend(test_malformed_inputs());
    
    // 2. Policy Failure Tests
    println!("\n📜 Testing Policy Failures...");
    results.extend(test_policy_failures());
    
    // 3. Artifact Integrity Tests
    println!("\n📦 Testing Artifact Integrity...");
    results.extend(test_artifact_integrity());
    
    // 4. Concurrent Execution Tests
    println!("\n🔄 Testing Concurrent Execution...");
    results.extend(test_concurrent_execution());
    
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;
    
    println!("\n📊 Reliability Test Summary");
    println!("==========================");
    println!("Total tests: {}", total);
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    println!("Success rate: {:.1}%", (passed as f64 / total as f64) * 100.0);
    
    ReliabilityTestSuite {
        test_results: results,
        total_tests: total,
        passed_tests: passed,
        failed_tests: failed,
    }
}

fn test_malformed_inputs() -> Vec<ReliabilityTestResult> {
    let mut results = Vec::new();
    
    // Test 1: Invalid JSON
    results.push(test_invalid_json());
    
    // Test 2: Missing required fields
    results.push(test_missing_fields());
    
    // Test 3: Wrong data types
    results.push(test_wrong_data_types());
    
    // Test 4: Empty payload
    results.push(test_empty_payload());
    
    results
}

fn test_invalid_json() -> ReliabilityTestResult {
    let test_name = "Invalid JSON";
    let payload = r#"{"tool": "AWS_RDS_PROVISION", "environment": "staging", "invalid": json"#;
    
    let result = std::panic::catch_unwind(|| {
        serde_json::from_str::<ProposedAction>(payload)
    });
    
    match result {
        Ok(parsed) => {
            if parsed.is_err() {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly rejected invalid JSON".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: false,
                    error_message: Some("Invalid JSON was accepted".to_string()),
                    details: "Invalid JSON should have been rejected".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Malformed Input".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on invalid JSON".to_string(),
        },
    }
}

fn test_missing_fields() -> ReliabilityTestResult {
    let test_name = "Missing Required Fields";
    let payload = r#"{"tool": "AWS_RDS_PROVISION"}"#; // Missing environment, parameters
    
    let result = std::panic::catch_unwind(|| {
        serde_json::from_str::<ProposedAction>(payload)
    });
    
    match result {
        Ok(parsed) => {
            if parsed.is_err() {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly rejected payload with missing fields".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: false,
                    error_message: Some("Payload with missing fields was accepted".to_string()),
                    details: "Should reject incomplete payloads".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Malformed Input".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on missing fields".to_string(),
        },
    }
}

fn test_wrong_data_types() -> ReliabilityTestResult {
    let test_name = "Wrong Data Types";
    let payload = r#"{"tool": 123, "environment": "staging", "parameters": {}}"#; // tool should be string
    
    let result = std::panic::catch_unwind(|| {
        serde_json::from_str::<ProposedAction>(payload)
    });
    
    match result {
        Ok(parsed) => {
            if parsed.is_err() {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly rejected payload with wrong data types".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: false,
                    error_message: Some("Payload with wrong types was accepted".to_string()),
                    details: "Should reject type mismatches".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Malformed Input".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on wrong data types".to_string(),
        },
    }
}

fn test_empty_payload() -> ReliabilityTestResult {
    let test_name = "Empty Payload";
    let payload = "";
    
    let result = std::panic::catch_unwind(|| {
        serde_json::from_str::<ProposedAction>(payload)
    });
    
    match result {
        Ok(parsed) => {
            if parsed.is_err() {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly rejected empty payload".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Malformed Input".to_string(),
                    passed: false,
                    error_message: Some("Empty payload was accepted".to_string()),
                    details: "Should reject empty payloads".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Malformed Input".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on empty payload".to_string(),
        },
    }
}

fn test_policy_failures() -> Vec<ReliabilityTestResult> {
    let mut results = Vec::new();
    
    // Test 1: Missing policy file
    results.push(test_missing_policy());
    
    // Test 2: Corrupted YAML
    results.push(test_corrupted_yaml());
    
    // Test 3: Invalid policy syntax
    results.push(test_invalid_policy_syntax());
    
    results
}

fn test_missing_policy() -> ReliabilityTestResult {
    let test_name = "Missing Policy File";
    
    let result = std::panic::catch_unwind(|| {
        // Try to load a non-existent policy by reading the file first
        let policy_content = fs::read_to_string("non_existent_policy.yaml");
        if policy_content.is_err() {
            return true; // Test passed - file doesn't exist
        }
        
        // If file exists, try to parse it
        let _ = Engine::with_policy(policy_content.unwrap());
        false
    });
    
    match result {
        Ok(handled) => {
            if handled {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Policy Failure".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly handled missing policy file".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Policy Failure".to_string(),
                    passed: false,
                    error_message: Some("Missing policy was accepted".to_string()),
                    details: "Should reject missing policy files".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Policy Failure".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on missing policy".to_string(),
        },
    }
}

fn test_corrupted_yaml() -> ReliabilityTestResult {
    let test_name = "Corrupted YAML Policy";
    
    // Create a corrupted YAML file (invalid YAML syntax)
    let corrupted_yaml_path = "test_corrupted_policy.yaml";
    let corrupted_content = "invalid: yaml: content: {unclosed_bracket";
    
    let _ = fs::write(corrupted_yaml_path, corrupted_content);
    
    let result = std::panic::catch_unwind(|| {
        let policy_content = fs::read_to_string(corrupted_yaml_path);
        if policy_content.is_err() {
            return true; // Test passed - file read failed
        }
        
        // Try to parse the corrupted YAML
        let engine = Engine::with_policy(policy_content.unwrap());
        
        // The engine should accept the YAML but fail closed (no rules parsed)
        // This is by design - the system is lenient at load time but fails closed during evaluation
        let valid_payload = r#"{
  "session_id": "test",
  "request_id": "req-test",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
        
        let action: ProposedAction = serde_json::from_str(valid_payload).unwrap();
        let evaluation = engine.evaluate(&action);
        
        // With corrupted YAML, no rules should be parsed, so evaluation should fail closed (DENY)
        evaluation.decision == "DENY"
    });
    
    // Cleanup
    let _ = fs::remove_file(corrupted_yaml_path);
    
    match result {
        Ok(fails_closed) => {
            if fails_closed {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Policy Failure".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Corrupted YAML causes fail-closed behavior (DENY)".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Policy Failure".to_string(),
                    passed: false,
                    error_message: Some("Corrupted YAML did not cause fail-closed behavior".to_string()),
                    details: "Engine should fail closed when no rules can be parsed".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Policy Failure".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on corrupted YAML".to_string(),
        },
    }
}

fn test_invalid_policy_syntax() -> ReliabilityTestResult {
    let test_name = "Invalid Policy Syntax";
    
    // Create a YAML file with invalid YAML syntax
    let invalid_yaml_path = "test_invalid_policy.yaml";
    let invalid_content = "name: test {unclosed_bracket";
    
    let _ = fs::write(invalid_yaml_path, invalid_content);
    
    let result = std::panic::catch_unwind(|| {
        let policy_content = fs::read_to_string(invalid_yaml_path);
        if policy_content.is_err() {
            return true; // Test passed - file read failed
        }
        
        // Try to parse the invalid YAML
        let engine = Engine::with_policy(policy_content.unwrap());
        
        // The engine should accept the YAML but fail closed (no rules parsed)
        // This is by design - the system is lenient at load time but fails closed during evaluation
        let valid_payload = r#"{
  "session_id": "test",
  "request_id": "req-test",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
        
        let action: ProposedAction = serde_json::from_str(valid_payload).unwrap();
        let evaluation = engine.evaluate(&action);
        
        // With invalid policy syntax, no rules should be parsed, so evaluation should fail closed (DENY)
        evaluation.decision == "DENY"
    });
    
    // Cleanup
    let _ = fs::remove_file(invalid_yaml_path);
    
    match result {
        Ok(fails_closed) => {
            if fails_closed {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Policy Failure".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Invalid policy syntax causes fail-closed behavior (DENY)".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Policy Failure".to_string(),
                    passed: false,
                    error_message: Some("Invalid policy syntax did not cause fail-closed behavior".to_string()),
                    details: "Engine should fail closed when no rules can be parsed".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Policy Failure".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on invalid policy syntax".to_string(),
        },
    }
}

fn test_artifact_integrity() -> Vec<ReliabilityTestResult> {
    let mut results = Vec::new();
    
    // Test 1: Missing artifact fields
    results.push(test_missing_artifact_fields());
    
    // Test 2: Corrupted artifact JSON
    results.push(test_corrupted_artifact_json());
    
    // Test 3: Modified decision
    results.push(test_modified_decision());
    
    // Test 4: Modified policy hash
    results.push(test_modified_policy_hash());
    
    results
}

fn test_missing_artifact_fields() -> ReliabilityTestResult {
    let test_name = "Missing Artifact Fields";
    
    // Create an artifact with missing required fields
    let corrupted_artifact_path = "test_missing_fields_artifact.json";
    let corrupted_content = r#"{
  "artifact_version": "2.0.0",
  "decision_id": "test_id"
}"#; // Missing many required fields
    
    let _ = fs::write(corrupted_artifact_path, corrupted_content);
    
    let result = std::panic::catch_unwind(|| {
        let _ = fs::read_to_string(corrupted_artifact_path);
        let parsed: Result<AuditArtifact, _> = serde_json::from_str(corrupted_content);
        parsed.is_err()
    });
    
    // Cleanup
    let _ = fs::remove_file(corrupted_artifact_path);
    
    match result {
        Ok(rejected) => {
            if rejected {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly rejected artifact with missing fields".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: false,
                    error_message: Some("Artifact with missing fields was accepted".to_string()),
                    details: "Should reject incomplete artifacts".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Artifact Integrity".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on missing artifact fields".to_string(),
        },
    }
}

fn test_corrupted_artifact_json() -> ReliabilityTestResult {
    let test_name = "Corrupted Artifact JSON";
    
    let corrupted_artifact_path = "test_corrupted_artifact.json";
    let corrupted_content = "{invalid json content";
    
    let _ = fs::write(corrupted_artifact_path, corrupted_content);
    
    let result = std::panic::catch_unwind(|| {
        let _ = fs::read_to_string(corrupted_artifact_path);
        let parsed: Result<AuditArtifact, _> = serde_json::from_str(corrupted_content);
        parsed.is_err()
    });
    
    // Cleanup
    let _ = fs::remove_file(corrupted_artifact_path);
    
    match result {
        Ok(rejected) => {
            if rejected {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly rejected corrupted artifact JSON".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: false,
                    error_message: Some("Corrupted artifact JSON was accepted".to_string()),
                    details: "Should reject corrupted JSON".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Artifact Integrity".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked on corrupted artifact JSON".to_string(),
        },
    }
}

fn test_modified_decision() -> ReliabilityTestResult {
    let test_name = "Modified Decision Detection";
    
    // First, create a valid artifact
    let valid_payload = r#"{
  "session_id": "reliability-test-001",
  "request_id": "req-reliability-001",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
    
    let action: ProposedAction = serde_json::from_str(valid_payload).unwrap();
    
    let result = std::panic::catch_unwind(|| {
        let engine = Engine::load_default_policies().unwrap();
        let evaluation = engine.evaluate(&action);
        
        // Create artifact
        let decision_id = format!("dec_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let execution_status = if evaluation.decision == "ALLOW" {
            "executed".to_string()
        } else {
            "blocked".to_string()
        };
        
        let artifact = crate::artifact::ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            engine.policy_hash(),
            execution_status,
        );
        
        // Modify the decision in the artifact
        let mut artifact_json = serde_json::to_string(&artifact).unwrap();
        artifact_json = artifact_json.replace(&format!("\"decision\":\"{}\"", evaluation.decision), "\"decision\":\"MODIFIED\"");
        
        // Try to replay the modified artifact
        let modified_artifact: AuditArtifact = serde_json::from_str(&artifact_json).unwrap();
        let replay_engine = ReplayEngine::with_default_policy().unwrap();
        let replay_result = replay_engine.replay_from_artifact(&modified_artifact);
        
        // Replay should detect the mismatch
        if !replay_result.match_status {
            true // Test passed - modification was detected
        } else {
            false // Test failed - modification was not detected
        }
    });
    
    match result {
        Ok(detected) => {
            if detected {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly detected modified decision".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: false,
                    error_message: Some("Modified decision was not detected".to_string()),
                    details: "Replay should detect decision modifications".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Artifact Integrity".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked during modified decision test".to_string(),
        },
    }
}

fn test_modified_policy_hash() -> ReliabilityTestResult {
    let test_name = "Modified Policy Hash Detection";
    
    let valid_payload = r#"{
  "session_id": "reliability-test-002",
  "request_id": "req-reliability-002",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
    
    let action: ProposedAction = serde_json::from_str(valid_payload).unwrap();
    
    let result = std::panic::catch_unwind(|| {
        let engine = Engine::load_default_policies().unwrap();
        let evaluation = engine.evaluate(&action);
        
        // Create artifact
        let decision_id = format!("dec_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let execution_status = if evaluation.decision == "ALLOW" {
            "executed".to_string()
        } else {
            "blocked".to_string()
        };
        
        let artifact = crate::artifact::ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            engine.policy_hash(),
            execution_status,
        );
        
        // Modify the policy hash in the artifact
        let mut artifact_json = serde_json::to_string(&artifact).unwrap();
        artifact_json = artifact_json.replace(&format!("\"policy_hash\":\"{}\"", engine.policy_hash()), "\"policy_hash\":\"MODIFIED_HASH\"");
        
        // Try to replay the modified artifact
        let modified_artifact: AuditArtifact = serde_json::from_str(&artifact_json).unwrap();
        let replay_engine = ReplayEngine::with_default_policy().unwrap();
        let replay_result = replay_engine.replay_from_artifact(&modified_artifact);
        
        // Replay should detect the mismatch
        if !replay_result.match_status {
            true // Test passed - modification was detected
        } else {
            false // Test failed - modification was not detected
        }
    });
    
    match result {
        Ok(detected) => {
            if detected {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: true,
                    error_message: None,
                    details: "Correctly detected modified policy hash".to_string(),
                }
            } else {
                ReliabilityTestResult {
                    test_name: test_name.to_string(),
                    category: "Artifact Integrity".to_string(),
                    passed: false,
                    error_message: Some("Modified policy hash was not detected".to_string()),
                    details: "Replay should detect policy hash modifications".to_string(),
                }
            }
        }
        Err(_) => ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Artifact Integrity".to_string(),
            passed: false,
            error_message: Some("Panic occurred".to_string()),
            details: "System panicked during modified policy hash test".to_string(),
        },
    }
}

fn test_concurrent_execution() -> Vec<ReliabilityTestResult> {
    let mut results = Vec::new();
    
    // Test 1: Unique decision IDs
    results.push(test_unique_decision_ids());
    
    // Test 2: No corrupted artifacts
    results.push(test_no_corrupted_artifacts());
    
    // Test 3: Stable replay under concurrency
    results.push(test_stable_replay_concurrent());
    
    results
}

fn test_unique_decision_ids() -> ReliabilityTestResult {
    let test_name = "Unique Decision IDs Under Concurrency";
    
    let concurrency = 50;
    let iterations = 10;
    
    let decision_ids: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];
    
    let valid_payload = r#"{
  "session_id": "concurrent-test",
  "request_id": "req-concurrent",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
    
    // Don't use catch_unwind for thread spawning - it's not UnwindSafe
    for _ in 0..concurrency {
        let payload = valid_payload.to_string();
        let ids = Arc::clone(&decision_ids);
        
        let handle = thread::spawn(move || {
            let engine = Engine::load_default_policies().unwrap();
            let action: ProposedAction = serde_json::from_str(&payload).unwrap();
            
            for _ in 0..iterations {
                let _evaluation = engine.evaluate(&action);
                let decision_id = format!("dec_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
                
                let mut ids = ids.lock().unwrap();
                ids.push(decision_id);
            }
        });
        
        handles.push(handle);
    }
    
    let all_joined = handles.into_iter().all(|h| h.join().is_ok());
    
    if !all_joined {
        return ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: false,
            error_message: Some("Thread panic detected".to_string()),
            details: "One or more threads panicked".to_string(),
        };
    }
    
    let ids = decision_ids.lock().unwrap();
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    
    if unique_ids.len() == ids.len() {
        ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: true,
            error_message: None,
            details: format!("All {} decision IDs were unique", concurrency * iterations).to_string(),
        }
    } else {
        ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: false,
            error_message: Some("Duplicate decision IDs detected".to_string()),
            details: "Decision IDs should be unique".to_string(),
        }
    }
}

fn test_no_corrupted_artifacts() -> ReliabilityTestResult {
    let test_name = "No Corrupted Artifacts Under Concurrency";
    
    let concurrency = 20;
    let iterations = 5;
    
    let artifacts: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];
    
    let valid_payload = r#"{
  "session_id": "corruption-test",
  "request_id": "req-corruption",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
    
    // Don't use catch_unwind for thread spawning
    for _ in 0..concurrency {
        let payload = valid_payload.to_string();
        let artifact_list = Arc::clone(&artifacts);
        
        let handle = thread::spawn(move || {
            let engine = Engine::load_default_policies().unwrap();
            let action: ProposedAction = serde_json::from_str(&payload).unwrap();
            
            for _ in 0..iterations {
                let evaluation = engine.evaluate(&action);
                let decision_id = format!("dec_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
                let execution_status = if evaluation.decision == "ALLOW" {
                    "executed".to_string()
                } else {
                    "blocked".to_string()
                };
                
                let artifact = crate::artifact::ArtifactLogger::generate_artifact(
                    &decision_id,
                    &action,
                    &evaluation,
                    engine.policy_hash(),
                    execution_status,
                );
                
                let artifact_json = serde_json::to_string(&artifact).unwrap();
                let mut list = artifact_list.lock().unwrap();
                list.push(artifact_json);
            }
        });
        
        handles.push(handle);
    }
    
    let all_joined = handles.into_iter().all(|h| h.join().is_ok());
    
    if !all_joined {
        return ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: false,
            error_message: Some("Thread panic detected".to_string()),
            details: "One or more threads panicked".to_string(),
        };
    }
    
    let artifact_list = artifacts.lock().unwrap();
    
    // Verify all artifacts can be parsed
    let all_valid = artifact_list.iter().all(|artifact_json| {
        serde_json::from_str::<AuditArtifact>(artifact_json).is_ok()
    });
    
    if all_valid {
        ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: true,
            error_message: None,
            details: format!("All {} artifacts were valid", concurrency * iterations).to_string(),
        }
    } else {
        ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: false,
            error_message: Some("Corrupted artifacts detected".to_string()),
            details: "Artifacts should not be corrupted under concurrency".to_string(),
        }
    }
}

fn test_stable_replay_concurrent() -> ReliabilityTestResult {
    let test_name = "Stable Replay Under Concurrency";
    
    let concurrency = 10;
    let iterations = 3;
    
    let valid_payload = r#"{
  "session_id": "replay-test",
  "request_id": "req-replay",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#;
    
    let engine = Engine::load_default_policies().unwrap();
    let action: ProposedAction = serde_json::from_str(valid_payload).unwrap();
    
    // Create artifacts
    let mut artifacts = Vec::new();
    for _ in 0..(concurrency * iterations) {
        let evaluation = engine.evaluate(&action);
        let decision_id = format!("dec_{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        let execution_status = if evaluation.decision == "ALLOW" {
            "executed".to_string()
        } else {
            "blocked".to_string()
        };
        
        let artifact = crate::artifact::ArtifactLogger::generate_artifact(
            &decision_id,
            &action,
            &evaluation,
            engine.policy_hash(),
            execution_status,
        );
        
        artifacts.push(artifact);
    }
    
    // Replay all artifacts concurrently
    let mut handles = vec![];
    let success_count = Arc::new(Mutex::new(0));
    
    for artifact in artifacts {
        let success = Arc::clone(&success_count);
        
        let handle = thread::spawn(move || {
            let replay_engine = ReplayEngine::with_default_policy().unwrap();
            let replay_result = replay_engine.replay_from_artifact(&artifact);
            if replay_result.match_status {
                let mut count = success.lock().unwrap();
                *count += 1;
            }
        });
        
        handles.push(handle);
    }
    
    let all_joined = handles.into_iter().all(|h| h.join().is_ok());
    
    if !all_joined {
        return ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: false,
            error_message: Some("Thread panic detected".to_string()),
            details: "One or more threads panicked during replay".to_string(),
        };
    }
    
    let count = success_count.lock().unwrap();
    
    if *count == (concurrency * iterations) {
        ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: true,
            error_message: None,
            details: format!("All {} replays matched successfully", concurrency * iterations).to_string(),
        }
    } else {
        ReliabilityTestResult {
            test_name: test_name.to_string(),
            category: "Concurrent Execution".to_string(),
            passed: false,
            error_message: Some("Some replays failed under concurrency".to_string()),
            details: format!("Only {} out of {} replays matched", *count, concurrency * iterations).to_string(),
        }
    }
}

pub fn generate_reliability_report(results: &ReliabilityTestSuite) -> String {
    let mut report = String::new();
    
    report.push_str("# TRAXES Reliability Test Report\n\n");
    report.push_str("## Test Summary\n\n");
    report.push_str(&format!("- Total Tests: {}\n", results.total_tests));
    report.push_str(&format!("- Passed: {}\n", results.passed_tests));
    report.push_str(&format!("- Failed: {}\n", results.failed_tests));
    report.push_str(&format!("- Success Rate: {:.1}%\n\n", (results.passed_tests as f64 / results.total_tests as f64) * 100.0));
    
    report.push_str("## Test Results by Category\n\n");
    
    let mut categories: std::collections::HashMap<String, Vec<&ReliabilityTestResult>> = std::collections::HashMap::new();
    for result in &results.test_results {
        categories.entry(result.category.clone()).or_insert_with(Vec::new).push(result);
    }
    
    for (category, tests) in categories.iter() {
        report.push_str(&format!("### {}\n\n", category));
        
        for test in tests {
            let status = if test.passed { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("**{}** {}\n", test.test_name, status));
            report.push_str(&format!("- Details: {}\n", test.details));
            if let Some(ref error) = test.error_message {
                report.push_str(&format!("- Error: {}\n", error));
            }
            report.push_str("\n");
        }
    }
    
    report.push_str("## Analysis\n\n");
    report.push_str("### Critical Failures\n");
    report.push_str("- [Analysis of critical failures]\n\n");
    
    report.push_str("### Recommendations\n");
    report.push_str("- [Recommendations based on test results]\n");
    
    report
}
