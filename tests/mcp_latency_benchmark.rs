use rmcp::{
    model::CallToolRequestParams,
    object,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;

const ALLOW_PATH: &str = "temp_executed_agent_allowed.txt";
const DENY_PATH: &str = "temp_executed_agent_forbidden.txt";
const CONTENT: &str = "TRAXES MCP latency benchmark payload";
const DEFAULT_WARMUP: usize = 50;
const DEFAULT_ITERATIONS: usize = 1_000;

#[derive(Debug)]
struct LatencyStats {
    samples: usize,
    min_us: f64,
    mean_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    ops_per_sec: f64,
}

fn tool_payload<T: serde::Serialize>(result: &T) -> Value {
    let wire = serde_json::to_value(result).expect("tool result must serialize");
    let text = wire
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .expect("tool result must contain text content");

    serde_json::from_str(text).expect("tool text must contain JSON")
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    assert!(!sorted.is_empty());
    let rank = ((percentile / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn stats(mut samples_us: Vec<f64>) -> LatencyStats {
    samples_us.sort_by(|a, b| a.partial_cmp(b).expect("latency must be finite"));
    let count = samples_us.len();
    let total_us: f64 = samples_us.iter().sum();
    let total_seconds = total_us / 1_000_000.0;

    LatencyStats {
        samples: count,
        min_us: samples_us[0],
        mean_us: total_us / count as f64,
        p50_us: percentile(&samples_us, 50.0),
        p95_us: percentile(&samples_us, 95.0),
        p99_us: percentile(&samples_us, 99.0),
        max_us: samples_us[count - 1],
        ops_per_sec: count as f64 / total_seconds,
    }
}

fn print_stats(label: &str, stats: &LatencyStats) {
    println!(
        "{label:<6} n={:<5} min={:>8.1} us  mean={:>8.1} us  p50={:>8.1} us  p95={:>8.1} us  p99={:>8.1} us  max={:>8.1} us  seq_ops/s={:>9.1}",
        stats.samples,
        stats.min_us,
        stats.mean_us,
        stats.p50_us,
        stats.p95_us,
        stats.p99_us,
        stats.max_us,
        stats.ops_per_sec,
    );
}

fn cleanup_artifacts(manifest_dir: &Path, artifact_paths: &[PathBuf]) {
    for artifact_path in artifact_paths {
        let path = if artifact_path.is_absolute() {
            artifact_path.clone()
        } else {
            manifest_dir.join(artifact_path)
        };
        let _ = fs::remove_file(path);
    }
}

async fn call_write(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    path: &str,
    expected_decision: &str,
    expected_status: &str,
    expected_outcome: &str,
) -> Result<(f64, PathBuf), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let result = client
        .call_tool(
            CallToolRequestParams::new("write_file").with_arguments(object!({
                "path": path,
                "content": CONTENT,
            })),
        )
        .await?;
    let elapsed_us = started.elapsed().as_secs_f64() * 1_000_000.0;
    let payload = tool_payload(&result);

    assert_eq!(payload["decision"], expected_decision);
    assert_eq!(payload["execution_status"], expected_status);
    assert_eq!(payload["execution_outcome"], expected_outcome);

    let artifact_path = payload["artifact_path"]
        .as_str()
        .expect("benchmark call must produce artifact_path");

    Ok((elapsed_us, PathBuf::from(artifact_path)))
}

#[tokio::test]
#[ignore = "manual performance benchmark; run in release mode with --ignored --nocapture"]
async fn mcp_end_to_end_latency() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let allow_path = manifest_dir.join(ALLOW_PATH);
    let deny_path = manifest_dir.join(DENY_PATH);

    let warmup = std::env::var("TRAXES_BENCH_WARMUP")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_WARMUP);
    let iterations = std::env::var("TRAXES_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_ITERATIONS);

    assert!(iterations > 0, "benchmark iterations must be greater than zero");

    let _ = fs::remove_file(&allow_path);
    let _ = fs::remove_file(&deny_path);

    let server_binary = env!("CARGO_BIN_EXE_traxes-mcp");
    let transport = TokioChildProcess::new(Command::new(server_binary).configure(|cmd| {
        cmd.current_dir(manifest_dir)
            .env("TRAXES_POLICY", "policies/file_write_agent_policy.yaml")
            // Keep the server's current logging behavior in the code path, but avoid
            // terminal rendering overhead from distorting the latency distribution.
            .stderr(Stdio::null());
    }))?;
    let client = ().serve(transport).await?;

    let tools = client.list_all_tools().await?;
    assert!(tools.iter().any(|tool| tool.name.as_ref() == "write_file"));

    let mut artifact_paths = Vec::with_capacity((warmup + iterations) * 2);

    println!("=== TRAXES MCP end-to-end latency benchmark ===");
    println!("warmup per path   : {warmup}");
    println!("measured per path : {iterations}");
    println!("mode              : sequential, local stdio, release build");
    println!("timed path        : MCP call -> policy -> permit/deny -> execution -> artifact -> MCP response");
    println!();

    for _ in 0..warmup {
        let (_, artifact) = call_write(&client, ALLOW_PATH, "ALLOW", "executed", "Executed").await?;
        artifact_paths.push(artifact);
        let (_, artifact) = call_write(&client, DENY_PATH, "DENY", "blocked", "Unauthorized").await?;
        artifact_paths.push(artifact);
    }

    let mut allow_samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let (elapsed_us, artifact) =
            call_write(&client, ALLOW_PATH, "ALLOW", "executed", "Executed").await?;
        allow_samples.push(elapsed_us);
        artifact_paths.push(artifact);
    }

    let mut deny_samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let (elapsed_us, artifact) =
            call_write(&client, DENY_PATH, "DENY", "blocked", "Unauthorized").await?;
        deny_samples.push(elapsed_us);
        artifact_paths.push(artifact);
    }

    assert!(allow_path.exists(), "ALLOW benchmark must create target file");
    assert_eq!(fs::read_to_string(&allow_path)?, CONTENT);
    assert!(!deny_path.exists(), "DENY benchmark must never create target file");

    let allow_stats = stats(allow_samples);
    let deny_stats = stats(deny_samples);

    print_stats("ALLOW", &allow_stats);
    print_stats("DENY", &deny_stats);

    client.cancel().await?;

    let _ = fs::remove_file(&allow_path);
    let _ = fs::remove_file(&deny_path);
    cleanup_artifacts(manifest_dir, &artifact_paths);

    println!();
    println!("NOTE: These are machine-local end-to-end MCP latencies, not engine-only timings.");

    Ok(())
}
