// TRAXES Replay Verification Example
//
// This example demonstrates the complete replay verification workflow:
// 1. Create a decision
// 2. Generate and store an audit artifact
// 3. Replay the artifact later
// 4. Verify the replay matches the original decision

use traxes_demo::action::ProposedAction;
use traxes_demo::artifact::ArtifactLogger;
use traxes_demo::replay::ReplayEngine;
use traxes_demo::traxes_engine::Engine;
use serde_json::json;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TRAXES Replay Verification Example ===\n");

    // Step 1: Create a decision
    println!("Step 1: Creating a decision...");
    let engine = Engine::load_default_policies()?;
    
    let action = ProposedAction {
        tool: "aws_ec2_provision".to_string(),
        session_id: "session-abc123".to_string(),
        environment: "staging".to_string(),
        parameters: json!({
            "instance_type": "t3.medium",
            "instance_cost_per_hour": 1.50
        }),
    };

    let evaluation = engine.evaluate(&action);
    println!("  Decision: {}", evaluation.decision);
    println!("  Policy Hash: {}", engine.policy_hash());

    // Step 2: Generate and store audit artifact
    println!("\nStep 2: Generating audit artifact...");
    let decision_id = uuid::Uuid::new_v4().to_string();
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        "executed".to_string(),
    );

    let artifact_path = artifact.write_to_file_sync()?;
    println!("  Artifact saved to: {}", artifact_path);

    // Step 3: Replay the artifact (simulating later verification)
    println!("\nStep 3: Replaying artifact for verification...");
    let replay_engine = ReplayEngine::new(engine);
    
    // Load the artifact from file
    let verification_report = replay_engine.verify_artifact(&artifact_path)?;
    
    // Step 4: Display verification result
    println!("\nStep 4: Verification Result");
    println!("{}", verification_report.display());

    // Cleanup
    fs::remove_file(&artifact_path)?;
    println!("\nCleanup: Removed artifact file");

    Ok(())
}
