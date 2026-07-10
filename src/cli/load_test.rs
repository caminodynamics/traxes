use traxes_demo::load_test::{run_load_test, generate_load_test_report, LoadTestConfig, TestMode};

pub fn run(args: &[String]) {
    let mut concurrency = 10;
    let mut duration_secs = 60;
    let mut mode: TestMode = TestMode::EvalOnly;
    let mut payload = "embedded".to_string();
    let mut run_comparison = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--concurrency" | "-c" => {
                if i + 1 < args.len() {
                    concurrency = args[i + 1].parse().unwrap_or(10);
                    i += 2;
                } else {
                    eprintln!("Error: --concurrency requires a value");
                    std::process::exit(1);
                }
            }
            "--duration" | "-d" => {
                if i + 1 < args.len() {
                    duration_secs = args[i + 1].parse().unwrap_or(60);
                    i += 2;
                } else {
                    eprintln!("Error: --duration requires a value");
                    std::process::exit(1);
                }
            }
            "--mode" | "-m" => {
                if i + 1 < args.len() {
                    mode = match args[i + 1].as_str() {
                        "eval-only" => TestMode::EvalOnly,
                        "with-artifacts" => TestMode::EvalWithArtifacts,
                        "with-coverage" => TestMode::EvalWithCoverage,
                        _ => {
                            eprintln!("Error: Invalid mode. Use: eval-only, with-artifacts, with-coverage");
                            std::process::exit(1);
                        }
                    };
                    i += 2;
                } else {
                    eprintln!("Error: --mode requires a value");
                    std::process::exit(1);
                }
            }
            "--payload" | "-p" => {
                if i + 1 < args.len() {
                    payload = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --payload requires a value");
                    std::process::exit(1);
                }
            }
            "--compare" => {
                run_comparison = true;
                i += 1;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            _ => {
                eprintln!("Error: Unknown argument: {}", args[i]);
                print_help();
                std::process::exit(1);
            }
        }
    }

    if run_comparison {
        run_comparison_test(concurrency, duration_secs, &payload);
    } else {
        let config = LoadTestConfig {
            concurrency,
            duration_secs,
            mode,
            payload,
        };

        match run_load_test(config) {
            Ok(result) => {
                // Save result to file
                let results = vec![result];
                let report = generate_load_test_report(&results);
                let report_path = "load_test_report.md";
                std::fs::write(report_path, report).unwrap();
                println!("Report saved to: {}", report_path);
            }
            Err(e) => {
                eprintln!("Error running load test: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn run_comparison_test(concurrency: usize, duration_secs: u64, payload: &str) {
    println!("🔄 Running comparison test across all modes...\n");
    
    let modes = vec![
        TestMode::EvalOnly,
        TestMode::EvalWithArtifacts,
        TestMode::EvalWithCoverage,
    ];
    
    let mut results = Vec::new();
    
    for mode in modes {
        println!("Testing mode: {:?}", mode);
        let test_mode = mode.clone();
        let config = LoadTestConfig {
            concurrency,
            duration_secs,
            mode: test_mode,
            payload: payload.to_string(),
        };
        
        match run_load_test(config) {
            Ok(result) => {
                results.push(result);
                println!("✅ Mode {:?} completed\n", mode);
            }
            Err(e) => {
                eprintln!("❌ Mode {:?} failed: {}\n", mode, e);
            }
        }
    }
    
    if !results.is_empty() {
        let report = generate_load_test_report(&results);
        let report_path = "load_test_comparison_report.md";
        std::fs::write(report_path, report).unwrap();
        println!("Comparison report saved to: {}", report_path);
    }
}

fn print_help() {
    println!(r#"
TRAXES Load Test Command

Usage: traxes-demo --dev load-test [OPTIONS]

Options:
  -c, --concurrency <N>     Number of concurrent workers (default: 10)
  -d, --duration <SECONDS> Test duration in seconds (default: 60)
  -m, --mode <MODE>        Test mode: eval-only, with-artifacts, with-coverage (default: eval-only)
  -p, --payload <PATH>     Payload file path or 'embedded' (default: embedded)
  --compare                Run comparison test across all modes
  -h, --help               Show this help message

Examples:
  traxes-demo --dev load-test
  traxes-demo --dev load-test -c 50 -d 300 -m with-artifacts
  traxes-demo --dev load-test --compare
"#);
}
