use std::thread;
use std::time::Duration;
use colored::Colorize;
use traxes_demo::assets;

fn print_proposed_action(payload_content: &str) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload_content) {
        let tool = json.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
        let environment = json.get("environment").and_then(|e| e.as_str()).unwrap_or("unknown");
        let parameters = json.get("parameters").and_then(|p| p.as_object());
        let instance_type = parameters
            .and_then(|p| p.get("instance_type"))
            .and_then(|i| i.as_str())
            .unwrap_or("unknown");

        println!("Proposed Action");
        println!();
        println!("Create AWS RDS database");
        println!();
        println!("tool: {}", tool);
        println!("environment: {}", environment);
        println!("instance_type: {}", instance_type);
        println!();
    }
}

fn pause_if_not_fast(ms: u64) {
    if !crate::cli_utils::is_demo_fast() {
        thread::sleep(Duration::from_millis(ms));
    }
}

fn get_latest_audit_artifact_id() -> Option<String> {
    use std::path::Path;
    let artifacts_dir = Path::new("artifacts");
    if !artifacts_dir.exists() {
        return None;
    }

    let mut json_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = match std::fs::read_dir(artifacts_dir) {
        Ok(rd) => rd.filter_map(|e| e.ok())
            .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "json"))
            .filter_map(|entry| {
                let path = entry.path();
                let metadata = path.metadata().ok()?;
                let modified = metadata.modified().ok()?;
                Some((path, modified))
            })
            .collect(),
        Err(_) => return None,
    };

    if json_files.is_empty() {
        return None;
    }

    json_files.sort_by(|a, b| b.1.cmp(&a.1));
    let (latest_path, _) = &json_files[0];
    if let Some(fname) = latest_path.file_name().and_then(|n| n.to_str()) {
        // Expected filename format: AuditArtifact_<id>.json
        if let Some(rest) = fname.strip_prefix("AuditArtifact_") {
            if let Some(id) = rest.strip_suffix(".json") {
                return Some(id.to_string());
            }
        }
    }
    None
}

pub async fn run_async() {
    // Enable demo mode to suppress debug logs
    crate::cli_utils::set_demo_mode(true);

    // Print Traxes banner
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║                        TRAXES                                   ║");
    println!("║    Deterministic Pre-Execution Decision Engine Demo            ║");
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Print intro text (concise)
    println!("This is TRAXES.");
    println!("TRAXES — a Deterministic Pre-Execution Decision Engine that evaluates proposed actions and produces replayable decision artifacts.");
    println!();

    // Pause 1800ms
    pause_if_not_fast(1800);

    // SCENARIO 1
    println!("------------------------");
    println!();
    println!("SCENARIO 1");
    println!();

    // Show proposed action
    print_proposed_action(assets::ALLOW_PAYLOAD);

    // Pause 1800ms
    pause_if_not_fast(1800);

    // Print evaluating policy message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("Evaluating policy...");
    }

    // Pause 1800ms
    pause_if_not_fast(1800);

    let allow_decision = crate::cli::eval::run_async_silent(&["allow".to_string()]).await;

    match &allow_decision {
        Ok(decision) => {
            if decision == "ALLOW" {
                println!("{}", decision.green());
                println!();
                println!("Action may proceed. Artifact recorded.");
            } else {
                println!("{}", decision.red());
            }
        }
        Err(e) => {
            eprintln!("ERROR: {:?}", e);
            std::process::exit(1);
        }
    }

    println!();

    // Pause 1800ms
    pause_if_not_fast(1800);

    // Print writing artifact message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("writing artifact...");
    }

    // Pause 1500ms
    pause_if_not_fast(1500);

    // Display compact artifact summary
    println!();
    println!("ARTIFACT SUMMARY");
    crate::cli::artifacts::last_demo_summary();
    println!();

    // Capture ALLOW artifact id for later replay (if available)
    let allow_artifact_id = get_latest_audit_artifact_id();

    // Pause 1800ms
    pause_if_not_fast(1800);

    // SCENARIO 2
    println!("------------------------");
    println!();
    println!("SCENARIO 2");
    println!();

    // Show proposed action
    print_proposed_action(assets::DENY_PAYLOAD);

    // Pause 1800ms
    pause_if_not_fast(1800);

    // Print evaluating policy message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("Evaluating policy...");
    }

    // Pause 1800ms
    pause_if_not_fast(1800);

    let deny_decision = crate::cli::eval::run_async_silent(&["deny".to_string()]).await;

    match &deny_decision {
        Ok(decision) => {
            if decision == "DENY" {
                println!("{}", decision.red());
                println!();
                println!("reason:");
                println!("instance_type is not allowed for this environment");
                println!("Action blocked. Artifact recorded.");
            } else {
                println!("{}", decision.red());
            }
        }
        Err(e) => {
            eprintln!("ERROR: {:?}", e);
            std::process::exit(1);
        }
    }

    println!();

    // Pause 1800ms
    pause_if_not_fast(1800);

    // Print writing artifact message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("writing artifact...");
    }

    // Pause 1500ms
    pause_if_not_fast(1500);

    // Display compact artifact summary
    println!();
    println!("ARTIFACT SUMMARY");
    crate::cli::artifacts::last_demo_summary();
    println!();

    // Pause 1800ms
    pause_if_not_fast(1800);

    // Inspect generated artifact (skip message in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("Inspecting generated artifact...");
    }

    // Pause 1800ms
    pause_if_not_fast(1800);

    // Display compact artifact inspection
    println!();
    crate::cli::artifacts::last_demo_inspection();
    println!();

    // Pause 2000ms
    pause_if_not_fast(2000);

    // Replay the ALLOW artifact if we captured it earlier
    if let Some(id) = allow_artifact_id {
        println!();
        println!("------------------------");
        println!();
        println!("Replay Verification");
        println!();
        if !crate::cli_utils::is_demo_fast() {
            println!("Replaying artifact: {}", id);
        } else {
            println!("Replaying artifact (fast): {}", id);
        }
        println!();

        // Call replay runner with the captured id
        crate::cli::replay::run(&[id.clone()]);

        // Pause 2200ms
        pause_if_not_fast(2200);

        // Affirmation messages after deterministic replay
        println!();

        // Pause 2000ms before final summary
        pause_if_not_fast(2000);

        // Final concise summary
        println!("Demo complete. Two decisions evaluated. Artifacts generated. Replay verified.");
    } else {
        println!("SUMMARY");
        println!("✓ Evaluated 2 decisions: ALLOW and DENY");
        println!("✓ Generated replayable artifacts for both decisions");
        println!("Note: Could not locate ALLOW artifact for replay.");
        println!("Demo complete.");
    }
}

#[allow(dead_code)]
pub fn run() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run_async());
}
