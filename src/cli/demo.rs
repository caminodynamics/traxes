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

pub async fn run_async() {
    // Enable demo mode to suppress debug logs
    crate::cli_utils::set_demo_mode(true);

    // Print Traxes banner
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║                        TRAXES                                   ║");
    println!("║                   Policy Engine Demo                           ║");
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Print intro text
    println!("This is Traxes.");
    println!("It evaluates actions before execution.");
    println!();

    // Pause 1500ms
    pause_if_not_fast(1500);

    // SCENARIO 1
    println!("---");
    println!();
    println!("SCENARIO 1");
    println!();

    // Show proposed action
    print_proposed_action(assets::ALLOW_PAYLOAD);

    // Pause 2000ms
    pause_if_not_fast(2000);

    // Print evaluating policy message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("Evaluating policy...");
    }

    // Pause 1000ms
    pause_if_not_fast(1000);

    let allow_decision = crate::cli::eval::run_async_silent(&["allow".to_string()]).await;

    match &allow_decision {
        Ok(decision) => {
            if decision == "ALLOW" {
                println!("{}", decision.green());
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

    // Pause 1500ms
    pause_if_not_fast(1500);

    // Print writing artifact message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("writing artifact...");
    }

    // Pause 1000ms
    pause_if_not_fast(1000);

    // Display compact artifact summary
    println!();
    println!("ARTIFACT SUMMARY");
    crate::cli::artifacts::last_demo_summary();
    println!();

    // Pause 3000ms
    pause_if_not_fast(3000);

    // SCENARIO 2
    println!("---");
    println!();
    println!("SCENARIO 2");
    println!();

    // Show proposed action
    print_proposed_action(assets::DENY_PAYLOAD);

    // Pause 2000ms
    pause_if_not_fast(2000);

    // Print evaluating policy message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("Evaluating policy...");
    }

    // Pause 1000ms
    pause_if_not_fast(1000);

    let deny_decision = crate::cli::eval::run_async_silent(&["deny".to_string()]).await;

    match &deny_decision {
        Ok(decision) => {
            if decision == "DENY" {
                println!("{}", decision.red());
                println!();
                println!("reason:");
                println!("instance_type is not allowed for this environment");
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

    // Pause 1500ms
    pause_if_not_fast(1500);

    // Print writing artifact message (skip in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("writing artifact...");
    }

    // Pause 1000ms
    pause_if_not_fast(1000);

    // Display compact artifact summary
    println!();
    println!("ARTIFACT SUMMARY");
    crate::cli::artifacts::last_demo_summary();
    println!();

    // Pause 3000ms
    pause_if_not_fast(3000);

    // Inspect generated artifact (skip message in fast mode)
    if !crate::cli_utils::is_demo_fast() {
        println!("Inspecting generated artifact...");
    }

    // Pause 1500ms
    pause_if_not_fast(1500);

    // Display compact artifact inspection
    println!();
    crate::cli::artifacts::last_demo_inspection();
    println!();
}

#[allow(dead_code)]
pub fn run() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run_async());
}
