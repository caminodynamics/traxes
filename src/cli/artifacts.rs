use std::fs;
use std::path::Path;

pub fn run(args: &[String]) {
    let cmd = args.get(0).map(|s| s.as_str()).unwrap_or("list");

    match cmd {
        "list" => list(),
        "last" => last(false),
        "show" => show(args.get(1)),
        _ => list(),
    }
}

pub fn list() {
    let content = fs::read_to_string("artifacts/index.json").unwrap_or_default();

    println!("RECENT ARTIFACTS:");
    println!("{}", content);
}

pub fn last(full: bool) {
    let artifacts_dir = Path::new("artifacts");

    if !artifacts_dir.exists() {
        println!("No artifacts found");
        return;
    }

    let mut json_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = fs::read_dir(artifacts_dir)
        .unwrap_or_else(|_| {
            println!("No artifacts found");
            std::process::exit(1);
        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "json"))
        .filter(|entry| {
            entry.file_name()
                .to_string_lossy()
                .starts_with("AuditArtifact_")
        })
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = path.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    if json_files.is_empty() {
        println!("No artifacts found");
        return;
    }

    json_files.sort_by(|a, b| b.1.cmp(&a.1));

    let (latest_path, _) = &json_files[0];

    if full {
        println!("{}", latest_path.display());
        match fs::read_to_string(latest_path) {
            Ok(content) => {
                println!("{}", content);
            }
            Err(e) => {
                println!("Failed to read artifact: {}", e);
            }
        }
    } else {
        match fs::read_to_string(latest_path) {
            Ok(content) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    let decision = json.get("decision").and_then(|d| d.as_str()).unwrap_or("unknown");
                    let _tool = json.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
                    let policy_bundle = json.get("policy_bundle").and_then(|p| p.as_str()).unwrap_or("unknown");
                    let _artifact_id = json.get("decision_id").and_then(|d| d.as_str()).unwrap_or("unknown");
                    let artifact_name = latest_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

                    println!("decision: {}", decision);
                    println!("policy: {}", policy_bundle);
                    println!("artifact: {}", artifact_name);
                } else {
                    println!("Failed to parse artifact JSON");
                }
            }
            Err(e) => {
                println!("Failed to read artifact: {}", e);
            }
        }
    }
}

pub fn show(id: Option<&String>) {
    println!("SHOW ARTIFACT: {:?}", id);
}

pub fn last_demo_summary() {
    let artifacts_dir = Path::new("artifacts");

    if !artifacts_dir.exists() {
        println!("No artifacts found");
        return;
    }

    let mut json_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = fs::read_dir(artifacts_dir)
        .unwrap_or_else(|_| {
            println!("No artifacts found");
            std::process::exit(1);
        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "json"))
        .filter(|entry| {
            entry.file_name()
                .to_string_lossy()
                .starts_with("AuditArtifact_")
        })
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = path.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    if json_files.is_empty() {
        println!("No artifacts found");
        return;
    }

    json_files.sort_by(|a, b| b.1.cmp(&a.1));

    let (latest_path, _) = &json_files[0];

    match fs::read_to_string(latest_path) {
        Ok(content) => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let decision = json.get("decision").and_then(|d| d.as_str()).unwrap_or("unknown");
                let tool = json.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
                let policy_bundle = json.get("policy_bundle").and_then(|p| p.as_str()).unwrap_or("unknown");
                let timestamp = json.get("timestamp").and_then(|t| t.as_str()).unwrap_or("unknown");
                let artifact_name = latest_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

                println!("decision: {}", decision);
                println!("tool: {}", tool);
                println!("policy: {}", policy_bundle);
                println!("artifact: {}", artifact_name);
                println!("timestamp: {}", timestamp);
            } else {
                println!("Failed to parse artifact JSON");
            }
        }
        Err(e) => {
            println!("Failed to read artifact: {}", e);
        }
    }
}

pub fn last_demo_inspection() {
    let artifacts_dir = Path::new("artifacts");

    if !artifacts_dir.exists() {
        println!("No artifacts found");
        return;
    }

    let mut json_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = fs::read_dir(artifacts_dir)
        .unwrap_or_else(|_| {
            println!("No artifacts found");
            std::process::exit(1);
        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "json"))
        .filter(|entry| {
            entry.file_name()
                .to_string_lossy()
                .starts_with("AuditArtifact_")
        })
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = path.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    if json_files.is_empty() {
        println!("No artifacts found");
        return;
    }

    json_files.sort_by(|a, b| b.1.cmp(&a.1));

    let (latest_path, _) = &json_files[0];

    match fs::read_to_string(latest_path) {
        Ok(content) => {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let decision = json.get("decision").and_then(|d| d.as_str()).unwrap_or("unknown");
                let tool = json.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown");
                let environment = json.get("environment").and_then(|e| e.as_str()).unwrap_or("unknown");
                let reason = json.get("reason").and_then(|r| r.as_str()).unwrap_or("unknown");
                let policy_bundle = json.get("policy_bundle").and_then(|p| p.as_str()).unwrap_or("unknown");
                let policy_hash = json.get("policy_hash").and_then(|p| p.as_str()).unwrap_or("unknown");
                let execution_status = json.get("execution_status").and_then(|e| e.as_str()).unwrap_or("unknown");
                let timestamp = json.get("timestamp").and_then(|t| t.as_str()).unwrap_or("unknown");

                println!("ARTIFACT INSPECTION");
                println!();
                println!("decision: {}", decision);
                println!();
                println!("tool: {}", tool);
                println!();
                println!("environment: {}", environment);
                println!();
                println!("reason: {}", reason);
                println!();
                println!("policy: {}", policy_bundle);
                println!();
                println!("policy_hash: {}", policy_hash);
                println!();
                println!("execution_status: {}", execution_status);
                println!();
                println!("timestamp: {}", timestamp);
            } else {
                println!("Failed to parse artifact JSON");
            }
        }
        Err(e) => {
            println!("Failed to read artifact: {}", e);
        }
    }
}
