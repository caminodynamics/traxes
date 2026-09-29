// External-crate runtime coverage. Compile-fail doctests on ExecutionPermit
// separately enforce constructor/field privacy, non-Clone, consumption, and
// absence of the legacy execution entry point.
use serde_json::json;
use std::{fs, path::PathBuf};
use traxes_demo::action::execute;
use traxes_demo::{Engine, ExecutionOutcome, ProposedAction};

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("traxes-api-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn action(&self) -> ProposedAction {
        ProposedAction {
            tool: "FILE_WRITE".into(),
            session_id: "external-test".into(),
            environment: "test".into(),
            parameters: json!({
                "path": self.0.join("target.txt").to_string_lossy().replace('\\', "/"),
                "content": "authorized content"
            }),
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn engine_for(action: &ProposedAction) -> Engine {
    // JSON strings also form valid YAML scalars. Bind the policy to this test's
    // unique absolute path rather than relying on platform-specific /tmp paths.
    Engine::with_policy(format!(
        "target:\n  tool: FILE_WRITE\nrules:\n  - name: path-allowlist\n    condition: payload.proposed_action.parameters.path not in [{}]\n    action: DENY",
        action.parameters["path"]
    ))
}

#[test]
fn test_external_crate_must_use_engine_for_permit() {
    let sandbox = Sandbox::new();
    let action = sandbox.action();
    let engine = engine_for(&action);
    let (allowed, permit) = engine.evaluate_with_permit(&action, "allowed");
    assert_eq!(allowed.decision, "ALLOW");
    assert!(permit.is_some());

    let mut denied = action.clone();
    denied.parameters["path"] = json!(sandbox.0.join("denied.txt").to_string_lossy());
    let (evaluation, permit) = engine.evaluate_with_permit(&denied, "denied");
    assert_eq!(evaluation.decision, "DENY");
    assert!(permit.is_none());
    assert_eq!(execute(&denied, permit), ExecutionOutcome::Unauthorized);
    assert_eq!(evaluation.decision, "DENY");
    assert!(!sandbox.0.join("denied.txt").exists());
}

#[test]
fn test_external_crate_cannot_execute_without_permit() {
    let sandbox = Sandbox::new();
    let action = sandbox.action();
    let target = sandbox.0.join("target.txt");
    assert_eq!(execute(&action, None), ExecutionOutcome::Unauthorized);
    assert!(!target.exists());
    fs::write(&target, "untouched").unwrap();
    assert_eq!(execute(&action, None), ExecutionOutcome::Unauthorized);
    assert_eq!(fs::read_to_string(target).unwrap(), "untouched");
}

#[test]
fn test_external_crate_authorization_workflow() {
    let sandbox = Sandbox::new();
    let action = sandbox.action();
    let engine = engine_for(&action);
    let (evaluation, permit) = engine.evaluate_with_permit(&action, "success");
    assert_eq!(evaluation.decision, "ALLOW");
    assert!(permit.is_some());
    assert_eq!(execute(&action, permit), ExecutionOutcome::Executed);
    assert_eq!(evaluation.decision, "ALLOW");
    assert_eq!(
        fs::read_to_string(sandbox.0.join("target.txt")).unwrap(),
        "authorized content"
    );
}

#[test]
fn test_external_allowed_execution_failure_remains_allow() {
    let sandbox = Sandbox::new();
    let action = sandbox.action();
    // A directory at the approved file path deterministically prevents writing.
    let target = sandbox.0.join("target.txt");
    fs::create_dir(&target).unwrap();
    let engine = engine_for(&action);
    let (evaluation, permit) = engine.evaluate_with_permit(&action, "failure");
    assert_eq!(evaluation.decision, "ALLOW");
    assert!(permit.is_some());
    assert!(matches!(
        execute(&action, permit),
        ExecutionOutcome::ExecutionFailed(_)
    ));
    assert_eq!(evaluation.decision, "ALLOW");
    assert!(target.is_dir());
    assert_eq!(fs::read_dir(target).unwrap().count(), 0);
}

#[test]
fn test_external_permit_rejects_each_changed_action_field() {
    let sandbox = Sandbox::new();
    let action = sandbox.action();
    let engine = engine_for(&action);
    let target = sandbox.0.join("target.txt");
    fs::write(&target, "untouched").unwrap();

    for field in [
        "tool",
        "session_id",
        "environment",
        "path",
        "content",
        "metadata",
    ] {
        let (evaluation, permit) = engine.evaluate_with_permit(&action, field);
        assert_eq!(evaluation.decision, "ALLOW");
        assert!(permit.is_some());
        let mut changed = action.clone();
        match field {
            "tool" => changed.tool = "AWS".into(),
            "session_id" => changed.session_id = "other-session".into(),
            "environment" => changed.environment = "production".into(),
            "path" => {
                changed.parameters["path"] = json!(sandbox.0.join("other.txt").to_string_lossy())
            }
            "content" => changed.parameters["content"] = json!("tampered"),
            "metadata" => changed.parameters["metadata"] = json!({"extra": true}),
            _ => unreachable!(),
        }
        assert_eq!(
            execute(&changed, permit),
            ExecutionOutcome::Unauthorized,
            "{field}"
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "untouched", "{field}");
        assert!(!sandbox.0.join("other.txt").exists());
    }
}

#[test]
fn test_external_target_mismatch_cannot_mint_permit() {
    let sandbox = Sandbox::new();
    let mut action = sandbox.action();
    let engine = engine_for(&action);
    action.tool = "AWS".into();
    let (evaluation, permit) = engine.evaluate_with_permit(&action, "wrong-tool");
    assert_eq!(evaluation.decision, "DENY");
    assert!(permit.is_none());
    assert_eq!(execute(&action, permit), ExecutionOutcome::Unauthorized);
}
