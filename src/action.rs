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
///
/// The permit uses SHA-256 hashing to bind to the specific action, providing action integrity
/// and ensuring a permit for one action cannot authorize a different action.
/// A permit is consumed by execution and cannot be cloned or reused.
/// Policy configuration is trusted: callers able to create an engine can choose
/// its policy. Permits do not expire or track later policy changes.
///
/// External callers cannot mint a permit directly:
/// ```compile_fail
/// use traxes_demo::action::{ExecutionPermit, ProposedAction};
/// fn forge(action: &ProposedAction) {
///     let _ = ExecutionPermit::from_allow_decision(action, "forged");
/// }
/// ```
/// Nor can they construct one using its fields:
/// ```compile_fail
/// use traxes_demo::action::ExecutionPermit;
/// let _ = ExecutionPermit {
///     action_hash: String::new(),
///     decision_id: String::new(),
///     _phantom: std::marker::PhantomData,
/// };
/// ```
/// Permits cannot be duplicated:
/// ```compile_fail
/// use traxes_demo::action::ExecutionPermit;
/// fn duplicate(permit: ExecutionPermit) {
///     let _: ExecutionPermit = permit.clone();
/// }
/// ```
/// Execution consumes the permit, including when the attempt fails:
/// ```compile_fail
/// use traxes_demo::action::{execute, ExecutionPermit, ProposedAction};
/// fn reuse(action: &ProposedAction, permit: ExecutionPermit) {
///     execute(action, Some(permit));
///     execute(action, Some(permit));
/// }
/// ```
/// There is no legacy string-based authorization entry point:
/// ```compile_fail
/// use traxes_demo::action::{execute_legacy, ProposedAction};
/// fn bypass(action: &ProposedAction) {
///     execute_legacy(action, "ALLOW", "forged");
/// }
/// ```
#[derive(Debug)]
pub struct ExecutionPermit {
    action_hash: String,
    decision_id: String,
    _phantom: std::marker::PhantomData<()>,
}

impl ExecutionPermit {
    /// Create an execution permit from an ALLOW decision.
    /// This is ONLY callable by trusted TRAXES evaluation code within this crate.
    /// External callers cannot create permits - they must go through the engine's
    /// evaluate_with_permit() method which creates permits only for ALLOW decisions.
    pub(crate) fn from_allow_decision(action: &ProposedAction, decision_id: &str) -> Option<Self> {
        Some(ExecutionPermit {
            action_hash: Self::hash_action(action),
            decision_id: decision_id.to_string(),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Verify that this permit authorizes the given action.
    /// A permit for one action cannot authorize a different action.
    fn authorizes(&self, action: &ProposedAction) -> bool {
        self.action_hash == Self::hash_action(action)
    }

    /// Get the decision ID for this permit.
    pub(crate) fn decision_id(&self) -> &str {
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
    /// Authorization was absent or did not match this action.
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

        // From within the same crate, we can still call from_allow_decision for testing
        // but external crates cannot - it's crate-private
        let permit = ExecutionPermit::from_allow_decision(&action, "test-id");
        // The function creates a permit if called, but the engine never calls it for DENY
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

        // With permit from engine, execution should be attempted (may fail due to invalid path, but not unauthorized)
        // Use the engine to create the permit (the proper workflow)
        let engine = crate::traxes_engine::Engine::with_policy(
            "apiVersion: Traxes.dev/v1\nkind: ExecutionPolicy\nmetadata:\n  name: test-allow\ntarget:\n  tool: FILE_WRITE\nrules:\n  - name: allow-all\n    condition: payload.proposed_action.parameters.content not in [\"test\", \"test1\", \"authorized content\", \"will fail\"]\n    action: DENY"
                .to_string(),
        );
        let (_, permit) = engine.evaluate_with_permit(&action, "test-id");
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

        // Create permit for action1 using the engine (proper workflow)
        let engine = crate::traxes_engine::Engine::with_policy(
            "apiVersion: Traxes.dev/v1\nkind: ExecutionPolicy\nmetadata:\n  name: test-allow\ntarget:\n  tool: FILE_WRITE\nrules:\n  - name: allow-all\n    condition: payload.proposed_action.parameters.content not in [\"test\", \"test1\", \"authorized content\", \"will fail\"]\n    action: DENY"
                .to_string(),
        );
        let (_, permit) = engine.evaluate_with_permit(&action1, "test-id");
        let permit = permit.unwrap();

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

        // Use engine to create permit (proper workflow)
        let engine = crate::traxes_engine::Engine::with_policy(
            "apiVersion: Traxes.dev/v1\nkind: ExecutionPolicy\nmetadata:\n  name: test-allow\ntarget:\n  tool: FILE_WRITE\nrules:\n  - name: allow-all\n    condition: payload.proposed_action.parameters.content not in [\"test\", \"test1\", \"authorized content\", \"will fail\"]\n    action: DENY"
                .to_string(),
        );
        let (_, permit) = engine.evaluate_with_permit(&action, "test-id");
        let outcome = execute(&action, permit);

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

        // Use engine to create permit (proper workflow)
        let engine = crate::traxes_engine::Engine::with_policy(
            "apiVersion: Traxes.dev/v1\nkind: ExecutionPolicy\nmetadata:\n  name: test-allow\ntarget:\n  tool: FILE_WRITE\nrules:\n  - name: allow-all\n    condition: payload.proposed_action.parameters.content not in [\"test\", \"test1\", \"authorized content\", \"will fail\"]\n    action: DENY"
                .to_string(),
        );
        let (_, permit) = engine.evaluate_with_permit(&action, "test-id");
        let outcome = execute(&action, permit);

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

        // Use engine to evaluate and get no permit for DENY (proper workflow)
        let engine = crate::traxes_engine::Engine::with_policy(
            "apiVersion: Traxes.dev/v1\nkind: ExecutionPolicy\nmetadata:\n  name: test-deny\ntarget:\n  tool: FILE_WRITE\nrules:\n  - name: deny-all\n    condition: payload.proposed_action.parameters.content not in [\"never-allowed\"]\n    action: DENY"
                .to_string(),
        );
        let (_, permit) = engine.evaluate_with_permit(&action, "test-id");
        let outcome = execute(&action, permit);

        // Should be unauthorized (engine returned None permit for DENY)
        assert_eq!(outcome, ExecutionOutcome::Unauthorized);

        // File should not exist
        assert!(!target.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&sandbox);
    }

    #[test]
    fn test_crate_private_permit_retains_decision_id() {
        // Constructor/field privacy is checked by external compile-fail doctests.
        // External crates cannot inspect or modify permit internals

        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: "test-session".to_string(),
            environment: "test".to_string(),
            parameters: json!({"path": "/tmp/test.txt", "content": "test"}),
        };

        // Create a permit (within the same crate)
        let permit = ExecutionPermit::from_allow_decision(&action, "test-id").unwrap();

        assert_eq!(permit.decision_id(), "test-id");
    }
}
