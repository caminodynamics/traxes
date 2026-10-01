use std::fs;
use std::process;
use traxes_demo::{replay::ReplayEngine, Engine};

pub fn run(args: &[String]) {
    let id = args.first().unwrap_or_else(|| {
        println!("Usage: traxes replay <artifact_id> [--policy <policy.yaml>]");
        process::exit(1);
    });

    let artifact_path = format!("artifacts/AuditArtifact_{}.json", id);
    if !std::path::Path::new(&artifact_path).exists() {
        println!("Error: Artifact not found: {}", artifact_path);
        process::exit(1);
    }

    println!("TRAXES Replay Verification");
    println!();

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

    let original_decision = artifact
        .get("decision")
        .and_then(|v| v.as_str())
        .unwrap_or("UNKNOWN");
    let policy_hash = artifact
        .get("policy_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let policy_info = artifact.get("policy_info");
    let governance_info = artifact.get("governance_info");
    let rule_evaluation = artifact.get("rule_evaluation");

    println!("Original decision: {}", original_decision);
    println!("Policy hash: {}", policy_hash);

    if let Some(pi) = policy_info {
        let policy_id = pi.get("policy_id").and_then(|v| v.as_str()).unwrap_or("");
        let policy_version = pi
            .get("policy_version")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        println!("Policy ID: {}", policy_id);
        println!("Policy version: {}", policy_version);
    }

    if let Some(gi) = governance_info {
        let coverage_status = gi
            .get("coverage_status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let endpoint = gi.get("endpoint").and_then(|v| v.as_str()).unwrap_or("");
        println!("Governance status: {}", coverage_status);
        println!("Endpoint: {}", endpoint);
    }

    println!();
    println!("Re-evaluating policy...");
    println!();

    let replay_engine = if let Some(policy_idx) = args.iter().position(|arg| arg == "--policy") {
        let policy_path = args.get(policy_idx + 1).unwrap_or_else(|| {
            println!("Error: --policy requires a policy file path");
            process::exit(1);
        });
        let policy_yaml = fs::read_to_string(policy_path).unwrap_or_else(|e| {
            println!("Error reading policy '{}': {}", policy_path, e);
            process::exit(1);
        });
        ReplayEngine::new(Engine::with_policy(policy_yaml))
    } else {
        ReplayEngine::with_default_policy().expect("Failed to initialize replay engine")
    };

    match replay_engine.replay_from_file(&artifact_path) {
        Ok(replay_result) => {
            println!("Replay decision: {}", replay_result.replay_decision);
            println!();
            println!("VERIFICATION RESULT");
            println!();

            let decision_match = replay_result.original_decision == replay_result.replay_decision;
            let policy_match = replay_engine.verify_policy_consistency(
                &serde_json::from_str(&artifact_content).expect("artifact already parsed"),
            );

            if decision_match && policy_match && replay_result.match_status {
                println!("✓ Decision matches");
                println!("✓ Policy hash matches");

                if governance_info.is_some() {
                    println!("✓ Governance status");
                }
                if rule_evaluation.is_some() {
                    println!("✓ Rule evaluation");
                }

                println!();
                println!("==============================");
                println!("REPLAY VERIFIED");
                println!("==============================");
                println!();
                println!("Replay verified successfully.");
                println!();
                println!("The original decision was reproduced using the same policy version.");
                println!();
                process::exit(0);
            }

            println!("REPLAY MISMATCH");
            println!();
            if !decision_match {
                println!(
                    "✗ Decision mismatch: original={}, replay={}",
                    replay_result.original_decision, replay_result.replay_decision
                );
            } else {
                println!(
                    "✓ Decision matches: {} == {}",
                    replay_result.original_decision, replay_result.replay_decision
                );
            }
            if !policy_match {
                println!("✗ Policy hash mismatch");
            } else {
                println!("✓ Policy hash matches: {}", policy_hash);
            }
            process::exit(1);
        }
        Err(e) => {
            println!("Error initializing replay engine: {}", e);
            process::exit(1);
        }
    }
}
