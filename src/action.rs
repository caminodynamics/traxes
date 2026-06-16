use serde::{Deserialize, Serialize};
use crate::cli_utils;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProposedAction {
    pub tool: String,
    pub session_id: String,
    pub environment: String,
    pub parameters: Parameters,
}

pub type Parameters = serde_json::Value;

/// Single execution boundary for governance targets.
/// This function contains ALL governance target side effects (temp file writes).
/// Infrastructure operations (artifact generation, logging, policy loading, etc.) do NOT flow through this boundary.
pub fn execute(action: &ProposedAction, decision: &str, decision_id: &str) -> String {
    if decision == "ALLOW" {
        // Execute real action: write a temp file
        let temp_file_path = format!("temp_executed_{}.txt", decision_id);
        if let Err(e) = std::fs::write(&temp_file_path, format!("Action executed: {}", action.tool)) {
            cli_utils::debug_log(format!("[ENFORCEMENT] Failed to write temp file: {}", e));
        }
        "executed".to_string()
    } else {
        // DENY: block execution completely, no side effects
        "blocked".to_string()
    }
}
