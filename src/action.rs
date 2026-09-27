use crate::cli_utils;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProposedAction {
    pub tool: String,
    pub session_id: String,
    pub environment: String,
    pub parameters: Parameters,
}

pub type Parameters = serde_json::Value;

mod file_write;

/// Single execution boundary for governance targets.
/// This function contains ALL governance target side effects.
/// Infrastructure operations (artifact generation, logging, policy loading, etc.) do NOT flow through this boundary.
///
/// Execution sequence:
/// 1. Evaluate action (TRAXES policy check happens before this function)
/// 2. If ALLOW, perform target action (write or other operation) and confirm success
/// 3. If DENY, perform no target action
/// 4. Return execution status: "executed" (only if write succeeded) or "blocked"
/// 5. Caller generates/writes artifact with the returned execution status
pub fn execute(action: &ProposedAction, decision: &str, decision_id: &str) -> String {
    if decision == "ALLOW" {
        // Handle FILE_WRITE action: write to the requested target path
        if action.tool == "FILE_WRITE" {
            // Extract path and content from action parameters
            let target_path = match action.parameters.get("path") {
                Some(path_val) => match path_val.as_str() {
                    Some(p) => p.to_string(),
                    None => {
                        cli_utils::debug_log(
                            "[ENFORCEMENT] FILE_WRITE: path parameter is not a string".to_string(),
                        );
                        return "blocked".to_string();
                    }
                },
                None => {
                    cli_utils::debug_log(
                        "[ENFORCEMENT] FILE_WRITE: missing 'path' parameter".to_string(),
                    );
                    return "blocked".to_string();
                }
            };

            let content = match action.parameters.get("content") {
                Some(content_val) => match content_val.as_str() {
                    Some(c) => c.to_string(),
                    None => {
                        cli_utils::debug_log(
                            "[ENFORCEMENT] FILE_WRITE: content parameter is not a string"
                                .to_string(),
                        );
                        return "blocked".to_string();
                    }
                },
                None => {
                    cli_utils::debug_log(
                        "[ENFORCEMENT] FILE_WRITE: missing 'content' parameter".to_string(),
                    );
                    return "blocked".to_string();
                }
            };

            // Resolve without following redirects, then write the validated handle.
            match file_write::write(std::path::Path::new(&target_path), content.as_bytes()) {
                Ok(_) => {
                    cli_utils::debug_log(format!(
                        "[ENFORCEMENT] FILE_WRITE executed successfully: {}",
                        target_path
                    ));
                    "executed".to_string()
                }
                Err(e) => {
                    cli_utils::debug_log(format!(
                        "[ENFORCEMENT] FILE_WRITE failed to write {}: {}",
                        target_path, e
                    ));
                    "blocked".to_string()
                }
            }
        } else {
            // For non-FILE_WRITE actions (e.g., AWS_RDS_PROVISION), use a marker file for now
            // This preserves existing behavior for other tools
            let temp_file_path = format!("temp_executed_{}.txt", decision_id);
            match std::fs::write(&temp_file_path, format!("Action executed: {}", action.tool)) {
                Ok(_) => "executed".to_string(),
                Err(e) => {
                    cli_utils::debug_log(format!(
                        "[ENFORCEMENT] Failed to write marker file: {}",
                        e
                    ));
                    "blocked".to_string()
                }
            }
        }
    } else {
        // DENY: block execution completely, no side effects
        "blocked".to_string()
    }
}
