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
use crate::artifact::ArtifactLogger;
use serde::Serialize;
use serde_json;
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use uuid::Uuid;
use traxes_demo::async_logger;

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
            // Initialize async logging queue (capacity: 1000 log messages)
            let (async_log_sender, async_log_rx) = async_logger::create_async_log_channel(1000);
            async_logger::init_global_async_logger(async_log_sender);
            
            // Spawn async logger worker
            let async_logger_worker = async_logger::AsyncLogger::new(async_log_rx);
            tokio::spawn(async_logger_worker.run());
            
            if !cli_utils::is_demo_mode() {
                println!("[Traxes] Async logger initialized");
            }
            
            // Initialize sharded event emitter (4 shards to reduce contention, total capacity: 1000)
            let shard_count = 4;
            let capacity_per_shard = 250;
            let (event_emitter, event_receivers, event_counter) = artifact_emitter::create_sharded_event_channels(shard_count, capacity_per_shard);
            
            // Spawn artifact emitter workers (one per shard)
            let policy_hash = Engine::load_default_policies()
                .expect("Failed to load default policy bundle")
                .policy_hash()
                .to_string();
            
            for event_rx in event_receivers {
                let artifact_emitter = ArtifactEmitter::new(event_rx, policy_hash.clone(), event_counter.clone());
                tokio::spawn(async move {
                    artifact_emitter.run().await;
                });
            }

            if !cli_utils::is_demo_mode() {
                println!("[Traxes] Sharded event emitter initialized: {} shards, {} capacity per shard", shard_count, capacity_per_shard);
                println!("[Traxes] Artifact emitter workers spawned: {}", shard_count);
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
            
            // Initialize async logging queue for CLI evaluate mode
            let (async_log_sender, async_log_rx) = async_logger::create_async_log_channel(100);
            async_logger::init_global_async_logger(async_log_sender);
            
            // Spawn async logger worker
            let async_logger_worker = async_logger::AsyncLogger::new(async_log_rx);
            tokio::spawn(async_logger_worker.run());
            
            // Initialize sharded event emitter for CLI evaluate mode (2 shards for CLI, total capacity: 100)
            let shard_count = 2;
            let capacity_per_shard = 50;
            let (event_emitter, event_receivers, event_counter) = artifact_emitter::create_sharded_event_channels(shard_count, capacity_per_shard);
            
            // Spawn artifact emitter workers (one per shard)
            let engine = Engine::load_default_policies().expect("Failed to load default policy bundle");
            let policy_hash = engine.policy_hash().to_string();
            
            for event_rx in event_receivers {
                let artifact_emitter = ArtifactEmitter::new(event_rx, policy_hash.clone(), event_counter.clone());
                tokio::spawn(async move {
                    artifact_emitter.run().await;
                });
            }
            
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

    // Emit lightweight decision record instead of full execution event
    let event_start = Instant::now();
    
    // Create lightweight decision record for async artifact construction
    let decision_record = crate::traxes_engine::DecisionRecord {
        decision: evaluation.decision.clone(),
        session_id: action.session_id.clone(),
        tool: action.tool.clone(),
        environment: action.environment.clone(),
        parameters: action.parameters.clone(),
        policy_hash: state.engine.policy_hash().to_string(),
        evaluation_result: evaluation.result.clone(),
        evaluation_latency_us: evaluation.evaluation_latency_us,
        decision_id: decision_id.clone(),
        trace_id: trace_id.clone(),
        execution_status: execution_status.clone(),
    };

    // Emit decision record to bounded queue (non-blocking)
    let event_io_start = Instant::now();
    match state.event_emitter.try_emit_record(decision_record) {
        Ok(_) => {}
        Err(mpsc::error::TrySendError::Full(_)) => {
            cli_utils::debug_log(format!("[EVENT QUEUE FULL] Using synchronous fallback for {}", decision_id));
            // Fallback to synchronous artifact generation to preserve Invariant #2
            let artifact = ArtifactLogger::generate_artifact(
                &decision_id,
                &action,
                &evaluation,
                state.engine.policy_hash(),
                execution_status.clone(),
            );
            if let Err(e) = ArtifactLogger::write_sync(&artifact) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson(ErrorResponse {
                        error: format!("Failed to write artifact (fallback): {}", e),
                    })
                ));
            }
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            cli_utils::debug_log(format!("[EVENT QUEUE CLOSED] Using synchronous fallback for {}", decision_id));
            // Fallback to synchronous artifact generation to preserve Invariant #2
            let artifact = ArtifactLogger::generate_artifact(
                &decision_id,
                &action,
                &evaluation,
                state.engine.policy_hash(),
                execution_status.clone(),
            );
            if let Err(e) = ArtifactLogger::write_sync(&artifact) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ResponseJson(ErrorResponse {
                        error: format!("Failed to write artifact (fallback): {}", e),
                    })
                ));
            }
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
