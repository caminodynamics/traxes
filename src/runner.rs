use chrono::Utc;
use traxes_demo::engine::{run_baseline, run_traxes};
use traxes_demo::metrics::estimate_workload_memory;
use traxes_demo::policy::PolicyEngine;
use traxes_demo::alloc_snapshot;
use serde::Serialize;
#[cfg(feature = "metrics")]
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
#[cfg(feature = "metrics")]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use traxes_demo::workload::{Scenario, ToolCall, WorkloadConfig, WorkloadGenerator};

#[cfg(all(feature = "instrumentation", feature = "metrics"))]
compile_error!("features \"instrumentation\" and \"metrics\" are mutually exclusive");

#[cfg(feature = "instrumentation")]
use dhat::Alloc;

#[cfg(feature = "instrumentation")]
#[global_allocator]
static ALLOC: Alloc = Alloc;

#[cfg(feature = "metrics")]
struct AllocCounting;

#[cfg(feature = "metrics")]
static TOTAL_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "metrics")]
static TOTAL_ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "metrics")]
unsafe impl GlobalAlloc for AllocCounting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            TOTAL_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            TOTAL_ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc_zeroed(layout);
        if !ptr.is_null() {
            TOTAL_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            TOTAL_ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = System.realloc(ptr, old_layout, new_size);
        if !new_ptr.is_null() {
            TOTAL_ALLOCATED_BYTES.fetch_add(
                new_size.saturating_sub(old_layout.size()),
                Ordering::Relaxed,
            );
        }
        new_ptr
    }
}

#[cfg(feature = "metrics")]
#[global_allocator]
static ALLOC: AllocCounting = AllocCounting;

#[derive(Debug, PartialEq, Clone)]
enum Mode {
    Baseline,
    Traxes,
    Compare,
}

impl Mode {
    fn from_str(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "baseline" => Mode::Baseline,
            "Traxes" => Mode::Traxes,
            _ => Mode::Compare,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Mode::Baseline => "baseline",
            Mode::Traxes => "Traxes",
            Mode::Compare => "compare",
        }
    }
}

#[derive(Debug)]
struct Config {
    request_count: usize,
    seed: u64,
    threads: usize,
    warmup: usize,
    mode: Mode,
    scenario: Scenario,
}

impl Config {
    fn from_args(args: &[String]) -> Self {
        let mut request_count = 10_000;
        let mut seed = 42;
        let mut threads = 1;
        let mut warmup = 1000;
        let mut mode = Mode::Compare;
        let mut scenario = Scenario::RogueInfraAgent;

        let mut iter = args.iter().skip(1);
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--requests" | "-r" => {
                    if let Some(value) = iter.next() {
                        request_count = value.parse().unwrap_or(request_count);
                    }
                }
                "--seed" | "-s" => {
                    if let Some(value) = iter.next() {
                        seed = value.parse().unwrap_or(seed);
                    }
                }
                "--threads" | "-t" => {
                    if let Some(value) = iter.next() {
                        threads = value.parse().unwrap_or(1).max(1);
                    }
                }
                "--warmup" => {
                    if let Some(value) = iter.next() {
                        warmup = value.parse().unwrap_or(warmup);
                    }
                }
                "--mode" => {
                    if let Some(value) = iter.next() {
                        mode = Mode::from_str(value);
                    }
                }
                "--scenario" => {
                    if let Some(value) = iter.next() {
                        scenario = Scenario::from_str(value);
                    }
                }
                _ => {}
            }
        }

        Config {
            request_count,
            seed,
            threads,
            warmup,
            mode,
            scenario,
        }
    }
}

#[derive(Debug, Serialize)]
struct EnvironmentInfo {
    os: String,
    cpu_arch: String,
    rustc_version: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct RunMetrics {
    // Total request latency (evaluation + decision_log + enqueue)
    p50_ns: u128,
    p95_ns: u128,
    p99_ns: u128,
    average_ns: u128,
    throughput_ops_sec: f64,

    // Evaluation latency breakdown
    eval_p50_ns: u128,
    eval_p95_ns: u128,
    eval_p99_ns: u128,
    eval_average_ns: u128,

    // Decision logging latency breakdown (Traxes only)
    log_p50_ns: u128,
    log_p95_ns: u128,
    log_p99_ns: u128,
    log_average_ns: u128,

    // Enqueue/receipt latency breakdown (currently 0 in sync path)
    enqueue_p50_ns: u128,
    enqueue_p95_ns: u128,
    enqueue_p99_ns: u128,
    enqueue_average_ns: u128,

    total_requests: usize,
    allow_count: usize,
    deny_count: usize,
    duration_ns: u128,
    allocations: usize,
    allocated_bytes: usize,
    artifact_enqueue_count: usize,
    artifact_enqueue_latency_ns: u128,
    queue_max_depth: usize,
    queue_drop_count: usize,
}

#[derive(Debug, Serialize)]
struct DeltaMetrics {
    p50_ns: i128,
    p95_ns: i128,
    p99_ns: i128,
    average_ns: i128,
    throughput_percent: f64,
}

impl DeltaMetrics {
    fn from_results(baseline: &RunMetrics, Traxes: &RunMetrics) -> Self {
        let p50_ns = Traxes.p50_ns as i128 - baseline.p50_ns as i128;
        let p95_ns = Traxes.p95_ns as i128 - baseline.p95_ns as i128;
        let p99_ns = Traxes.p99_ns as i128 - baseline.p99_ns as i128;
        let average_ns = Traxes.average_ns as i128 - baseline.average_ns as i128;
        let throughput_percent = if baseline.throughput_ops_sec > 0.0 {
            ((Traxes.throughput_ops_sec - baseline.throughput_ops_sec)
                / baseline.throughput_ops_sec)
                * 100.0
        } else {
            0.0
        };

        Self {
            p50_ns,
            p95_ns,
            p99_ns,
            average_ns,
            throughput_percent,
        }
    }
}

#[derive(Debug, Serialize)]
struct BenchmarkResult {
    mode: String,
    scenario: String,
    requests: usize,
    threads: usize,
    seed: u64,
    warmup: usize,
    scenario_allow_count: usize,
    scenario_deny_count: usize,
    policy_bypasses: usize,
    baseline: Option<RunMetrics>,
    Traxes: Option<RunMetrics>,
    delta: Option<DeltaMetrics>,
    environment: EnvironmentInfo,
}

#[derive(Debug, Serialize)]
struct BenchmarkMatrix {
    mode: String,
    scenario: String,
    requests: usize,
    seed: u64,
    warmup: usize,
    policy_bypasses: usize,
    thread_counts: Vec<usize>,
    results: BTreeMap<usize, BenchmarkResult>,
    environment: EnvironmentInfo,
}

fn main() {
    let config = Config::from_args(&env::args().collect::<Vec<_>>());

    // Instrumentation is only enabled when building with `--features instrumentation`.
    #[cfg(feature = "instrumentation")]
    let _dhat = dhat::Profiler::builder().testing().build();

    let total_requests = config.request_count + config.warmup;
    let workload = WorkloadGenerator::generate(&WorkloadConfig {
        request_count: total_requests,
        seed: config.seed,
        scenario: config.scenario.clone(),
    });

    let memory_estimate = estimate_workload_memory(&workload);
    let policy_engine = Arc::new(PolicyEngine::default());
    let workload = Arc::new(workload);

    let mut thread_counts = vec![1, 4, 16];
    if config.threads != 1 && !thread_counts.contains(&config.threads) {
        thread_counts.push(config.threads);
        thread_counts.sort_unstable();
        thread_counts.dedup();
    }

    let mut results = BTreeMap::new();

    for &thread_count in &thread_counts {
        let baseline_result = if config.mode == Mode::Baseline || config.mode == Mode::Compare {
            Some(execute_mode(
                &workload,
                config.warmup,
                thread_count,
                Mode::Baseline,
                Some(policy_engine.clone()),
            ))
        } else {
            None
        };

        let traxes_result = if config.mode == Mode::Traxes || config.mode == Mode::Compare {
            Some(execute_mode(
                &workload,
                config.warmup,
                thread_count,
                Mode::Traxes,
                Some(policy_engine.clone()),
            ))
        } else {
            None
        };

        let delta = match (&baseline_result, &traxes_result) {
            (Some(baseline), Some(Traxes)) => Some(DeltaMetrics::from_results(baseline, Traxes)),
            _ => None,
        };

        let scenario_metrics = match (&traxes_result, &baseline_result) {
            (Some(Traxes), _) => (Traxes.allow_count, Traxes.deny_count),
            (None, Some(baseline)) => (baseline.allow_count, baseline.deny_count),
            _ => (0, 0),
        };

        let benchmark_result = BenchmarkResult {
            mode: config.mode.as_str().to_string(),
            scenario: config.scenario.as_str().to_string(),
            requests: config.request_count,
            threads: thread_count,
            seed: config.seed,
            warmup: config.warmup,
            scenario_allow_count: scenario_metrics.0,
            scenario_deny_count: scenario_metrics.1,
            policy_bypasses: 0,
            baseline: baseline_result,
            Traxes: traxes_result,
            delta,
            environment: collect_environment_info(),
        };

        results.insert(thread_count, benchmark_result);
    }

    let benchmark_matrix = BenchmarkMatrix {
        mode: config.mode.as_str().to_string(),
        scenario: config.scenario.as_str().to_string(),
        requests: config.request_count,
        seed: config.seed,
        warmup: config.warmup,
        policy_bypasses: 0,
        thread_counts: thread_counts.clone(),
        results,
        environment: collect_environment_info(),
    };

    print_matrix_summary(memory_estimate, &benchmark_matrix);
    let json_path = write_results_json(&benchmark_matrix).expect("Failed to write benchmark JSON");
    println!("Results written to {}", json_path.display());
}

fn execute_mode(
    workload: &Arc<Vec<ToolCall>>,
    warmup: usize,
    threads: usize,
    mode: Mode,
    engine: Option<Arc<PolicyEngine>>,
) -> RunMetrics {
    let total = workload.len();
    let warmup_count = warmup.min(total);
    let final_start = warmup_count;
    let final_end = total;

    if warmup_count > 0 {
        run_slice(
            workload.clone(),
            0,
            warmup_count,
            threads,
            mode.clone(),
            engine.clone(),
        );
    }

    let final_result = run_slice(
        workload.clone(),
        final_start,
        final_end,
        threads,
        mode,
        engine,
    );
    let total_latency = final_result.stats();
    let eval_latency = final_result.evaluation_stats();
    let log_latency = final_result.decision_log_stats();
    let enqueue_latency = final_result.enqueue_stats();

    RunMetrics {
        p50_ns: total_latency.p50_ns,
        p95_ns: total_latency.p95_ns,
        p99_ns: total_latency.p99_ns,
        average_ns: total_latency.average_ns,
        throughput_ops_sec: total_latency.throughput_ops_per_sec,
        eval_p50_ns: eval_latency.p50_ns,
        eval_p95_ns: eval_latency.p95_ns,
        eval_p99_ns: eval_latency.p99_ns,
        eval_average_ns: eval_latency.average_ns,
        log_p50_ns: log_latency.p50_ns,
        log_p95_ns: log_latency.p95_ns,
        log_p99_ns: log_latency.p99_ns,
        log_average_ns: log_latency.average_ns,
        enqueue_p50_ns: enqueue_latency.p50_ns,
        enqueue_p95_ns: enqueue_latency.p95_ns,
        enqueue_p99_ns: enqueue_latency.p99_ns,
        enqueue_average_ns: enqueue_latency.average_ns,
        total_requests: final_result.latencies_ns.len(),
        allow_count: final_result.allowed,
        deny_count: final_result.denied,
        duration_ns: final_result.duration_ns,
        allocations: final_result.allocations,
        allocated_bytes: final_result.allocated_bytes,
        artifact_enqueue_count: final_result.artifact_enqueue_count,
        artifact_enqueue_latency_ns: final_result.artifact_enqueue_latency_ns,
        queue_max_depth: final_result.queue_max_depth,
        queue_drop_count: final_result.queue_drop_count,
    }
}

fn run_slice(
    workload: Arc<Vec<ToolCall>>,
    start: usize,
    end: usize,
    threads: usize,
    mode: Mode,
    engine: Option<Arc<PolicyEngine>>,
) -> traxes_demo::engine::RunResult {
    if threads <= 1 {
        match mode {
            Mode::Baseline => run_baseline(&workload[start..end]),
            Mode::Traxes => run_traxes(
                &workload[start..end],
                engine
                    .as_ref()
                    .expect("Policy engine required for Traxes path"),
            ),
            Mode::Compare => run_baseline(&workload[start..end]),
        }
    } else {
        match mode {
            Mode::Baseline => run_baseline_concurrent_range(workload, start, end, threads),
            Mode::Traxes => run_traxes_concurrent_range(
                workload,
                engine.expect("Policy engine required for Traxes path"),
                start,
                end,
                threads,
            ),
            Mode::Compare => run_baseline_concurrent_range(workload, start, end, threads),
        }
    }
}

fn run_baseline_concurrent_range(
    workload: Arc<Vec<ToolCall>>,
    start: usize,
    end: usize,
    threads: usize,
) -> traxes_demo::engine::RunResult {
    let len = end.saturating_sub(start);
    let chunk_size = (len + threads - 1) / threads;
    let mut handles = Vec::with_capacity(threads);
    let run_start = std::time::Instant::now();

    for offset in (0..len).step_by(chunk_size) {
        let chunk_start = start + offset;
        let chunk_end = (chunk_start + chunk_size).min(end);
        let chunk = workload.clone();
        handles.push(thread::spawn(move || {
            run_baseline(&chunk[chunk_start..chunk_end])
        }));
    }

    let mut merged_latencies = Vec::with_capacity(len);
    let mut merged_eval_latencies = Vec::with_capacity(len);
    let mut merged_log_latencies = Vec::with_capacity(len);
    let mut merged_enqueue_latencies = Vec::with_capacity(len);
    let mut allowed = 0;
    let mut denied = 0;
    let mut allocations = 0;
    let mut allocated_bytes = 0;

    for handle in handles {
        let result: traxes_demo::engine::RunResult = handle.join().expect("baseline thread panicked");
        merged_latencies.extend(result.latencies_ns);
        merged_eval_latencies.extend(result.evaluation_latencies_ns);
        merged_log_latencies.extend(result.decision_log_latencies_ns);
        merged_enqueue_latencies.extend(result.enqueue_latencies_ns);
        allowed += result.allowed;
        denied += result.denied;
        allocations += result.allocations;
        allocated_bytes += result.allocated_bytes;
    }

    let duration_ns = run_start.elapsed().as_nanos();
    traxes_demo::engine::RunResult {
        latencies_ns: merged_latencies,
        evaluation_latencies_ns: merged_eval_latencies,
        decision_log_latencies_ns: merged_log_latencies,
        enqueue_latencies_ns: merged_enqueue_latencies,
        allowed,
        denied,
        duration_ns,
        allocations,
        allocated_bytes,
        artifact_enqueue_count: 0,
        artifact_enqueue_latency_ns: 0,
        queue_max_depth: 0,
        queue_drop_count: 0,
    }
}

fn run_traxes_concurrent_range(
    workload: Arc<Vec<ToolCall>>,
    engine: Arc<PolicyEngine>,
    start: usize,
    end: usize,
    threads: usize,
) -> traxes_demo::engine::RunResult {
    let len = end.saturating_sub(start);
    let chunk_size = (len + threads - 1) / threads;
    let mut handles = Vec::with_capacity(threads);
    let run_start = std::time::Instant::now();

    for offset in (0..len).step_by(chunk_size) {
        let chunk_start = start + offset;
        let chunk_end = (chunk_start + chunk_size).min(end);
        let chunk = workload.clone();
        let engine = engine.clone();
        handles.push(thread::spawn(move || {
            run_traxes(&chunk[chunk_start..chunk_end], &engine)
        }));
    }

    let mut merged_latencies = Vec::with_capacity(len);
    let mut merged_eval_latencies = Vec::with_capacity(len);
    let mut merged_log_latencies = Vec::with_capacity(len);
    let mut merged_enqueue_latencies = Vec::with_capacity(len);
    let mut allowed = 0;
    let mut denied = 0;
    let mut allocations = 0;
    let mut allocated_bytes = 0;

    for handle in handles {
        let result: traxes_demo::engine::RunResult = handle.join().expect("Traxes thread panicked");
        merged_latencies.extend(result.latencies_ns);
        merged_eval_latencies.extend(result.evaluation_latencies_ns);
        merged_log_latencies.extend(result.decision_log_latencies_ns);
        merged_enqueue_latencies.extend(result.enqueue_latencies_ns);
        allowed += result.allowed;
        denied += result.denied;
        allocations += result.allocations;
        allocated_bytes += result.allocated_bytes;
    }

    let duration_ns = run_start.elapsed().as_nanos();
    traxes_demo::engine::RunResult {
        latencies_ns: merged_latencies,
        evaluation_latencies_ns: merged_eval_latencies,
        decision_log_latencies_ns: merged_log_latencies,
        enqueue_latencies_ns: merged_enqueue_latencies,
        allowed,
        denied,
        duration_ns,
        allocations,
        allocated_bytes,
        artifact_enqueue_count: 0,
        artifact_enqueue_latency_ns: 0,
        queue_max_depth: 0,
        queue_drop_count: 0,
    }
}

fn print_matrix_summary(memory_bytes: usize, report: &BenchmarkMatrix) {
    println!("Traxes-bench summary");
    println!("mode: {}", report.mode);
    println!("scenario: {}", report.scenario);
    println!("requests: {}", report.requests);
    println!("warmup: {}", report.warmup);
    println!("seed: {}", report.seed);
    println!("thread matrix: {:?}", report.thread_counts);
    println!("estimated workload memory: {} bytes", memory_bytes);
    println!("");

    for (threads, result) in &report.results {
        println!("=== Thread count: {} ===", threads);
        println!("Scenario: {}", result.scenario);
        println!("Allowed: {}", result.scenario_allow_count);
        println!("Denied: {}", result.scenario_deny_count);
        println!("Policy bypasses: {}", result.policy_bypasses);
        println!("");

        if result.mode == "compare" {
            if let (Some(baseline), Some(Traxes), Some(delta)) =
                (&result.baseline, &result.Traxes, &result.delta)
            {
                println!("=== Benchmark Comparison ===");
                println!("Threads: {}", result.threads);
                println!("Warmup: {}", result.warmup);
                println!("");
                println!("Baseline:");
                println!("  p50: {} ns", baseline.p50_ns);
                println!("  p95: {} ns", baseline.p95_ns);
                println!("  p99: {} ns", baseline.p99_ns);
                println!("  throughput: {:.2} ops/sec", baseline.throughput_ops_sec);
                println!("");
                println!("Traxes:");
                println!("  p50: {} ns", Traxes.p50_ns);
                println!("  p95: {} ns", Traxes.p95_ns);
                println!("  p99: {} ns", Traxes.p99_ns);
                println!("  throughput: {:.2} ops/sec", Traxes.throughput_ops_sec);
                println!("");
                println!("Overhead:");
                println!("  p50 delta: {}", format_delta(delta.p50_ns));
                println!("  p95 delta: {}", format_delta(delta.p95_ns));
                println!("  p99 delta: {}", format_delta(delta.p99_ns));
                println!("  average delta: {}", format_delta(delta.average_ns));
                println!("  throughput delta: {:+.1}%", delta.throughput_percent);
                println!("");
                continue;
            }
        }

        if let Some(baseline) = &result.baseline {
            print_metrics_summary("baseline", baseline);
        }
        if let Some(Traxes) = &result.Traxes {
            print_metrics_summary("Traxes", Traxes);
        }
    }
}

fn print_metrics_summary(label: &str, metrics: &RunMetrics) {
    println!("{}:", label);
    println!("  total_requests: {}", metrics.total_requests);
    println!("  allow_count: {}", metrics.allow_count);
    println!("  deny_count: {}", metrics.deny_count);
    println!("");

    println!("  === Total Request Latency ===");
    println!("    p50: {} ns", metrics.p50_ns);
    println!("    p95: {} ns", metrics.p95_ns);
    println!("    p99: {} ns", metrics.p99_ns);
    println!("    average: {} ns", metrics.average_ns);
    println!("    throughput: {:.2} ops/sec", metrics.throughput_ops_sec);
    println!("");

    println!("  === Policy Evaluation Latency ===");
    println!("    p50: {} ns", metrics.eval_p50_ns);
    println!("    p95: {} ns", metrics.eval_p95_ns);
    println!("    p99: {} ns", metrics.eval_p99_ns);
    println!("    average: {} ns", metrics.eval_average_ns);
    println!("");

    println!("  === Decision Logging Latency ===");
    println!("    p50: {} ns", metrics.log_p50_ns);
    println!("    p95: {} ns", metrics.log_p95_ns);
    println!("    p99: {} ns", metrics.log_p99_ns);
    println!("    average: {} ns", metrics.log_average_ns);
    println!("");

    println!("  === Async Enqueue Latency ===");
    println!("    p50: {} ns", metrics.enqueue_p50_ns);
    println!("    p95: {} ns", metrics.enqueue_p95_ns);
    println!("    p99: {} ns", metrics.enqueue_p99_ns);
    println!("    average: {} ns", metrics.enqueue_average_ns);
    println!("");

    println!("  === Allocation & Throughput ===");
    println!("    total_duration: {} ns", metrics.duration_ns);
    println!("    total_allocations: {}", metrics.allocations);
    println!("    allocated_bytes: {}", metrics.allocated_bytes);
    println!(
        "    artifact_enqueue_count: {}",
        metrics.artifact_enqueue_count
    );
    println!(
        "    artifact_enqueue_ns: {} ns",
        metrics.artifact_enqueue_latency_ns
    );
    println!("    queue_max_depth: {}", metrics.queue_max_depth);
    println!("    queue_drop_count: {}", metrics.queue_drop_count);
    println!("");
}

fn format_delta(value: i128) -> String {
    if value >= 0 {
        format!("+{} ns", value)
    } else {
        format!("{} ns", value)
    }
}

fn write_results_json(report: &BenchmarkMatrix) -> std::io::Result<std::path::PathBuf> {
    let results_dir = Path::new("results");
    fs::create_dir_all(results_dir)?;

    let filename = format!(
        "benchmark_{}_{}req_seed{}_{}_warmup{}.json",
        report.mode, report.requests, report.seed, report.scenario, report.warmup
    );

    let path = results_dir.join(filename);
    let json = serde_json::to_string_pretty(report)?;
    let mut file = File::create(&path)?;
    file.write_all(json.as_bytes())?;
    Ok(path)
}

fn collect_environment_info() -> EnvironmentInfo {
    let os = env::consts::OS.to_string();
    let cpu_arch = env::consts::ARCH.to_string();
    let rustc_version = get_rustc_version();
    let timestamp = Utc::now().to_rfc3339();

    EnvironmentInfo {
        os,
        cpu_arch,
        rustc_version,
        timestamp,
    }
}

fn get_rustc_version() -> String {
    if let Ok(output) = Command::new("rustc").arg("--version").output() {
        if output.status.success() {
            if let Ok(text) = String::from_utf8(output.stdout) {
                return text.trim().to_string();
            }
        }
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use traxes_demo::policy::PolicyEngine;
    use traxes_demo::workload::{Scenario, WorkloadConfig, WorkloadGenerator};
    use std::sync::Arc;

    #[test]
    fn scenario_cli_parsing_handles_known_and_unknown_values() {
        let rogue = Config::from_args(&[
            "bench".to_string(),
            "--scenario".to_string(),
            "rogue-infra-agent".to_string(),
        ]);
        assert_eq!(rogue.scenario, Scenario::RogueInfraAgent);

        let finops = Config::from_args(&[
            "bench".to_string(),
            "--scenario".to_string(),
            "finops-guardrails".to_string(),
        ]);
        assert_eq!(finops.scenario, Scenario::FinopsGuardrails);

        let dangerous = Config::from_args(&[
            "bench".to_string(),
            "--scenario".to_string(),
            "dangerous-db-ops".to_string(),
        ]);
        assert_eq!(dangerous.scenario, Scenario::DangerousDbOps);

        let clean_allow = Config::from_args(&[
            "bench".to_string(),
            "--scenario".to_string(),
            "clean-allow-agent".to_string(),
        ]);
        assert_eq!(clean_allow.scenario, Scenario::CleanAllowAgent);

        let fallback = Config::from_args(&[
            "bench".to_string(),
            "--scenario".to_string(),
            "invalid-scenario".to_string(),
        ]);
        assert_eq!(fallback.scenario, Scenario::RogueInfraAgent);
    }

    #[test]
    fn warmup_requests_are_excluded_from_metrics() {
        let workload = Arc::new(WorkloadGenerator::generate(&WorkloadConfig {
            request_count: 10,
            seed: 11,
            scenario: Scenario::DangerousDbOps,
        }));
        let metrics = execute_mode(
            &workload,
            3,
            1,
            Mode::Traxes,
            Some(Arc::new(PolicyEngine::default())),
        );

        assert_eq!(metrics.total_requests, 7);
        assert_eq!(metrics.allow_count + metrics.deny_count, 7);
    }

    #[test]
    fn compare_mode_can_produce_both_baseline_and_traxes_metrics() {
        let workload = Arc::new(WorkloadGenerator::generate(&WorkloadConfig {
            request_count: 6,
            seed: 31,
            scenario: Scenario::FinopsGuardrails,
        }));
        let engine = Arc::new(PolicyEngine::default());

        let baseline_metrics = execute_mode(&workload, 0, 1, Mode::Baseline, Some(engine.clone()));
        let traxes_metrics = execute_mode(&workload, 0, 1, Mode::Traxes, Some(engine));

        assert_eq!(baseline_metrics.total_requests, 6);
        assert_eq!(baseline_metrics.allow_count, 6);
        assert_eq!(baseline_metrics.deny_count, 0);
        assert_eq!(traxes_metrics.total_requests, 6);
        assert_eq!(traxes_metrics.allow_count + traxes_metrics.deny_count, 6);
    }

    #[test]
    fn threads_one_path_remains_deterministic_for_traxes() {
        let workload = Arc::new(WorkloadGenerator::generate(&WorkloadConfig {
            request_count: 20,
            seed: 42,
            scenario: Scenario::RogueInfraAgent,
        }));
        let engine = Arc::new(PolicyEngine::default());

        let first = execute_mode(&workload, 0, 1, Mode::Traxes, Some(engine.clone()));
        let second = execute_mode(&workload, 0, 1, Mode::Traxes, Some(engine));

        assert_eq!(first.total_requests, second.total_requests);
        assert_eq!(first.allow_count, second.allow_count);
        assert_eq!(first.deny_count, second.deny_count);
    }
}
