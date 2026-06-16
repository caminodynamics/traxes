// --- CLAP IMPORTS ---
use clap::Parser;

// --- CLAP CLI STRUCT ---
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(long)]
    policy: String,
    #[arg(long)]
    request: String,
}
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::post,
    Router,
};
use chrono::Utc;
use serde::Serialize;
use serde_json;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use uuid::Uuid;

fn write_artifact_on_exit<T: Serialize>(context: &T) {
    let json_result = serde_json::to_string_pretty(context);
    match json_result {
        Ok(json_content) => {
            if let Err(e) = std::fs::create_dir_all("logs") {
                cli_utils::debug_log(format!("[Traxes] Failed to create logs directory: {}", e));
            }
            if let Err(e) = std::fs::write("logs/artifact_latest.json", json_content) {
                cli_utils::debug_log(format!("[Traxes] Failed to write artifact_latest.json: {}", e));
            }
        }
        Err(e) => {
            cli_utils::debug_log(format!("[Traxes] Failed to serialize context: {}", e));
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorContext {
    error_type: String,
    message: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct TimingMetadata {
    parse_time_us: f64,
    eval_time_us: f64,
    hash_generation_time_us: f64,
    artifact_io_time_us: f64,
}

mod action;
mod artifact;
mod artifact_emitter;
mod artifact_index;
mod cli;
mod cli_utils;
mod evaluate;
mod execution_event;
mod policy_bundle;
mod traxes_engine;
mod server_policy;

mod cli_layer;

use action::ProposedAction;
use artifact_emitter::{ArtifactEmitter, EventEmitter};
use traxes_engine::Engine;

fn show_latest_artifact(raw_mode: bool) -> Result<(), Box<dyn std::error::Error>> {
    let artifacts_dir = std::path::Path::new("artifacts");
    
    // Check if artifacts directory exists
    if !artifacts_dir.exists() {
        println!("No artifacts found");
        return Ok(());
    }
    
    // Scan directory for JSON files and sort by modification time
    let mut json_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = std::fs::read_dir(artifacts_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "json"))
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = path.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((path, modified))
        })
        .collect();
    
    if json_files.is_empty() {
        println!("No artifacts found");
        return Ok(());
    }
    
    // Sort by modification time (descending) to get most recent
    json_files.sort_by(|a, b| b.1.cmp(&a.1));
    
    let (latest_path, _) = &json_files[0];
    
    // Read file contents
    let content = std::fs::read_to_string(latest_path)?;
    
    if raw_mode {
        // Raw mode: print full JSON exactly as stored
        println!("{}", content);
    } else {
        // Human-readable mode: parse and format
        let artifact: serde_json::Value = serde_json::from_str(&content)?;
        
        let decision = artifact.get("decision").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
        let reason = artifact.get("reason").and_then(|v| v.as_str()).unwrap_or("");
        let tool = artifact.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown");
        let environment = artifact.get("environment").and_then(|v| v.as_str()).unwrap_or("unknown");
        let trace_id = artifact.get("execution_context")
            .and_then(|ctx| ctx.get("trace_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let artifact_path = latest_path.display();
        
        println!("LATEST ARTIFACT");
        println!("──────────────────────────────");
        println!("decision: {}", decision);
        if !reason.is_empty() {
            println!("reason: {}", reason);
        }
        println!("tool: {}", tool);
        println!("environment: {}", environment);
        println!("trace_id: {}", trace_id);
        println!("artifact_path: {}", artifact_path);
        println!("──────────────────────────────");
    }
    
    Ok(())
}

#[derive(Debug, Serialize)]
struct DecisionResponse {
    decision: String,
    decision_id: String,
    artifact_path: String,
    timing: TimingMetadata,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Clone)]
struct AppState {
    event_emitter: EventEmitter,
    engine: Arc<Engine>,
}


#[tokio::main]
async fn main() {
    let raw_args: Vec<String> = env::args().collect();
    let explicit_demo_off = raw_args.iter().any(|a| a == "--demo-mode=false");
    let demo_fast = raw_args.iter().any(|a| a == "--demo-fast");
    let (mut demo_mode, debug_mode, args): (bool, bool, Vec<String>) = cli_utils::parse_demo_flags(raw_args);

    if args.len() >= 2 && args[1] == "evaluate" && !explicit_demo_off {
        demo_mode = true;
    }
    cli_utils::set_demo_mode(demo_mode);
    cli_utils::set_debug_mode(debug_mode);
    cli_utils::set_demo_fast(demo_fast);

    if args.len() < 2 {
        eprintln!("Usage: Traxes-demo <command> [--demo-mode] [--debug]");
        eprintln!("Commands:");
        eprintln!("  server    - Run the HTTP server (default)");
        eprintln!("  evaluate  <payload-file> - Evaluate a payload file and exit");
        eprintln!("  benchmark --iterations <N> --concurrency <N> --payload <file> - Run benchmark mode");
        eprintln!("  show-latest-artifact [--raw] - Display the most recent artifact");
        eprintln!("  eval      <allow|deny|file> - Quick evaluation (new CLI)");
        eprintln!("  artifacts <list|last|show> - Artifact management (new CLI)");
        eprintln!("  replay    <id> - Replay an artifact (new CLI)");
        eprintln!("  status    - Show engine status (new CLI)");
        eprintln!("Flags:");
        eprintln!("  --demo-mode       Compact investor/demo terminal output (default for evaluate)");
        eprintln!("  --demo-mode=false Legacy verbose output");
        eprintln!("  --debug           Show payload expressions and policy debug logs");
        let error_ctx = ErrorContext {
            error_type: "USAGE_ERROR".to_string(),
            message: "No command provided".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };
        write_artifact_on_exit(&error_ctx);
        std::process::exit(1);
    }

    match args[1].as_str() {
        "eval" => {
            cli_layer::run().await;
            return;
        }
        "demo" => {
            cli::demo::run_async().await;
            return;
        }
        "artifacts" => {
            let subcommand = args.get(2).map(|s| s.as_str()).unwrap_or("list");
            match subcommand {
                "last" => {
                    let full = args.get(3).map(|s| s.as_str()) == Some("--full");
                    cli::artifacts::last(full);
                    return;
                }
                "list" => {
                    cli::artifacts::list();
                    return;
                }
                "show" => {
                    cli::artifacts::show(args.get(3));
                    return;
                }
                _ => {
                    cli::artifacts::list();
                    return;
                }
            }
        }
        "replay" => {
            cli_layer::run().await;
            return;
        }
        "status" => {
            cli_layer::run().await;
            return;
        }
        "server" => {
            // Server mode: initialize all server infrastructure here only
            // Initialize event emitter with bounded queue (capacity: 1000 events)
            let (event_emitter, event_rx) = artifact_emitter::create_event_channel(1000);
            
            // Spawn artifact emitter worker
            let policy_hash = Engine::load_default_policies()
                .expect("Failed to load default policy bundle")
                .policy_hash()
                .to_string();
            
            let artifact_emitter = ArtifactEmitter::new(event_rx, policy_hash);
            tokio::spawn(artifact_emitter.run());

            if !cli_utils::is_demo_mode() {
                println!("[Traxes] Event emitter initialized with capacity: 1000");
                println!("[Traxes] Artifact emitter worker spawned");
            }

            let engine = Arc::new(
                Engine::load_default_policies().expect("Failed to load default policy bundle"),
            );
            let state = AppState {
                event_emitter,
                engine,
            };

            let app = Router::new()
                .route("/evaluate", post(evaluate_action))
                .with_state(state)
                .route("/", axum::routing::get(|| async { "Traxes Engine v0.3.2" }));

            let port = env::var("PORT").unwrap_or_else(|_| "8082".to_string());
            let bind_address = format!("0.0.0.0:{}", port);
            let listener = tokio::net::TcpListener::bind(&bind_address)
                .await
                .expect("Failed to bind to address");
            if cli_utils::is_demo_mode() {
                println!("Traxes demo server → http://{}/evaluate  (--demo-mode)", bind_address);
            } else {
                println!("Traxes Engine listening on http://{}", bind_address);
            }
            axum::serve(listener, app)
                .await
                .expect("Failed to start server");
        }
        "evaluate" => {
            let payload_path = if args.len() >= 3 && args[2] != "--payload" && args[2] != "--demo-mode" && args[2] != "--demo-mode=false" && args[2] != "--debug" {
                args[2].clone()
            } else if args.len() >= 4 {
                // Handle --payload flag format
                let payload_idx = args.iter().position(|x| x == "--payload").unwrap_or(0);
                if payload_idx > 0 && payload_idx + 1 < args.len() {
                    args[payload_idx + 1].clone()
                } else {
                    let error_ctx = ErrorContext {
                        error_type: "USAGE_ERROR".to_string(),
                        message: "No payload file provided for evaluate command".to_string(),
                        timestamp: Utc::now().to_rfc3339(),
                    };
                    write_artifact_on_exit(&error_ctx);
                    std::process::exit(1);
                }
            } else {
                let error_ctx = ErrorContext {
                    error_type: "USAGE_ERROR".to_string(),
                    message: "No payload file provided for evaluate command".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                };
                write_artifact_on_exit(&error_ctx);
                std::process::exit(1);
            };
            
            // Initialize event emitter for CLI evaluate mode
            let (event_emitter, event_rx) = artifact_emitter::create_event_channel(100);
            
            // Spawn artifact emitter worker
            let engine = Engine::load_default_policies().expect("Failed to load default policy bundle");
            let policy_hash = engine.policy_hash().to_string();
            
            let artifact_emitter = ArtifactEmitter::new(event_rx, policy_hash);
            tokio::spawn(artifact_emitter.run());
            
            // Run evaluation with event emitter
            match evaluate::run_evaluate_with_emitter(payload_path, event_emitter, &engine).await {
                Ok(result) => {
                    // Print result as JSON to stdout
                    println!("{}", serde_json::to_string(&result).unwrap_or_else(|e| {
                        eprintln!("error: Failed to serialize result: {:?}", e);
                        std::process::exit(1);
                    }));
                    std::process::exit(0);
                }
                Err(e) => {
                    // System error: file not found, JSON parse failure, engine panic
                    let error_message = if e.to_string().contains("cannot find the file") {
                        "File not found".to_string()
                    } else if e.to_string().contains("parse") {
                        "Invalid payload format".to_string()
                    } else {
                        "System error".to_string()
                    };
                    eprintln!("error: {}", error_message);
                    eprintln!("details: {:?}", e);
                    let error_ctx = ErrorContext {
                        error_type: "SYSTEM_ERROR".to_string(),
                        message: error_message,
                        timestamp: Utc::now().to_rfc3339(),
                    };
                    write_artifact_on_exit(&error_ctx);
                    std::process::exit(1);
                }
            }
        }
        "benchmark" => {
            match evaluate::run_benchmark(&args) {
                Ok(_) => {
                    println!("Benchmark completed successfully");
                }
                Err(e) => {
                    let error_message = if e.to_string().contains("cannot find the file") {
                        "File not found".to_string()
                    } else if e.to_string().contains("parse") {
                        "Invalid payload format".to_string()
                    } else {
                        "System error".to_string()
                    };
                    eprintln!("error: {}", error_message);
                    eprintln!("details: {:?}", e);
                    let error_ctx = ErrorContext {
                        error_type: "SYSTEM_ERROR".to_string(),
                        message: error_message,
                        timestamp: Utc::now().to_rfc3339(),
                    };
                    write_artifact_on_exit(&error_ctx);
                    std::process::exit(1);
                }
            }
        }
        "show-latest-artifact" => {
            let raw_mode = args.iter().any(|a| a == "--raw");
            match show_latest_artifact(raw_mode) {
                Ok(_) => std::process::exit(0),
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            let error_ctx = ErrorContext {
                error_type: "UNKNOWN_COMMAND".to_string(),
                message: format!("Unknown command: {}", args[1]),
                timestamp: Utc::now().to_rfc3339(),
            };
            write_artifact_on_exit(&error_ctx);
            std::process::exit(1);
        }
    }
}

async fn evaluate_action(
    State(state): State<AppState>,
    Json(action): Json<ProposedAction>,
) -> Result<ResponseJson<DecisionResponse>, (StatusCode, ResponseJson<ErrorResponse>)> {
    let parse_start = Instant::now();
    let parse_time_us = parse_start.elapsed().as_micros() as f64;

    // Generate cryptographically unique IDs per request at the edge of the handler
    let trace_id = format!("trace_{}", Uuid::new_v4().to_string().replace("-", ""));
    let decision_id = format!("dec_{}", Uuid::new_v4().to_string().replace("-", ""));

    // Generate action hash for replay metadata
    use sha2::Digest;
    let action_hash = format!("{:x}", sha2::Sha256::digest(serde_json::to_string(&action).unwrap_or_default()));

    if !cli_utils::is_demo_mode() {
        sleep(Duration::from_millis(150)).await;
    }

    let evaluation = state.engine.evaluate(&action);

    // Enforcement gate: execute action if ALLOW, block if DENY
    let execution_status = action::execute(&action, &evaluation.decision, &decision_id);

    if !cli_utils::is_demo_mode() {
        sleep(Duration::from_millis(100)).await;
    }

    // Emit compact execution event instead of generating artifact directly
    let event_start = Instant::now();
    
    // Create rule trace from evaluation result
    let rule_trace = execution_event::RuleTrace {
        rule_id: "infra-cost-limit".to_string(),
        field: evaluation.result.field.clone(),
        observed_value: evaluation.result.observed_value_str.clone(),
        operator: evaluation.result.rule.clone(),
        violation: evaluation.result.action.is_some(),
        evaluation_result: evaluation.result.action.is_some(),
    };

    // Create replay metadata
    let replay_metadata = execution_event::ReplayMetadata {
        policy_hash: state.engine.policy_hash().to_string(),
        action_hash,
        engine_version: "0.3.2".to_string(),
        sequence: 0, // Will be assigned by emitter
    };

    // Create performance metrics
    let performance = execution_event::PerformanceMetrics {
        evaluation_latency_us: evaluation.evaluation_latency_us as u64,
        decision_latency_us: 4, // Fixed decision latency
    };

    // Create and emit execution event
    let environment = action.environment.clone();
    
    let event = execution_event::ExecutionEvent::new(
        decision_id.clone(),
        action.session_id.clone(),
        trace_id.clone(),
        action.tool.clone(),
        environment,
        evaluation.decision.clone(),
        rule_trace,
        replay_metadata,
        performance,
        execution_status.clone(),
    );

    // Emit event to bounded queue (non-blocking)
    let event_io_start = Instant::now();
    match state.event_emitter.try_emit(event) {
        Ok(_) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            cli_utils::debug_log(format!("[EVENT QUEUE FULL] Dropping event {}", decision_id));
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            cli_utils::debug_log(format!("[EVENT QUEUE CLOSED] Dropping event {}", decision_id));
        }
    }
    let _event_io_time_us = event_io_start.elapsed().as_micros() as f64;

    let artifact_path = format!("artifacts/AuditArtifact_{}.json", decision_id);
    let hash_generation_time_us = event_start.elapsed().as_micros() as f64;

    evaluate::render_terminal_output(
        &action,
        &evaluation,
        &artifact_path,
        &decision_id,
        state.engine.policy_hash(),
    );

    Ok(ResponseJson(DecisionResponse {
        decision: evaluation.decision.clone(),
        decision_id,
        artifact_path,
        timing: TimingMetadata {
            parse_time_us,
            eval_time_us: evaluation.evaluation_latency_us,
            hash_generation_time_us,
            artifact_io_time_us: 0.0, // Now handled by event emitter
        },
    }))
}
