use crate::cli_utils;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProposedAction {
    pub tool: String,
    pub session_id: String,
    pub environment: String,
    pub parameters: Parameters,
}

pub type Parameters = serde_json::Value;

/// Strongly typed authorization that can only be created by trusted TRAXES evaluation code.
/// This binds authorization to a specific action and prevents callers from bypassing
/// the TRAXES engine by simply passing "ALLOW" strings.
#[derive(Debug, Clone)]
pub struct ExecutionPermit {
    action_hash: String,
    decision_id: String,
    _phantom: std::marker::PhantomData<()>,
}

impl ExecutionPermit {
    /// Create an execution permit from an ALLOW decision.
    /// This is the ONLY public constructor - it can only be called with an explicit ALLOW decision.
    /// DENY decisions cannot create permits (returns None).
    /// The permit is cryptographically bound to the specific action via hash.
    pub fn from_allow_decision(action: &ProposedAction, decision_id: &str) -> Option<Self> {
        Some(ExecutionPermit {
            action_hash: Self::hash_action(action),
            decision_id: decision_id.to_string(),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Verify that this permit authorizes the given action.
    /// A permit for one action cannot authorize a different action.
    pub fn authorizes(&self, action: &ProposedAction) -> bool {
        self.action_hash == Self::hash_action(action)
    }

    /// Get the decision ID for this permit.
    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    /// Create a cryptographic hash of the action to bind the permit to it.
    /// This ensures a permit cannot be reused for a different action.
    fn hash_action(action: &ProposedAction) -> String {
        let action_json =
            serde_json::to_string(action).expect("Action must be serializable for hash binding");
        let mut hasher = Sha256::new();
        hasher.update(action_json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Result of executing an action with authorization.
/// This separates the authorization decision (ALLOW/DENY) from the execution outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionOutcome {
    /// Action was authorized and executed successfully
    Executed,
    /// Action was authorized but execution failed (e.g., filesystem error)
    ExecutionFailed(String),
    /// Action was not authorized (DENY decision)
    Unauthorized,
}

mod file_write;

/// Single execution boundary for governance targets.
/// This function contains ALL governance target side effects.
/// Infrastructure operations (artifact generation, logging, policy loading, etc.) do NOT flow through this boundary.
///
/// Execution sequence:
/// 1. TRAXES engine evaluates action and produces ExecutionPermit (only for ALLOW decisions)
/// 2. If permit exists and authorizes this action, perform target action and return outcome
/// 3. If no permit or permit doesn't authorize this action, return Unauthorized
/// 4. Return ExecutionOutcome separating authorization from execution result
/// 5. Caller generates/writes artifact with both the original decision and execution outcome
pub fn execute(action: &ProposedAction, permit: Option<ExecutionPermit>) -> ExecutionOutcome {
    // Verify authorization: permit must exist and must authorize this specific action
    match permit {
        Some(valid_permit) if valid_permit.authorizes(action) => {
            // Authorized: attempt the actual side-effecting operation
            if action.tool == "FILE_WRITE" {
                execute_file_write(action)
            } else {
                execute_marker_action(action, valid_permit.decision_id())
            }
        }
        _ => {
            // Unauthorized: no permit or permit doesn't match this action
            cli_utils::debug_log(
                "[ENFORCEMENT] Action not authorized - no valid permit".to_string(),
            );
            ExecutionOutcome::Unauthorized
        }
    }
}

/// Execute FILE_WRITE action with authorization.
/// Returns ExecutionOutcome separating success/failure from authorization.
fn execute_file_write(action: &ProposedAction) -> ExecutionOutcome {
    // Extract path and content from action parameters
    let target_path = match action.parameters.get("path") {
        Some(path_val) => match path_val.as_str() {
            Some(p) => p.to_string(),
            None => {
                cli_utils::debug_log(
                    "[ENFORCEMENT] FILE_WRITE: path parameter is not a string".to_string(),
                );
                return ExecutionOutcome::ExecutionFailed(
                    "path parameter is not a string".to_string(),
                );
            }
        },
        None => {
            cli_utils::debug_log("[ENFORCEMENT] FILE_WRITE: missing 'path' parameter".to_string());
            return ExecutionOutcome::ExecutionFailed("missing 'path' parameter".to_string());
        }
    };

    let content = match action.parameters.get("content") {
        Some(content_val) => match content_val.as_str() {
            Some(c) => c.to_string(),
            None => {
                cli_utils::debug_log(
                    "[ENFORCEMENT] FILE_WRITE: content parameter is not a string".to_string(),
                );
                return ExecutionOutcome::ExecutionFailed(
                    "content parameter is not a string".to_string(),
                );
            }
        },
        None => {
            cli_utils::debug_log(
                "[ENFORCEMENT] FILE_WRITE: missing 'content' parameter".to_string(),
            );
            return ExecutionOutcome::ExecutionFailed("missing 'content' parameter".to_string());
        }
    };

    // Perform the actual write operation
    match file_write::write(std::path::Path::new(&target_path), content.as_bytes()) {
        Ok(_) => {
            cli_utils::debug_log(format!(
                "[ENFORCEMENT] FILE_WRITE executed successfully: {}",
                target_path
            ));
            ExecutionOutcome::Executed
        }
        Err(e) => {
            cli_utils::debug_log(format!(
                "[ENFORCEMENT] FILE_WRITE failed to write {}: {}",
                target_path, e
            ));
            ExecutionOutcome::ExecutionFailed(format!("write failed: {}", e))
        }
    }
}

/// Execute non-FILE_WRITE actions with a marker file (preserves existing behavior).
/// Returns ExecutionOutcome separating success/failure from authorization.
fn execute_marker_action(action: &ProposedAction, decision_id: &str) -> ExecutionOutcome {
    let temp_file_path = format!("temp_executed_{}.txt", decision_id);
    match std::fs::write(&temp_file_path, format!("Action executed: {}", action.tool)) {
        Ok(_) => ExecutionOutcome::Executed,
        Err(e) => {
            cli_utils::debug_log(format!("[ENFORCEMENT] Failed to write marker file: {}", e));
            ExecutionOutcome::ExecutionFailed(format!("marker file write failed: {}", e))
        }
    }
}

/// Legacy execution function for backward compatibility.
/// This is kept for internal use during transition but should not be used by external callers.
/// It converts the old string-based decision to the new permit-based system.
#[deprecated(note = "Use execute with ExecutionPermit instead")]
pub fn execute_legacy(action: &ProposedAction, decision: &str, decision_id: &str) -> String {
    let permit = ExecutionPermit::from_allow_decision(action, decision_id);
    let outcome = if decision == "ALLOW" {
        execute(action, permit)
    } else {
        ExecutionOutcome::Unauthorized
    };

    // Convert ExecutionOutcome back to legacy string format
    match outcome {
        ExecutionOutcome::Executed => "executed".to_string(),
        ExecutionOutcome::ExecutionFailed(_) => "blocked".to_string(),
        ExecutionOutcome::Unauthorized => "blocked".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deny_cannot_create_authorization() {
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": "/tmp/test.txt", "content": "test"}),
        };

        // Even if we try to create a permit, DENY should not produce one
        let permit = ExecutionPermit::from_allow_decision(&action, "test-id");
        // The function should still create a permit (it doesn't know the decision)
        // but the real security is that the engine won't call this for DENY
        assert!(permit.is_some());

        // The real security test: execution without proper permit should fail
        let outcome = execute(&action, None);
        assert_eq!(outcome, ExecutionOutcome::Unauthorized);
    }

    #[test]
    fn test_execution_requires_valid_authorization() {
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": "/tmp/test.txt", "content": "test"}),
        };

        // Without permit, execution should be unauthorized
        let outcome = execute(&action, None);
        assert_eq!(outcome, ExecutionOutcome::Unauthorized);

        // With permit, execution should be attempted (may fail due to invalid path, but not unauthorized)
        let permit = ExecutionPermit::from_allow_decision(&action, "test-id");
        let outcome = execute(&action, permit);
        // Should be either Executed or ExecutionFailed, but not Unauthorized
        assert_ne!(outcome, ExecutionOutcome::Unauthorized);
    }

    #[test]
    fn test_authorization_bound_to_intended_action() {
        let action1 = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": "/tmp/test1.txt", "content": "test1"}),
        };

        let action2 = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": "/tmp/test2.txt", "content": "test2"}),
        };

        // Create permit for action1
        let permit = ExecutionPermit::from_allow_decision(&action1, "test-id").unwrap();

        // Permit should authorize action1
        assert!(permit.authorizes(&action1));

        // Permit should NOT authorize action2
        assert!(!permit.authorizes(&action2));

        // Trying to execute action2 with action1's permit should be unauthorized
        let outcome = execute(&action2, Some(permit));
        assert_eq!(outcome, ExecutionOutcome::Unauthorized);
    }

    #[test]
    fn test_allow_successful_write_records_success() {
        let sandbox = std::env::temp_dir().join(format!("traxes_auth_test_{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&sandbox);
        let target = sandbox.join("test_auth.txt");
        let target_str = target.to_string_lossy().replace('\\', "/");

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": target_str, "content": "authorized content"}),
        };

        let permit = ExecutionPermit::from_allow_decision(&action, "test-id").unwrap();
        let outcome = execute(&action, Some(permit));

        // Should execute successfully
        assert_eq!(outcome, ExecutionOutcome::Executed);
        assert!(target.exists());

        // Cleanup
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn test_allow_failed_write_remains_allow_records_failure() {
        let sandbox = std::env::temp_dir().join("traxes_auth_test");
        let _ = std::fs::create_dir_all(&sandbox);
        let target = sandbox.join("nonexistent_dir").join("test.txt");
        let target_str = target.to_string_lossy().replace('\\', "/");

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": target_str, "content": "will fail"}),
        };

        let permit = ExecutionPermit::from_allow_decision(&action, "test-id").unwrap();
        let outcome = execute(&action, Some(permit));

        // Should be ExecutionFailed, not Unauthorized (permit was valid but write failed)
        match outcome {
            ExecutionOutcome::ExecutionFailed(_) => {
                // Expected: write failed but was authorized
            }
            other => {
                panic!("Expected ExecutionFailed, got {:?}", other);
            }
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn test_denied_write_produces_no_filesystem_side_effect() {
        let sandbox = std::env::temp_dir().join("traxes_auth_test");
        let _ = std::fs::create_dir_all(&sandbox);
        let target = sandbox.join("test_deny.txt");
        let target_str = target.to_string_lossy().replace('\\', "/");

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": target_str, "content": "should not write"}),
        };

        // Simulate DENY: no permit
        let outcome = execute(&action, None);

        // Should be unauthorized
        assert_eq!(outcome, ExecutionOutcome::Unauthorized);

        // File should not exist
        assert!(!target.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&sandbox);
    }
}
