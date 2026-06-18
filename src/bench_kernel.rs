use std::env;
use std::sync::Arc;
use std::time::Instant;
use traxes_demo::traxes_engine::Engine;
use traxes_demo::action::ProposedAction;
use std::thread;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    let iterations = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(1_000_000)
    } else {
        1_000_000
    };
    
    let threads = if args.len() > 2 {
        args[2].parse::<usize>().unwrap_or(1)
    } else {
        1
    };
    
    let warmup_iterations = 10_000;
    
    println!("========================================================================");
    println!("TRAXES KERNEL BENCHMARK - Pure Engine Evaluation");
    println!("========================================================================");
    println!("Iterations: {}", iterations);
    println!("Threads: {}", threads);
    println!("Warm-up: {} iterations", warmup_iterations);
    println!();
    
    // Load engine (this happens once, not measured)
    println!("Loading engine...");
    let engine = Arc::new(Engine::load_default_policies().expect("Failed to load engine"));
    println!("Engine loaded successfully.");
    println!();
    
    // Create test action
    let action = ProposedAction {
        tool: "aws.rds.provision_db".to_string(),
        session_id: "bench_session_001".to_string(),
        environment: "staging".to_string(),
        parameters: serde_json::json!({
            "instance_type": "db.t3.micro",
            "instance_cost_per_hour": 0.03
        }),
    };
    
    if threads == 1 {
        run_single_threaded(engine, action, iterations, warmup_iterations);
    } else {
        run_multi_threaded(engine, action, iterations, warmup_iterations, threads);
    }
}

fn run_single_threaded(
    engine: Arc<Engine>,
    action: ProposedAction,
    iterations: usize,
    warmup_iterations: usize,
) {
    println!("Running single-threaded benchmark...");
    println!();
    
    // Warm-up phase
    println!("Warm-up phase ({} iterations)...", warmup_iterations);
    for _ in 0..warmup_iterations {
        let _ = engine.evaluate(&action);
    }
    println!("Warm-up complete.");
    println!();
    
    // Benchmark phase
    println!("Benchmark phase ({} iterations)...", iterations);
    let mut latencies_us: Vec<f64> = Vec::with_capacity(iterations);
    
    let start = Instant::now();
    for _ in 0..iterations {
        let eval_start = Instant::now();
        let _ = engine.evaluate(&action);
        let latency_us = eval_start.elapsed().as_micros() as f64;
        latencies_us.push(latency_us);
    }
    let total_duration = start.elapsed();
    
    println!("Benchmark complete.");
    println!();
    
    // Calculate statistics
    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let p50_idx = latencies_us.len() / 2;
    let p95_idx = (latencies_us.len() as f64 * 0.95) as usize;
    let p99_idx = (latencies_us.len() as f64 * 0.99) as usize;
    
    let p50_us = latencies_us[p50_idx];
    let p95_us = latencies_us[p95_idx];
    let p99_us = latencies_us[p99_idx];
    
    let avg_us: f64 = latencies_us.iter().sum::<f64>() / latencies_us.len() as f64;
    let min_us = latencies_us[0];
    let max_us = latencies_us[latencies_us.len() - 1];
    
    let throughput = iterations as f64 / total_duration.as_secs_f64();
    
    // Print results
    println!("========================================================================");
    println!("RESULTS - Single-Threaded");
    println!("========================================================================");
    println!("Total iterations: {}", iterations);
    println!("Total duration: {:.2}s", total_duration.as_secs_f64());
    println!("Throughput: {:.2} ops/sec", throughput);
    println!();
    println!("Latency (microseconds):");
    println!("  P50: {:.2} us ({:.4} ms)", p50_us, p50_us / 1000.0);
    println!("  P95: {:.2} us ({:.4} ms)", p95_us, p95_us / 1000.0);
    println!("  P99: {:.2} us ({:.4} ms)", p99_us, p99_us / 1000.0);
    println!("  Avg: {:.2} us ({:.4} ms)", avg_us, avg_us / 1000.0);
    println!("  Min: {:.2} us ({:.4} ms)", min_us, min_us / 1000.0);
    println!("  Max: {:.2} us ({:.4} ms)", max_us, max_us / 1000.0);
    println!("========================================================================");
}

fn run_multi_threaded(
    engine: Arc<Engine>,
    action: ProposedAction,
    iterations: usize,
    warmup_iterations: usize,
    num_threads: usize,
) {
    println!("Running multi-threaded benchmark with {} threads...", num_threads);
    println!();
    
    let iterations_per_thread = iterations / num_threads;
    let warmup_per_thread = warmup_iterations / num_threads;
    
    println!("Iterations per thread: {}", iterations_per_thread);
    println!("Warm-up per thread: {}", warmup_per_thread);
    println!();
    
    // Warm-up phase (all threads)
    println!("Warm-up phase...");
    let mut handles = vec![];
    for _ in 0..num_threads {
        let engine_clone = Arc::clone(&engine);
        let action_clone = action.clone();
        let handle = thread::spawn(move || {
            for _ in 0..warmup_per_thread {
                let _ = engine_clone.evaluate(&action_clone);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().expect("Thread panicked during warm-up");
    }
    println!("Warm-up complete.");
    println!();
    
    // Benchmark phase
    println!("Benchmark phase...");
    let mut handles = vec![];
    let mut latencies_us: Vec<f64> = Vec::with_capacity(iterations);
    
    let start = Instant::now();
    for _ in 0..num_threads {
        let engine_clone = Arc::clone(&engine);
        let action_clone = action.clone();
        let handle = thread::spawn(move || {
            let mut local_latencies: Vec<f64> = Vec::with_capacity(iterations_per_thread);
            for _ in 0..iterations_per_thread {
                let eval_start = Instant::now();
                let _ = engine_clone.evaluate(&action_clone);
                let latency_us = eval_start.elapsed().as_micros() as f64;
                local_latencies.push(latency_us);
            }
            local_latencies
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let thread_latencies = handle.join().expect("Thread panicked during benchmark");
        latencies_us.extend(thread_latencies);
    }
    let total_duration = start.elapsed();
    
    println!("Benchmark complete.");
    println!();
    
    // Calculate statistics
    latencies_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
    let p50_idx = latencies_us.len() / 2;
    let p95_idx = (latencies_us.len() as f64 * 0.95) as usize;
    let p99_idx = (latencies_us.len() as f64 * 0.99) as usize;
    
    let p50_us = latencies_us[p50_idx];
    let p95_us = latencies_us[p95_idx];
    let p99_us = latencies_us[p99_idx];
    
    let avg_us: f64 = latencies_us.iter().sum::<f64>() / latencies_us.len() as f64;
    let min_us = latencies_us[0];
    let max_us = latencies_us[latencies_us.len() - 1];
    
    let throughput = iterations as f64 / total_duration.as_secs_f64();
    
    // Print results
    println!("========================================================================");
    println!("RESULTS - Multi-Threaded ({} threads)", num_threads);
    println!("========================================================================");
    println!("Total iterations: {}", iterations);
    println!("Total duration: {:.2}s", total_duration.as_secs_f64());
    println!("Throughput: {:.2} ops/sec", throughput);
    println!("Throughput per thread: {:.2} ops/sec", throughput / num_threads as f64);
    println!();
    println!("Latency (microseconds):");
    println!("  P50: {:.2} us ({:.4} ms)", p50_us, p50_us / 1000.0);
    println!("  P95: {:.2} us ({:.4} ms)", p95_us, p95_us / 1000.0);
    println!("  P99: {:.2} us ({:.4} ms)", p99_us, p99_us / 1000.0);
    println!("  Avg: {:.2} us ({:.4} ms)", avg_us, avg_us / 1000.0);
    println!("  Min: {:.2} us ({:.4} ms)", min_us, min_us / 1000.0);
    println!("  Max: {:.2} us ({:.4} ms)", max_us, max_us / 1000.0);
    println!("========================================================================");
}
