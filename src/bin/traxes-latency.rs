use serde_json::json;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use traxes_demo::action::{self, ExecutionOutcome, ProposedAction};
use traxes_demo::artifact::ArtifactLogger;
use traxes_demo::traxes_engine::Engine;
use uuid::Uuid;

#[derive(Debug)]
struct SampleSet {
    name: &'static str,
    samples_ns: Vec<u128>,
    wall: Duration,
}

impl SampleSet {
    fn print(&mut self) {
        self.samples_ns.sort_unstable();
        let n = self.samples_ns.len();
        let percentile = |q: f64| -> f64 {
            let idx = (((n - 1) as f64) * q).round() as usize;
            self.samples_ns[idx] as f64 / 1_000.0
        };
        let sum_ns: u128 = self.samples_ns.iter().copied().sum();
        let avg_us = (sum_ns as f64 / n as f64) / 1_000.0;
        let min_us = self.samples_ns[0] as f64 / 1_000.0;
        let max_us = self.samples_ns[n - 1] as f64 / 1_000.0;
        let throughput = n as f64 / self.wall.as_secs_f64();

        println!("{}", self.name);
        println!("  samples    : {n}");
        println!("  p50        : {:.3} us", percentile(0.50));
        println!("  p95        : {:.3} us", percentile(0.95));
        println!("  p99        : {:.3} us", percentile(0.99));
        println!("  avg        : {:.3} us", avg_us);
        println!("  min / max  : {:.3} / {:.3} us", min_us, max_us);
        println!("  throughput : {:.0} ops/sec", throughput);
        println!();
    }
}

fn normalize(path: &PathBuf) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn measure<F>(name: &'static str, iterations: usize, mut op: F) -> SampleSet
where
    F: FnMut(usize),
{
    let mut samples_ns = Vec::with_capacity(iterations);
    let wall_start = Instant::now();
    for i in 0..iterations {
        let start = Instant::now();
        op(i);
        samples_ns.push(start.elapsed().as_nanos());
    }
    SampleSet {
        name,
        samples_ns,
        wall: wall_start.elapsed(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let engine_iterations = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100_000);
    let io_iterations = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1_000);

    let sandbox = std::env::temp_dir().join(format!("traxes-latency-{}", Uuid::new_v4()));
    fs::create_dir_all(&sandbox)?;
    let allow_path = sandbox.join("allowed.txt");
    let deny_path = sandbox.join("denied.txt");
    let allow_path_text = normalize(&allow_path);
    let deny_path_text = normalize(&deny_path);

    let policy = format!(
        "target:\n  tool: FILE_WRITE\nrules:\n  - name: latency-path-allowlist\n    condition: payload.proposed_action.parameters.path not in [{}]\n    action: DENY",
        serde_json::to_string(&allow_path_text)?
    );
    let engine = Engine::with_policy(policy);

    let allow_action = ProposedAction {
        tool: "FILE_WRITE".into(),
        session_id: "latency-allow".into(),
        environment: "sandbox".into(),
        parameters: json!({"path": allow_path_text, "content": "latency benchmark bytes"}),
    };
    let deny_action = ProposedAction {
        tool: "FILE_WRITE".into(),
        session_id: "latency-deny".into(),
        environment: "sandbox".into(),
        parameters: json!({"path": deny_path_text, "content": "must not be written"}),
    };

    println!("TRAXES layered latency benchmark");
    println!("engine iterations : {engine_iterations}");
    println!("I/O iterations    : {io_iterations}");
    println!("timer resolution  : nanoseconds (reported as microseconds)\n");

    // Warm the pure policy path and the permit path before recording samples.
    for _ in 0..10_000 {
        black_box(engine.evaluate(&allow_action));
        let (evaluation, permit) = engine.evaluate_with_permit(&deny_action, "warm-deny");
        black_box(evaluation);
        black_box(action::execute(&deny_action, permit));
    }

    let mut engine_only = measure("1) policy decision only", engine_iterations, |_| {
        black_box(engine.evaluate(&allow_action));
    });

    let mut deny_boundary = measure(
        "2) decision + permit enforcement (DENY, no side effect)",
        engine_iterations,
        |_| {
            let (evaluation, permit) = engine.evaluate_with_permit(&deny_action, "bench-deny");
            black_box(evaluation);
            let outcome = action::execute(&deny_action, permit);
            assert_eq!(outcome, ExecutionOutcome::Unauthorized);
            black_box(outcome);
        },
    );

    // A small disk warm-up keeps first-write filesystem setup out of the measured series.
    for _ in 0..25 {
        let (_, permit) = engine.evaluate_with_permit(&allow_action, "warm-write");
        let outcome = action::execute(&allow_action, permit);
        assert_eq!(outcome, ExecutionOutcome::Executed);
    }

    let mut allow_write = measure(
        "3) decision + permit + real FILE_WRITE",
        io_iterations,
        |_| {
            let (evaluation, permit) = engine.evaluate_with_permit(&allow_action, "bench-write");
            black_box(evaluation);
            let outcome = action::execute(&allow_action, permit);
            assert_eq!(outcome, ExecutionOutcome::Executed);
            black_box(outcome);
        },
    );

    let mut artifact_paths = Vec::with_capacity(io_iterations);
    let mut allow_write_artifact = measure(
        "4) decision + permit + FILE_WRITE + synchronous artifact",
        io_iterations,
        |i| {
            let decision_id = format!("latency_{}_{}", std::process::id(), i);
            let (evaluation, permit) = engine.evaluate_with_permit(&allow_action, &decision_id);
            let outcome = action::execute(&allow_action, permit);
            assert_eq!(outcome, ExecutionOutcome::Executed);

            let artifact = ArtifactLogger::generate_artifact_with_outcome(
                &decision_id,
                &allow_action,
                &evaluation,
                engine.policy_hash(),
                "executed".to_string(),
                Some(format!("{:?}", outcome)),
            );
            let path = artifact
                .write_to_file_sync()
                .expect("latency artifact write must succeed");
            artifact_paths.push(path);
        },
    );

    println!("RESULTS\n-------");
    engine_only.print();
    deny_boundary.print();
    allow_write.print();
    allow_write_artifact.print();

    println!("Interpretation:");
    println!("  #1 isolates policy evaluation.");
    println!("  #2 adds permit creation/checking without filesystem work.");
    println!("  #3 adds the governed target side effect.");
    println!("  #4 adds durable synchronous artifact I/O.");
    println!("  HTTP and MCP transport benchmarks are measured separately.\n");

    for path in artifact_paths {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_file(&allow_path);
    let _ = fs::remove_file(&deny_path);
    let _ = fs::remove_dir_all(&sandbox);

    Ok(())
}
