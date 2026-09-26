// Minimal FILE_WRITE integration proof for TRAXES.
//
// A coding-agent-style proposed action is routed through the TRAXES engine
// BEFORE any filesystem write happens. ALLOW -> real write. DENY -> no write.
// Both decisions are persisted as audit artifacts and replayed for match.

use serde_json::json;
use std::fs; // Used in main() for setup/cleanup
use std::path::PathBuf;
use traxes_demo::action::{self, ProposedAction};
use traxes_demo::artifact::ArtifactLogger;
use traxes_demo::replay::ReplayEngine;
use traxes_demo::traxes_engine::Engine;
use uuid::Uuid;

const FILE_WRITE_POLICY_TEMPLATE: &str = r#"apiVersion: Traxes.dev/v1
kind: ExecutionPolicy
metadata:
  name: filesystem-write-allowlist
target:
  tool: FILE_WRITE
rules:
  - name: file_write_path_allowlist
    condition: payload.proposed_action.parameters.path not in ["__ALLOWED_PATH__"]
    action: DENY
    reason: "File write path outside allowlist for sandbox."
"#;

const ALLOW_FILE_CONTENTS: &str = "traxes file_write_integration proof payload v1";

struct RunOutcome {
    decision: String,
    artifact_path: String,
}

fn normalize_path(p: &PathBuf) -> String {
    p.to_string_lossy().replace('\\', "/")
}

fn propose_and_execute(engine: &Engine, target: &PathBuf, label: &str) -> RunOutcome {
    let target_str = normalize_path(target);
    let action = ProposedAction {
        tool: "FILE_WRITE".to_string(),
        session_id: format!("file-write-demo-{}", label),
        environment: "sandbox".to_string(),
        // Include both path and content in parameters for the execute() boundary
        parameters: json!({
            "path": target_str,
            "content": ALLOW_FILE_CONTENTS
        }),
    };

    // TRAXES evaluates BEFORE any filesystem side-effect.
    let evaluation = engine.evaluate(&action);
    let decision_id = Uuid::new_v4().to_string();

    // Execute through the central governance boundary.
    // This handles the actual FILE_WRITE based on the decision.
    let execution_status = action::execute(&action, &evaluation.decision, &decision_id);

    // Generate artifact AFTER execute() so execution_status reflects what actually happened.
    let artifact = ArtifactLogger::generate_artifact(
        &decision_id,
        &action,
        &evaluation,
        engine.policy_hash(),
        execution_status,
    );
    let artifact_path = artifact
        .write_to_file_sync()
        .expect("artifact write must succeed");

    println!(
        "  [{}] decision={}  path={}  artifact={}",
        label,
        evaluation.decision,
        target.display(),
        artifact_path
    );

    RunOutcome {
        decision: evaluation.decision,
        artifact_path,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== TRAXES FILE_WRITE integration proof ===");

    let sandbox = std::env::temp_dir().join("traxes_file_write_demo");
    fs::create_dir_all(&sandbox)?;
    let allow_path = sandbox.join("allowed_write.txt");
    let deny_path = sandbox.join("forbidden_write.txt");

    // Pre-clean so no prior run can produce a false positive.
    let _ = fs::remove_file(&allow_path);
    let _ = fs::remove_file(&deny_path);
    assert!(!allow_path.exists(), "pre-clean: ALLOW path must not exist");
    assert!(!deny_path.exists(), "pre-clean: DENY path must not exist");

    println!("sandbox   : {}", sandbox.display());
    println!("allow_path: {}", allow_path.display());
    println!("deny_path : {}", deny_path.display());

    // Explicit-policy path per task: Engine::with_policy with FILE_WRITE rule.
    let policy_yaml =
        FILE_WRITE_POLICY_TEMPLATE.replace("__ALLOWED_PATH__", &normalize_path(&allow_path));
    let engine = Engine::with_policy(policy_yaml.clone());
    println!("policy_hash: {}", engine.policy_hash());

    println!("\n[1/2] ALLOW case (path is in allowlist)");
    let allow = propose_and_execute(&engine, &allow_path, "ALLOW");
    assert_eq!(
        allow.decision, "ALLOW",
        "ALLOW case: decision must be ALLOW"
    );
    assert!(
        allow_path.exists(),
        "ALLOW case: file MUST exist at {}",
        allow_path.display()
    );
    let allow_contents = fs::read_to_string(&allow_path)?;
    assert_eq!(
        allow_contents, ALLOW_FILE_CONTENTS,
        "ALLOW case: file contents MUST match expected payload"
    );
    println!(
        "  ALLOW proof: exists={}  contents_match={}",
        allow_path.exists(),
        allow_contents == ALLOW_FILE_CONTENTS
    );

    println!("\n[2/2] DENY case (path is NOT in allowlist)");
    let deny = propose_and_execute(&engine, &deny_path, "DENY");
    assert_eq!(deny.decision, "DENY", "DENY case: decision must be DENY");
    assert!(
        !deny_path.exists(),
        "DENY case: file MUST NOT exist at {}",
        deny_path.display()
    );
    println!(
        "  DENY proof:  exists={} (expected false)",
        deny_path.exists()
    );

    // Replay both artifacts against a fresh engine built from the same policy.
    println!("\n[replay] rebuilding engine from same policy and replaying artifacts");
    let replay_engine = ReplayEngine::new(Engine::with_policy(policy_yaml));

    let allow_replay = replay_engine.replay_from_file(&allow.artifact_path)?;
    println!(
        "  ALLOW replay: original={}  replay={}  match={}",
        allow_replay.original_decision, allow_replay.replay_decision, allow_replay.match_status
    );
    assert!(
        allow_replay.match_status,
        "ALLOW replay: match_status must be true"
    );
    assert_eq!(
        allow_replay.original_decision, allow_replay.replay_decision,
        "ALLOW replay: original and replay decisions must match"
    );

    let deny_replay = replay_engine.replay_from_file(&deny.artifact_path)?;
    println!(
        "  DENY  replay: original={}  replay={}  match={}",
        deny_replay.original_decision, deny_replay.replay_decision, deny_replay.match_status
    );
    assert!(
        deny_replay.match_status,
        "DENY replay: match_status must be true"
    );
    assert_eq!(
        deny_replay.original_decision, deny_replay.replay_decision,
        "DENY replay: original and replay decisions must match"
    );

    println!("\nOK - all assertions passed");
    Ok(())
}
