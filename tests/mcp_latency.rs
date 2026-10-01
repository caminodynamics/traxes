use rmcp::{
    model::CallToolRequestParams,
    object,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::process::Command;

const ALLOW_PATH: &str = "temp_executed_agent_allowed.txt";
const CONTENT: &str = "mcp latency benchmark";

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

fn print_stats(samples_ns: &mut [u128], wall: Duration) {
    samples_ns.sort_unstable();
    let n = samples_ns.len();
    let percentile = |q: f64| -> f64 {
        let idx = (((n - 1) as f64) * q).round() as usize;
        samples_ns[idx] as f64 / 1_000.0
    };
    let avg_us = (samples_ns.iter().copied().sum::<u128>() as f64 / n as f64) / 1_000.0;
    let min_us = samples_ns[0] as f64 / 1_000.0;
    let max_us = samples_ns[n - 1] as f64 / 1_000.0;

    println!("MCP stdio end-to-end latency (persistent session)");
    println!("  samples    : {n}");
    println!("  p50        : {:.3} us", percentile(0.50));
    println!("  p95        : {:.3} us", percentile(0.95));
    println!("  p99        : {:.3} us", percentile(0.99));
    println!("  avg        : {:.3} us", avg_us);
    println!("  min / max  : {:.3} / {:.3} us", min_us, max_us);
    println!("  throughput : {:.1} calls/sec", n as f64 / wall.as_secs_f64());
    println!("  includes   : MCP JSON-RPC + stdio + policy + permit + FILE_WRITE + sync artifact");
    println!("  excludes   : process startup and MCP initialization handshake");
}

#[tokio::test]
#[ignore = "manual latency benchmark"]
async fn mcp_persistent_stdio_latency() -> Result<(), Box<dyn std::error::Error>> {
    let iterations = std::env::var("TRAXES_MCP_LATENCY_ITERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);
    let warmup = std::env::var("TRAXES_MCP_LATENCY_WARMUP")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let allow_path = manifest_dir.join(ALLOW_PATH);
    let _ = fs::remove_file(&allow_path);

    let server_binary = env!("CARGO_BIN_EXE_traxes-mcp");
    let transport = TokioChildProcess::new(Command::new(server_binary).configure(|cmd| {
        cmd.current_dir(manifest_dir)
            .env("TRAXES_POLICY", "policies/file_write_agent_policy.yaml");
    }))?;
    let client = ().serve(transport).await?;

    let mut artifact_paths = Vec::with_capacity(iterations + warmup);

    for _ in 0..warmup {
        let result = client
            .call_tool(
                CallToolRequestParams::new("write_file").with_arguments(object!({
                    "path": ALLOW_PATH,
                    "content": CONTENT,
                })),
            )
            .await?;
        let payload = tool_payload(&result);
        assert_eq!(payload["decision"], "ALLOW");
        assert_eq!(payload["execution_status"], "executed");
        if let Some(path) = payload["artifact_path"].as_str() {
            artifact_paths.push(path.to_string());
        }
    }

    let mut samples_ns = Vec::with_capacity(iterations);
    let wall_start = Instant::now();
    for _ in 0..iterations {
        let start = Instant::now();
        let result = client
            .call_tool(
                CallToolRequestParams::new("write_file").with_arguments(object!({
                    "path": ALLOW_PATH,
                    "content": CONTENT,
                })),
            )
            .await?;
        samples_ns.push(start.elapsed().as_nanos());

        let payload = tool_payload(&result);
        assert_eq!(payload["decision"], "ALLOW");
        assert_eq!(payload["execution_status"], "executed");
        assert_eq!(payload["execution_outcome"], "Executed");
        if let Some(path) = payload["artifact_path"].as_str() {
            artifact_paths.push(path.to_string());
        }
    }
    let wall = wall_start.elapsed();

    assert_eq!(fs::read_to_string(&allow_path)?, CONTENT);
    print_stats(&mut samples_ns, wall);

    client.cancel().await?;

    for path in artifact_paths {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_file(&allow_path);

    Ok(())
}
