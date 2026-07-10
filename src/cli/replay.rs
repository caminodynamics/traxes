use traxes_demo::replay::ReplayEngine;
use std::fs;
use std::process;

pub fn run(args: &[String]) {
    let id = args.get(0);

    if id.is_none() {
        println!("Usage: traxes replay <artifact_id>");
        process::exit(1);
    }

    let id = id.unwrap();
    
    // Construct artifact path
    let artifact_path = format!("artifacts/AuditArtifact_{}.json", id);
    
    // Check if artifact exists
    if !std::path::Path::new(&artifact_path).exists() {
        println!("Error: Artifact not found: {}", artifact_path);
        process::exit(1);
    }
    
    println!("TRAXES Replay Verification");
    println!();
    println!("Loading artifact: {}", id);
    
    // Load artifact
    let artifact_content = match fs::read_to_string(&artifact_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Error reading artifact: {}", e);
            process::exit(1);
        }
    };
    
    let artifact: serde_json::Value = match serde_json::from_str(&artifact_content) {
        Ok(value) => value,
        Err(e) => {
            println!("Error parsing artifact: {}", e);
            process::exit(1);
        }
    };
    
    // Extract key fields for comparison
    let original_decision = artifact.get("decision").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
    let policy_hash = artifact.get("policy_hash").and_then(|v| v.as_str()).unwrap_or("");
    let policy_info = artifact.get("policy_info");
    let governance_info = artifact.get("governance_info");
    let rule_evaluation = artifact.get("rule_evaluation");
    
    println!("Original decision: {}", original_decision);
    println!("Policy hash: {}", policy_hash);
    
    if let Some(pi) = policy_info {
        let policy_id = pi.get("policy_id").and_then(|v| v.as_str()).unwrap_or("");
        let policy_version = pi.get("policy_version").and_then(|v| v.as_str()).unwrap_or("");
        println!("Policy ID: {}", policy_id);
        println!("Policy version: {}", policy_version);
    }
    
    if let Some(gi) = governance_info {
        let coverage_status = gi.get("coverage_status").and_then(|v| v.as_str()).unwrap_or("");
        let endpoint = gi.get("endpoint").and_then(|v| v.as_str()).unwrap_or("");
        println!("Governance status: {}", coverage_status);
        println!("Endpoint: {}", endpoint);
    }
    
    println!();
    println!("Re-evaluating policy...");
    
    // Create replay engine and replay
    match ReplayEngine::with_default_policy() {
        Ok(replay_engine) => {
            let replay_result: traxes_demo::replay::ReplayResult = match replay_engine.replay_from_file(&artifact_path) {
                Ok(result) => result,
                Err(e) => {
                    println!("Error during replay: {}", e);
                    process::exit(1);
                }
            };
            
            println!("Replay decision: {}", replay_result.replay_decision);
            
            // Compare results
            println!();
            println!("VERIFICATION RESULT:");
            
            let decision_match = replay_result.original_decision == replay_result.replay_decision;
            let policy_match = replay_result.policy_hash == policy_hash;
            
            if decision_match && policy_match {
                println!("REPLAY MATCH");
                println!();
                println!("✓ Decision matches: {} == {}", 
                    replay_result.original_decision, replay_result.replay_decision);
                println!("✓ Policy hash matches: {}", replay_result.policy_hash);
                
                if let Some(gi) = governance_info {
                    let coverage_status = gi.get("coverage_status").and_then(|v| v.as_str()).unwrap_or("");
                    println!("✓ Governance status: {}", coverage_status);
                }
                
                if let Some(re) = rule_evaluation {
                    let rule_id = re.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
                    let evaluation_result = re.get("evaluation_result").and_then(|v| v.as_bool()).unwrap_or(false);
                    println!("✓ Rule evaluation: {} ({})", rule_id, evaluation_result);
                }
                
                process::exit(0);
            } else {
                println!("REPLAY MISMATCH");
                println!();
                
                if !decision_match {
                    println!("✗ Decision mismatch: original={}, replay={}", 
                        replay_result.original_decision, replay_result.replay_decision);
                } else {
                    println!("✓ Decision matches: {} == {}", 
                        replay_result.original_decision, replay_result.replay_decision);
                }
                
                if !policy_match {
                    println!("✗ Policy hash mismatch: original={}, replay={}", 
                        policy_hash, replay_result.policy_hash);
                } else {
                    println!("✓ Policy hash matches: {}", replay_result.policy_hash);
                }
                
                process::exit(1);
            }
        }
        Err(e) => {
            println!("Error initializing replay engine: {}", e);
            process::exit(1);
        }
    }
}
