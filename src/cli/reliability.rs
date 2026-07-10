use traxes_demo::reliability_tests::{run_reliability_tests, generate_reliability_report};

pub fn run(args: &[String]) {
    let mut verbose = false;
    let mut output_report = true;
    
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--verbose" | "-v" => {
                verbose = true;
                i += 1;
            }
            "--no-report" => {
                output_report = false;
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
    
    let results = run_reliability_tests();
    
    if verbose {
        println!("\n📋 Detailed Results:");
        println!("===================");
        for result in &results.test_results {
            let status = if result.passed { "✅ PASS" } else { "❌ FAIL" };
            println!("{}: {} - {}", status, result.category, result.test_name);
            if let Some(ref error) = result.error_message {
                println!("  Error: {}", error);
            }
            println!("  Details: {}", result.details);
        }
    }
    
    if output_report {
        let report = generate_reliability_report(&results);
        let report_path = "reliability_test_report.md";
        std::fs::write(report_path, report).unwrap();
        println!("\n📄 Report saved to: {}", report_path);
    }
    
    // Exit with error code if any tests failed
    if results.failed_tests > 0 {
        std::process::exit(1);
    }
}

fn print_help() {
    println!(r#"
TRAXES Reliability Test Command

Usage: traxes-demo --dev reliability [OPTIONS]

Options:
  -v, --verbose     Show detailed test results
  --no-report       Skip generating the report file
  -h, --help        Show this help message

Description:
  Runs comprehensive reliability tests to verify TRAXES robustness
  against malformed inputs, policy failures, artifact integrity issues,
  and concurrent execution problems.

Test Categories:
  - Malformed Input: Invalid JSON, missing fields, wrong data types, empty payloads
  - Policy Failure: Missing policy, corrupted YAML, invalid policy syntax
  - Artifact Integrity: Missing fields, corrupted JSON, modified decision/hash
  - Concurrent Execution: Unique IDs, no corruption, stable replay

Examples:
  traxes-demo --dev reliability
  traxes-demo --dev reliability --verbose
  traxes-demo --dev reliability --no-report
"#);
}
