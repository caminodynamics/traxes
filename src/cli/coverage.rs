use std::fs;
use std::path::Path;
use crate::coverage::{CoverageRecord, CoverageStatus};

pub fn run(args: &[String]) {
    let subcommand = args.get(0).map(|s| s.as_str()).unwrap_or("summary");
    
    match subcommand {
        "summary" => print_summary(),
        "list" => list_records(),
        "show" => show_record(args.get(1)),
        _ => print_help(),
    }
}

fn print_summary() {
    let records = load_coverage_records();
    
    let total = records.len();
    let governed = records.iter().filter(|r| matches!(r.coverage_status, CoverageStatus::GOVERNED)).count();
    let ungoverned = records.iter().filter(|r| matches!(r.coverage_status, CoverageStatus::UNGOVERNED)).count();
    let unknown = records.iter().filter(|r| matches!(r.coverage_status, CoverageStatus::UNKNOWN)).count();
    
    println!("COVERAGE SUMMARY");
    println!("──────────────────────────────");
    println!("Total expected actions: {}", total);
    println!("Governed actions: {}", governed);
    println!("Ungoverned actions: {}", ungoverned);
    println!("Unknown actions: {}", unknown);
    
    if total > 0 {
        let coverage_percent = (governed as f64 / total as f64) * 100.0;
        println!("Coverage: {:.1}%", coverage_percent);
    }
}

fn list_records() {
    let records = load_coverage_records();
    
    println!("COVERAGE RECORDS");
    println!("──────────────────────────────");
    
    for record in records {
        println!("ID: {}", record.coverage_id);
        println!("Tool: {}", record.action_tool);
        println!("Status: {:?}", record.coverage_status);
        println!("Expected: {}", record.expected_control_path);
        println!("Observed: {}", record.observed_control_path);
        println!("Timestamp: {}", record.timestamp);
        println!("Decision ID: {:?}", record.decision_id);
        println!("──────────────────────────────");
    }
}

fn show_record(coverage_id: Option<&String>) {
    let id = match coverage_id {
        Some(id) => id,
        None => {
            eprintln!("Error: show command requires a coverage ID");
            eprintln!("Usage: traxes-demo --dev coverage show <coverage_id>");
            std::process::exit(1);
        }
    };
    
    let records = load_coverage_records();
    let record = records.iter().find(|r| r.coverage_id == *id);
    
    match record {
        Some(record) => {
            println!("{}", serde_json::to_string_pretty(record).unwrap());
        }
        None => {
            eprintln!("Error: Coverage record '{}' not found", id);
            std::process::exit(1);
        }
    }
}

fn load_coverage_records() -> Vec<CoverageRecord> {
    let coverage_dir = Path::new("coverage");
    
    if !coverage_dir.exists() {
        return vec![];
    }
    
    let mut records = vec![];
    
    if let Ok(entries) = fs::read_dir(coverage_dir) {
        for entry in entries.flatten() {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                if let Ok(record) = serde_json::from_str::<CoverageRecord>(&content) {
                    records.push(record);
                }
            }
        }
    }
    
    // Sort by timestamp (newest first)
    records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    records
}

fn print_help() {
    println!(r#"
TRAXES Coverage Commands:

  traxes-demo --dev coverage summary    Show coverage summary
  traxes-demo --dev coverage list        List all coverage records
  traxes-demo --dev coverage show <id>   Show specific coverage record
"#);
}
