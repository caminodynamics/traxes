use std::fs::OpenOptions;
use std::io::Write;
use serde_json::json;

#[allow(dead_code)]
pub fn record_artifact(id: &str, decision: &str, path: &str) {
    let entry = json!({
        "id": id,
        "decision": decision,
        "path": path,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("artifacts/index.json")
        .unwrap();

    writeln!(file, "{}", entry).unwrap();
}
