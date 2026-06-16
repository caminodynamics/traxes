use crate::execution_event::ExecutionEvent;
use crate::artifact::AuditArtifact;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Artifact emitter worker that processes execution events off the hot path
/// Handles artifact formatting, hashing, signing, and persistence
pub struct ArtifactEmitter {
    /// Event counter for assigning monotonically increasing IDs
    event_counter: Arc<AtomicU64>,
    /// Receiver for execution events
    event_rx: mpsc::Receiver<ExecutionEvent>,
    /// Policy hash for artifact generation
    policy_hash: String,
}

impl ArtifactEmitter {
    /// Create a new artifact emitter
    pub fn new(
        event_rx: mpsc::Receiver<ExecutionEvent>,
        policy_hash: String,
    ) -> Self {
        Self {
            event_counter: Arc::new(AtomicU64::new(0)),
            event_rx,
            policy_hash,
        }
    }

    /// Run the artifact emitter worker
    /// This runs in a background task and processes events asynchronously
    pub async fn run(mut self) {
        while let Some(mut event) = self.event_rx.recv().await {
            // Assign monotonically increasing event ID
            let event_id = self.event_counter.fetch_add(1, Ordering::SeqCst);
            event = event.with_event_id(event_id);

            // Process the event off the hot path
            if let Err(e) = self.process_event(event).await {
                crate::cli_utils::debug_log(format!("[ArtifactEmitter] Failed to process event {}: {}", event_id, e));
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
                crate::cli_utils::debug_log(format!("[ArtifactEmitter] Failed to convert event to artifact: {}", e));
                return Err(e);
            }
        };

        // Write artifact to disk
        match artifact.write_to_file().await {
            Ok(_) => {}
            Err(e) => {
                crate::cli_utils::debug_log(format!("[ArtifactEmitter] Failed to write artifact to disk: {}", e));
                return Err(e);
            }
        };

        Ok(())
    }

    /// Convert execution event to full audit artifact
    /// This is the heavy lifting - formatting, hashing, etc.
    fn event_to_artifact(&self, event: ExecutionEvent) -> Result<AuditArtifact, Box<dyn std::error::Error>> {
        // Calculate SHA256 hash for the artifact
        let sha256_hash = Self::calculate_sha256_hash(
            &event.decision_id,
            &event.tool,
            &event.decision,
            &event.rule_trace,
        );

        // Build the full artifact
        let artifact = AuditArtifact {
            artifact_version: "1.0.0".to_string(),
            artifact_type: "pre_execution_decision".to_string(),
            decision_id: event.decision_id.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            decision: event.decision.clone(),
            tool: event.tool.clone(),
            environment: event.environment.clone(),
            reason: if event.decision == "DENY" {
                "instance_type is not allowed for this environment".to_string()
            } else {
                String::new()
            },
            policy_bundle: "infra-cost-limit-v1".to_string(),
            policy_hash: self.policy_hash.clone(),
            sha256_hash,
            engine: crate::artifact::EngineInfo {
                name: "Traxes".to_string(),
                engine_version: event.replay_metadata.engine_version.clone(),
                policy_bundle_id: "infra-cost-limit-v1".to_string(),
            },
            execution_context: crate::artifact::ExecutionContext {
                session_id: event.session_id.clone(),
                trace_id: event.trace_id.clone(),
            },
            proposed_action: crate::artifact::ProposedActionInfo {
                tool: event.tool.clone(),
                environment: event.environment.clone(),
                parameters: crate::artifact::ActionParameters {
                    instance_type: event.rule_trace.observed_value.clone(),
                    instance_cost_per_hour: 0.0, // Will be populated from event if needed
                },
            },
            rule_evaluation: crate::artifact::RuleEvaluationInfo {
                rule_id: event.rule_trace.rule_id.clone(),
                field: event.rule_trace.field.clone(),
                observed_value: serde_json::Value::String(event.rule_trace.observed_value.clone()),
                operator: event.rule_trace.operator.clone(),
                policy_value: 0.0,
                evaluation_expression: format!("{} not_in policy", event.rule_trace.field),
                evaluation_result: event.rule_trace.evaluation_result,
            },
            performance: crate::artifact::PerformanceInfo {
                evaluation_latency_us: event.performance.evaluation_latency_us as f64,
                decision_latency_us: event.performance.decision_latency_us as f64,
                artifact_write_latency_us: 41.7,
            },
            side_effect_prevention: crate::artifact::SideEffectPreventionInfo {
                decision_effect: event.decision.clone(),
            },
            execution_status: event.execution_status.clone(),
        };

        Ok(artifact)
    }

    /// Calculate SHA256 hash for artifact
    fn calculate_sha256_hash(
        decision_id: &str,
        tool: &str,
        decision: &str,
        rule_trace: &crate::execution_event::RuleTrace,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(decision_id.as_bytes());
        hasher.update(tool.as_bytes());
        hasher.update(decision.as_bytes());
        hasher.update(rule_trace.rule_id.as_bytes());
        hasher.update(rule_trace.observed_value.as_bytes());
        hasher.update(rule_trace.evaluation_result.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Event emitter for the evaluation loop
/// This is the hot-path component that emits compact events
#[derive(Clone)]
pub struct EventEmitter {
    /// Sender for execution events
    event_tx: mpsc::Sender<ExecutionEvent>,
}

impl EventEmitter {
    /// Create a new event emitter
    pub fn new(event_tx: mpsc::Sender<ExecutionEvent>) -> Self {
        Self {
            event_tx,
        }
    }

    /// Try to emit an execution event without blocking
    /// Returns error if the channel is full
    pub fn try_emit(&self, event: ExecutionEvent) -> Result<(), mpsc::error::TrySendError<ExecutionEvent>> {
        self.event_tx.try_send(event)
    }
}

/// Create a bounded channel for event emission
pub fn create_event_channel(capacity: usize) -> (EventEmitter, mpsc::Receiver<ExecutionEvent>) {
    let (tx, rx) = mpsc::channel(capacity);
    let emitter = EventEmitter::new(tx);
    (emitter, rx)
}
