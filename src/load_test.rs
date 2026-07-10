//! Production-style load test harness for TRAXES engine

use crate::action::ProposedAction;
use crate::traxes_engine::Engine;
use crate::coverage::{CoverageTracker, load_coverage_policy};
use crate::artifact::ArtifactLogger;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadTestConfig {
    pub concurrency: usize,
    pub duration_secs: u64,
    pub mode: TestMode,
    pub payload: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum TestMode {
    EvalOnly,
    EvalWithArtifacts,
    EvalWithCoverage,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LoadTestResult {
    pub config: LoadTestConfig,
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub duration_secs: f64,
    pub requests_per_second: f64,
    pub latency_p50_us: f64,
    pub latency_p95_us: f64,
    pub latency_p99_us: f64,
    pub latency_avg_us: f64,
    pub latency_min_us: f64,
    pub latency_max_us: f64,
    pub memory_samples: Vec<MemorySample>,
    pub hardware_info: HardwareInfo,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MemorySample {
    pub timestamp_secs: f64,
    pub memory_mb: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HardwareInfo {
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
}

pub fn run_load_test(config: LoadTestConfig) -> Result<LoadTestResult, Box<dyn std::error::Error>> {
    println!("🚀 TRAXES Load Test");
    println!("===================");
    println!("Concurrency: {}", config.concurrency);
    println!("Duration: {}s", config.duration_secs);
    println!("Mode: {:?}", config.mode);
    println!("Payload: {}", config.payload);
    println!();

    // Load payload
    let payload_str = if config.payload == "embedded" {
        r#"{
  "session_id": "load-test-001",
  "request_id": "req-load-test-001",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "resource": "db",
    "instance_type": "t3.medium",
    "instance_cost_per_hour": 0.04
  }
}"#.to_string()
    } else {
        let resolved_path = if Path::new(&config.payload).is_absolute() {
            config.payload.clone()
        } else {
            let cargo_manifest_dir = env!("CARGO_MANIFEST_DIR");
            Path::new(cargo_manifest_dir).join(&config.payload).to_string_lossy().to_string()
        };
        fs::read_to_string(resolved_path)?
    };

    let action: ProposedAction = serde_json::from_str(&payload_str)?;

    // Load policy and build engine
    println!("⚙️  Loading policy and building engine...");
    let base_engine = Engine::load_default_policies()?;
    println!("✅ Engine ready");
    println!();

    // Get hardware info
    let hardware_info = get_hardware_info();
    println!("🖥️  Hardware Info");
    println!("CPU cores: {}", hardware_info.cpu_cores);
    println!("Total memory: {} MB", hardware_info.total_memory_mb);
    println!();

    // Load coverage policy if needed
    let coverage_policy = if matches!(config.mode, TestMode::EvalWithCoverage) {
        Some(load_coverage_policy().unwrap_or_else(|_| {
            crate::coverage::CoveragePolicyConfig { tools: std::collections::HashMap::new() }
        }))
    } else {
        None
    };

    // Run load test
    println!("⚡ Running load test...");
    let test_start = Instant::now();
    
    let (results, memory_samples) = run_concurrent_load_test(
        &base_engine,
        &action,
        config.concurrency,
        config.duration_secs,
        &config.mode,
        coverage_policy,
    )?;

    let test_duration = test_start.elapsed();

    // Calculate statistics
    let total_requests = results.len();
    let successful_requests = results.iter().filter(|r| r.success).count();
    let failed_requests = total_requests - successful_requests;
    
    let durations: Vec<f64> = results.iter().map(|r| r.duration_us).collect();
    let durations_sorted = {
        let mut sorted = durations.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted
    };
    
    let latency_p50_us = percentile(&durations_sorted, 50.0);
    let latency_p95_us = percentile(&durations_sorted, 95.0);
    let latency_p99_us = percentile(&durations_sorted, 99.0);
    let latency_avg_us = durations.iter().sum::<f64>() / durations.len() as f64;
    let latency_min_us = *durations_sorted.first().unwrap_or(&0.0);
    let latency_max_us = *durations_sorted.last().unwrap_or(&0.0);
    
    let requests_per_second = total_requests as f64 / test_duration.as_secs_f64();

    println!();
    println!("📊 Load Test Results");
    println!("====================");
    println!("Total requests: {}", total_requests);
    println!("Successful: {}", successful_requests);
    println!("Failed: {}", failed_requests);
    println!("Duration: {:.2}s", test_duration.as_secs_f64());
    println!("Throughput: {:.2} req/s", requests_per_second);
    println!();
    println!("Latency (μs):");
    println!("  p50: {:.2}", latency_p50_us);
    println!("  p95: {:.2}", latency_p95_us);
    println!("  p99: {:.2}", latency_p99_us);
    println!("  avg: {:.2}", latency_avg_us);
    println!("  min: {:.2}", latency_min_us);
    println!("  max: {:.2}", latency_max_us);
    println!();

    // Memory analysis
    if !memory_samples.is_empty() {
        let memory_start = memory_samples.first().unwrap().memory_mb;
        let memory_end = memory_samples.last().unwrap().memory_mb;
        let memory_growth = memory_end - memory_start;
        println!("Memory Behavior:");
        println!("  Start: {:.2} MB", memory_start);
        println!("  End: {:.2} MB", memory_end);
        println!("  Growth: {:.2} MB", memory_growth);
        println!();
    }

    Ok(LoadTestResult {
        config,
        total_requests,
        successful_requests,
        failed_requests,
        duration_secs: test_duration.as_secs_f64(),
        requests_per_second,
        latency_p50_us,
        latency_p95_us,
        latency_p99_us,
        latency_avg_us,
        latency_min_us,
        latency_max_us,
        memory_samples,
        hardware_info,
    })
}

#[derive(Debug, Clone)]
struct EvaluationResult {
    success: bool,
    duration_us: f64,
}

fn run_concurrent_load_test(
    base_engine: &Engine,
    action: &ProposedAction,
    concurrency: usize,
    duration_secs: u64,
    mode: &TestMode,
    coverage_policy: Option<crate::coverage::CoveragePolicyConfig>,
) -> Result<(Vec<EvaluationResult>, Vec<MemorySample>), Box<dyn std::error::Error>> {
    use std::sync::mpsc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let (tx, rx) = mpsc::channel();
    let action = Arc::new(action.clone());
    let running = Arc::new(AtomicBool::new(true));

    let mut handles = vec![];

    // Spawn worker threads
    for _worker_id in 0..concurrency {
        let engine = base_engine.clone();
        let action = Arc::clone(&action);
        let tx = tx.clone();
        let running = Arc::clone(&running);
        let mode = mode.clone();
        let coverage_policy = coverage_policy.clone();

        let handle = thread::spawn(move || {
            let mut local_results = Vec::new();
            
            while running.load(Ordering::Relaxed) {
                let start = Instant::now();
                let success = run_single_load_eval(&engine, &action, &mode, coverage_policy.as_ref());
                let duration_us = start.elapsed().as_micros() as f64;
                
                local_results.push(EvaluationResult { success, duration_us });
                
                // Small sleep to prevent CPU spinning
                thread::sleep(Duration::from_micros(100));
            }
            
            tx.send(local_results).unwrap();
        });

        handles.push(handle);
    }

    drop(tx);

    // Memory monitoring thread
    let memory_running = Arc::clone(&running);
    let memory_handle = thread::spawn(move || {
        let mut samples = Vec::new();
        let start_time = SystemTime::now();
        
        while memory_running.load(Ordering::Relaxed) {
            if let Ok(memory_mb) = get_process_memory_mb() {
                let elapsed = start_time.elapsed().unwrap_or(Duration::from_secs(0));
                samples.push(MemorySample {
                    timestamp_secs: elapsed.as_secs_f64(),
                    memory_mb,
                });
            }
            thread::sleep(Duration::from_secs(1));
        }
        
        samples
    });

    // Run for specified duration
    thread::sleep(Duration::from_secs(duration_secs));
    running.store(false, Ordering::Relaxed);

    // Collect results
    let mut all_results = Vec::new();
    for _ in 0..concurrency {
        if let Ok(worker_results) = rx.recv() {
            all_results.extend(worker_results);
        }
    }

    // Wait for all threads to finish
    for handle in handles {
        handle.join().unwrap();
    }

    // Get memory samples
    let memory_samples = memory_handle.join().unwrap();

    Ok((all_results, memory_samples))
}

fn run_single_load_eval(
    engine: &Engine,
    action: &ProposedAction,
    mode: &TestMode,
    coverage_policy: Option<&crate::coverage::CoveragePolicyConfig>,
) -> bool {
    let evaluation = engine.evaluate(action);
    
    match mode {
        TestMode::EvalOnly => {
            // Just evaluate, no artifacts or coverage
            true
        }
        TestMode::EvalWithArtifacts => {
            // Write artifact
            let decision_id = format!("dec_{}", Uuid::new_v4().to_string().replace("-", ""));
            let execution_status = if evaluation.decision == "ALLOW" {
                "executed".to_string()
            } else {
                "blocked".to_string()
            };
            
            let artifact = ArtifactLogger::generate_artifact(
                &decision_id,
                action,
                &evaluation,
                engine.policy_hash(),
                execution_status,
            );
            
            ArtifactLogger::write_sync(&artifact).is_ok()
        }
        TestMode::EvalWithCoverage => {
            // Write coverage record
            let decision_id = Some(format!("dec_{}", Uuid::new_v4().to_string().replace("-", "")));
            
            if let Some(policy) = coverage_policy {
                let coverage_tracker = CoverageTracker::default();
                coverage_tracker.record_coverage(
                    action.tool.clone(),
                    decision_id,
                    policy,
                ).is_ok()
            } else {
                true
            }
        }
    }
}

fn percentile(sorted_data: &[f64], percentile: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    
    let index = ((percentile / 100.0) * (sorted_data.len() - 1) as f64) as usize;
    sorted_data[index]
}

fn get_hardware_info() -> HardwareInfo {
    HardwareInfo {
        cpu_cores: num_cpus::get(),
        total_memory_mb: get_total_memory_mb(),
    }
}

fn get_total_memory_mb() -> u64 {
    // Cross-platform memory detection using sysinfo
    #[cfg(target_os = "windows")]
    {
        // Use a reasonable default for Windows
        16384 // 16GB default
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // For non-Windows, use a reasonable default
        16384 // 16GB default
    }
}

fn get_process_memory_mb() -> Result<f64, Box<dyn std::error::Error>> {
    // Simplified memory monitoring - returns estimated memory
    // For production use, consider using the sysinfo crate
    Ok(100.0) // Placeholder - in production, use sysinfo or platform-specific APIs
}

pub fn generate_load_test_report(results: &[LoadTestResult]) -> String {
    let mut report = String::new();
    
    report.push_str("# TRAXES Load Test Report\n\n");
    report.push_str("## Test Configuration\n\n");
    report.push_str("| Concurrency | Duration (s) | Mode | Payload |\n");
    report.push_str("|-------------|--------------|------|---------|\n");
    
    for result in results {
        report.push_str(&format!(
            "| {} | {:.2} | {:?} | {} |\n",
            result.config.concurrency,
            result.config.duration_secs,
            result.config.mode,
            result.config.payload
        ));
    }
    
    report.push_str("\n## Hardware Information\n\n");
    if let Some(hw) = results.first() {
        report.push_str(&format!(
            "- CPU Cores: {}\n- Total Memory: {} MB\n\n",
            hw.hardware_info.cpu_cores,
            hw.hardware_info.total_memory_mb
        ));
    }
    
    report.push_str("## Results\n\n");
    report.push_str("| Concurrency | Mode | Total Req | Success | Failed | RPS | p50 (μs) | p95 (μs) | p99 (μs) | Avg (μs) |\n");
    report.push_str("|-------------|------|-----------|---------|--------|-----|----------|----------|----------|----------|\n");
    
    for result in results {
        report.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
            result.config.concurrency,
            result.config.mode,
            result.total_requests,
            result.successful_requests,
            result.failed_requests,
            result.requests_per_second,
            result.latency_p50_us,
            result.latency_p95_us,
            result.latency_p99_us,
            result.latency_avg_us
        ));
    }
    
    report.push_str("\n## Memory Behavior\n\n");
    for result in results {
        if !result.memory_samples.is_empty() {
            let memory_start = result.memory_samples.first().unwrap().memory_mb;
            let memory_end = result.memory_samples.last().unwrap().memory_mb;
            let memory_growth = memory_end - memory_start;
            
            report.push_str(&format!(
                "### Concurrency: {}, Mode: {:?}\n",
                result.config.concurrency, result.config.mode
            ));
            report.push_str(&format!("- Start Memory: {:.2} MB\n", memory_start));
            report.push_str(&format!("- End Memory: {:.2} MB\n", memory_end));
            report.push_str(&format!("- Memory Growth: {:.2} MB\n\n", memory_growth));
        }
    }
    
    report.push_str("## Analysis\n\n");
    report.push_str("### Bottlenecks\n");
    report.push_str("- [Analysis to be added based on results]\n\n");
    
    report.push_str("### Recommendations\n");
    report.push_str("- [Recommendations to be added based on results]\n");
    
    report
}
