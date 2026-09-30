use sha2::{Digest, Sha256};
use traxes_demo::{artifact::ArtifactLogger, replay::ReplayEngine, AuditArtifact, Engine};

const LEGACY: &str = include_str!("fixtures/file_write_v1.json");
const POLICY: &str = include_str!("fixtures/file_write_v1.yaml");

#[test]
fn file_write_failed_status_replays_without_relabeling_historical_artifacts() {
    let legacy: AuditArtifact = serde_json::from_str(LEGACY).unwrap();
    let replay = ReplayEngine::new(Engine::with_policy(POLICY.into()));
    let action = replay.reconstruct_action(&legacy);
    let evaluation = replay.engine.evaluate(&action);
    assert_eq!(evaluation.decision, "ALLOW");
    // Old failures could be stored as blocked, with or without an outcome.
    // Verify those original values; never normalize them before hashing.
    for (status, outcome) in [
        (
            "failed",
            Some("ExecutionFailed(\"write error\")".to_string()),
        ),
        (
            "blocked",
            Some("ExecutionFailed(\"write error\")".to_string()),
        ),
        ("blocked", None),
    ] {
        let artifact = ArtifactLogger::generate_artifact_with_outcome(
            &legacy.decision_id,
            &action,
            &evaluation,
            replay.engine.policy_hash(),
            status.into(),
            outcome,
        );
        let artifact: AuditArtifact =
            serde_json::from_str(&serde_json::to_string(&artifact).unwrap()).unwrap();
        assert_eq!(artifact.execution_status, status);
        assert_eq!(artifact.decision, "ALLOW");
        assert!(replay.replay_from_artifact(&artifact).match_status);
        let mut relabeled = artifact;
        relabeled.execution_status = if status == "failed" {
            "blocked"
        } else {
            "failed"
        }
        .into();
        assert!(!replay.replay_from_artifact(&relabeled).match_status);
    }
}

#[test]
fn pre_change_file_write_v1_artifact_still_verifies() {
    // Frozen pre-PR composition from 3fe51c2: no execution_outcome key,
    // including no null key. Do not regenerate this fixture with the new writer.
    let raw: serde_json::Value = serde_json::from_str(LEGACY).unwrap();
    assert!(raw.get("execution_outcome").is_none());
    let artifact: AuditArtifact = serde_json::from_str(LEGACY).unwrap();
    let old_hash = format!(
        "{:x}",
        Sha256::digest(include_bytes!("fixtures/file_write_v1.preimage.json"))
    );
    assert_eq!(
        old_hash,
        "2775f65be4e1fa7836b3d0e80a7f2a6d98e9b4507660b7982637bf578e3fc3a3"
    );
    assert_eq!(artifact.sha256_hash, old_hash);
    assert!(artifact.execution_outcome.is_none());
    let replay = ReplayEngine::new(Engine::with_policy(POLICY.into()));
    assert!(replay.replay_from_artifact(&artifact).match_status);
    let action = replay.reconstruct_action(&artifact);
    let evaluation = replay.engine.evaluate(&action);
    let regenerated = ArtifactLogger::generate_artifact(
        &artifact.decision_id,
        &action,
        &evaluation,
        replay.engine.policy_hash(),
        artifact.execution_status.clone(),
    );
    assert_eq!(regenerated.sha256_hash, old_hash);

    let mut changed = artifact.clone();
    changed
        .proposed_action
        .parameters
        .extra
        .insert("content".into(), "tampered".into());
    assert!(!replay.replay_from_artifact(&changed).match_status);
    // Adding an outcome to an old artifact selects v2, never the old v1 hash.
    changed = artifact;
    changed.execution_outcome = Some("Executed".into());
    assert!(!replay.replay_from_artifact(&changed).match_status);
}

#[test]
fn file_write_v2_outcome_is_bound_and_cannot_be_stripped() {
    let legacy: AuditArtifact = serde_json::from_str(LEGACY).unwrap();
    let replay = ReplayEngine::new(Engine::with_policy(POLICY.into()));
    let action = replay.reconstruct_action(&legacy);
    let evaluation = replay.engine.evaluate(&action);
    let artifact = ArtifactLogger::generate_artifact_with_outcome(
        &legacy.decision_id,
        &action,
        &evaluation,
        replay.engine.policy_hash(),
        legacy.execution_status.clone(),
        Some("Executed".into()),
    );
    // Independently check the v2 domain tag as well as inclusion of the outcome.
    let mut input: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/file_write_v1.preimage.json")).unwrap();
    input["fingerprint_version"] = "file_write_v2".into();
    input["execution_outcome"] = "Executed".into();
    assert_eq!(
        artifact.sha256_hash,
        format!("{:x}", Sha256::digest(input.to_string()))
    );
    assert_ne!(artifact.sha256_hash, legacy.sha256_hash);
    assert!(replay.replay_from_artifact(&artifact).match_status);
    for outcome in [
        None,
        Some("Unauthorized".into()),
        Some("ExecutionFailed(\"error\")".into()),
    ] {
        let mut changed = artifact.clone();
        changed.execution_outcome = outcome;
        assert!(!replay.replay_from_artifact(&changed).match_status);
    }
}
