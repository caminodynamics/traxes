use crate::action::ProposedAction;
use crate::artifact::AuditArtifact;
use crate::execution_event::ExecutionEvent;
use crate::traxes_engine::{DecisionRecord, EvaluationDecision};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Artifact emitter worker that processes execution events off the hot path
/// Handles artifact formatting, hashing, signing, and persistence
pub struct ArtifactEmitter {
    /// Event counter for assigning monotonically increasing IDs (shared across all shards)
    event_counter: Arc<AtomicU64>,
    /// Receiver for execution events (single shard)
    event_rx: mpsc::Receiver<ExecutionEvent>,
    /// Policy hash for artifact generation
    policy_hash: String,
}

impl ArtifactEmitter {
    /// Create a new artifact emitter
    pub fn new(
        event_rx: mpsc::Receiver<ExecutionEvent>,
        policy_hash: String,
        event_counter: Arc<AtomicU64>,
    ) -> Self {
        Self {
            event_counter,
            event_rx,
            policy_hash,
        }
    }

    /// Run the artifact emitter worker
    /// This runs in a background task and processes events asynchronously
    pub async fn run(mut self) {
        while let Some(mut event) = self.event_rx.recv().await {
            // Assign monotonically increasing event ID (shared across all shards)
            // Using Relaxed ordering for performance - uniqueness only required, not strict ordering
            let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
            event = event.with_event_id(event_id);

            // Process the event off the hot path
            if let Err(e) = self.process_event(event).await {
                crate::cli_utils::debug_log(format!(
                    "[ArtifactEmitter] Failed to process event {}: {}",
                    event_id, e
                ));
            }
        }
    }

    /// Process a single execution event
    /// This includes artifact formatting, hashing, and persistence
    async fn process_event(&self, event: ExecutionEvent) -> Result<(), Box<dyn std::error::Error>> {
        // Convert execution event to full artifact
        let artifact = match self.event_to_artifact(event) {
            Ok(artifact) => artifact,
            Err(e) => {
                crate::cli_utils::debug_log(format!(
                    "[ArtifactEmitter] Failed to convert event to artifact: {}",
                    e
                ));
                return Err(e);
            }
        };

        // Write artifact to disk
        match artifact.write_to_file().await {
            Ok(_) => {}
            Err(e) => {
                crate::cli_utils::debug_log(format!(
                    "[ArtifactEmitter] Failed to write artifact to disk: {}",
                    e
                ));
                return Err(e);
            }
        };

        Ok(())
    }

    /// Convert execution event to full audit artifact
    /// This now uses the canonical artifact builder (AuditArtifact::from_evaluation)
    /// to ensure identical artifacts from async and sync paths
    pub fn event_to_artifact(
        &self,
        event: ExecutionEvent,
    ) -> Result<AuditArtifact, Box<dyn std::error::Error>> {
        // Convert event to evaluation decision
        let evaluation_result = event.to_evaluation_decision();

        // Reconstruct ProposedAction from event
        let action = ProposedAction {
            tool: event.tool.clone(),
            session_id: event.session_id.clone(),
            environment: event.environment.clone(),
            parameters: event.action_parameters.clone(),
        };

        // Create EvaluationDecision wrapper
        let evaluation_decision = EvaluationDecision {
            result: evaluation_result,
            decision: event.decision.clone(),
            evaluation_latency_us: event.performance.evaluation_latency_us as f64,
        };

        // Use canonical artifact builder
        let artifact = AuditArtifact::from_evaluation(
            &event.decision_id,
            &action,
            &evaluation_decision,
            &self.policy_hash,
            event.execution_status.clone(),
            event.execution_outcome.clone(),
        );

        Ok(artifact)
    }
}

/// Sharded event emitter for the evaluation loop
/// This reduces contention by distributing events across multiple channels
#[derive(Clone)]
pub struct EventEmitter {
    /// Multiple senders for execution events (sharded by hash)
    senders: Vec<mpsc::Sender<ExecutionEvent>>,
    /// Number of shards
    shard_count: usize,
}

impl EventEmitter {
    /// Create a new sharded event emitter
    pub fn new(senders: Vec<mpsc::Sender<ExecutionEvent>>) -> Self {
        let shard_count = senders.len();
        Self {
            senders,
            shard_count,
        }
    }

    /// Try to emit an execution event without blocking
    /// Routes to specific shard based on decision_id hash to reduce contention
    /// Returns error if the channel is full
    pub fn try_emit(
        &self,
        event: ExecutionEvent,
    ) -> Result<(), mpsc::error::TrySendError<ExecutionEvent>> {
        // Hash decision_id to determine shard
        let mut hasher = DefaultHasher::new();
        event.decision_id.hash(&mut hasher);
        let shard_index = (hasher.finish() as usize) % self.shard_count;

        // Send to specific shard
        self.senders[shard_index].try_send(event)
    }

    /// Try to emit a lightweight decision record without blocking
    /// Routes to specific shard based on decision_id hash to reduce contention
    /// Converts DecisionRecord to ExecutionEvent before sending
    /// Returns error if the channel is full
    pub fn try_emit_record(
        &self,
        record: DecisionRecord,
    ) -> Result<(), mpsc::error::TrySendError<ExecutionEvent>> {
        // Hash decision_id to determine shard
        let mut hasher = DefaultHasher::new();
        record.decision_id.hash(&mut hasher);
        let shard_index = (hasher.finish() as usize) % self.shard_count;

        // Convert to ExecutionEvent and send to specific shard
        let event = ExecutionEvent::from_record(record);
        self.senders[shard_index].try_send(event)
    }
}

/// Create sharded channels for event emission
/// Returns (EventEmitter with sharded senders, Vec of receivers for background workers)
pub fn create_sharded_event_channels(
    shard_count: usize,
    capacity_per_shard: usize,
) -> (
    EventEmitter,
    Vec<mpsc::Receiver<ExecutionEvent>>,
    Arc<AtomicU64>,
) {
    let mut senders = Vec::with_capacity(shard_count);
    let mut receivers = Vec::with_capacity(shard_count);

    for _ in 0..shard_count {
        let (tx, rx) = mpsc::channel(capacity_per_shard);
        senders.push(tx);
        receivers.push(rx);
    }

    let emitter = EventEmitter::new(senders);
    let event_counter = Arc::new(AtomicU64::new(0));

    (emitter, receivers, event_counter)
}

/// Create a bounded channel for event emission (legacy single-channel interface)
pub fn create_event_channel(capacity: usize) -> (EventEmitter, mpsc::Receiver<ExecutionEvent>) {
    let (tx, rx) = mpsc::channel(capacity);
    let emitter = EventEmitter::new(vec![tx]);
    (emitter, rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        artifact::ArtifactLogger, evaluate::run_evaluate_with_emitter, traxes_engine::Engine,
    };
    use serde_json::json;
    use std::fs;

    #[tokio::test]
    async fn cli_async_artifacts_preserve_success_failure_and_deny() {
        let sandbox =
            std::env::temp_dir().join(format!("traxes-outcomes-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&sandbox).unwrap();
        for case in ["success", "failure", "deny"] {
            let target = sandbox.join(format!("{case}.txt"));
            if case == "failure" {
                fs::create_dir(&target).unwrap();
            }
            let path = target.to_string_lossy().replace('\\', "/");
            let allowed = if case == "deny" {
                "never-allowed"
            } else {
                &path
            };
            let engine = Engine::with_policy(format!(
                "target:\n  tool: FILE_WRITE\nrules:\n  - name: paths\n    condition: payload.proposed_action.parameters.path not in [{}]\n    action: DENY",
                json!(allowed)
            ));
            let action = ProposedAction {
                tool: "FILE_WRITE".into(),
                session_id: case.into(),
                environment: "test".into(),
                parameters: json!({"path": path, "content": "actual bytes"}),
            };
            let payload = sandbox.join(format!("{case}.json"));
            fs::write(&payload, serde_json::to_vec(&action).unwrap()).unwrap();
            let (sender, mut receiver) = create_event_channel(1);
            let result =
                run_evaluate_with_emitter(payload.to_string_lossy().into(), sender, &engine)
                    .await
                    .unwrap();
            let event = receiver.recv().await.unwrap();
            let expected_decision = if case == "deny" { "DENY" } else { "ALLOW" };
            assert_eq!(result.decision, expected_decision);
            let outcome = event
                .execution_outcome
                .clone()
                .expect("outcome lost in queue");
            match case {
                "success" => {
                    assert_eq!(outcome, "Executed");
                    assert_eq!(fs::read_to_string(&target).unwrap(), "actual bytes");
                }
                "failure" => {
                    assert!(outcome.starts_with("ExecutionFailed("), "{outcome}");
                    assert!(target.is_dir());
                }
                "deny" => {
                    assert_eq!(outcome, "Unauthorized");
                    assert!(!target.exists());
                }
                _ => unreachable!(),
            }
            let expected = ArtifactLogger::generate_artifact_with_outcome(
                &event.decision_id,
                &action,
                &engine.evaluate(&action),
                engine.policy_hash(),
                event.execution_status.clone(),
                Some(outcome.clone()),
            );
            let artifact_path = format!("artifacts/AuditArtifact_{}.json", event.decision_id);
            let emitter = ArtifactEmitter::new(
                receiver,
                engine.policy_hash().into(),
                Arc::new(AtomicU64::new(0)),
            );
            // Exercise the worker's actual persistence path, not only its builder.
            emitter.process_event(event).await.unwrap();
            let persisted: AuditArtifact =
                serde_json::from_slice(&fs::read(&artifact_path).unwrap()).unwrap();
            assert_eq!(persisted.decision, expected_decision);
            assert_eq!(
                persisted.execution_outcome.as_deref(),
                Some(outcome.as_str())
            );
            assert_eq!(
                persisted.execution_status,
                if case == "success" {
                    "executed"
                } else if case == "failure" {
                    "failed"
                } else {
                    "blocked"
                }
            );
            assert_eq!(persisted.sha256_hash, expected.sha256_hash);

            fs::remove_file(artifact_path).unwrap();
        }
        fs::remove_dir_all(sandbox).unwrap();
    }
}
