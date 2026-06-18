use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use crate::traxes_engine::DecisionRecord;

/// Compact, immutable execution event emitted by the evaluation loop
/// Designed for allocation-minimal hot path emission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// Unique event identifier (monotonically increasing)
    pub event_id: u64,
    
    /// Timestamp when the event was created
    pub timestamp: SystemTime,
    
    /// Deterministic decision ID for this evaluation
    pub decision_id: String,
    
    /// Session identifier
    pub session_id: String,
    
    /// Trace identifier for distributed tracing
    pub trace_id: String,
    
    /// Tool being evaluated
    pub tool: String,
    
    /// Environment context
    pub environment: String,
    
    /// Final decision (ALLOW/DENY)
    pub decision: String,
    
    /// Compact rule trace - minimal allocation
    pub rule_trace: RuleTrace,
    
    /// Replay metadata for deterministic replay
    pub replay_metadata: ReplayMetadata,
    
    /// Performance metrics (compact)
    pub performance: PerformanceMetrics,

    /// Execution status after enforcement gate
    pub execution_status: String,
    
    /// Full action parameters for complete artifact construction
    pub action_parameters: serde_json::Value,
    
    /// Policy value from evaluation
    pub policy_value: f64,
    
    /// Full reason text from evaluation
    pub reason: String,
}

/// Compact rule execution trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleTrace {
    /// Rule ID that was evaluated
    pub rule_id: String,
    
    /// Field being evaluated
    pub field: String,
    
    /// Observed value (compact representation)
    pub observed_value: String,
    
    /// Operator used
    pub operator: String,
    
    /// Whether the rule was violated
    pub violation: bool,
    
    /// Evaluation result (true if violation detected)
    pub evaluation_result: bool,
}

/// Metadata for deterministic replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMetadata {
    /// Hash of the policy used for evaluation
    pub policy_hash: String,
    
    /// Hash of the action payload
    pub action_hash: String,
    
    /// Engine version for replay compatibility
    pub engine_version: String,
    
    /// Evaluation sequence number for ordering
    pub sequence: u64,
}

/// Compact performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Evaluation latency in microseconds
    pub evaluation_latency_us: u64,
    
    /// Decision latency in microseconds
    pub decision_latency_us: u64,
}

impl ExecutionEvent {
    /// Create a new execution event from evaluation results
    /// Designed for minimal allocation in the hot path
    pub fn new(
        decision_id: String,
        session_id: String,
        trace_id: String,
        tool: String,
        environment: String,
        decision: String,
        rule_trace: RuleTrace,
        replay_metadata: ReplayMetadata,
        performance: PerformanceMetrics,
        execution_status: String,
        action_parameters: serde_json::Value,
        policy_value: f64,
        reason: String,
    ) -> Self {
        Self {
            event_id: 0, // Will be assigned by emitter
            timestamp: SystemTime::now(),
            decision_id,
            session_id,
            trace_id,
            tool,
            environment,
            decision,
            rule_trace,
            replay_metadata,
            performance,
            execution_status,
            action_parameters,
            policy_value,
            reason,
        }
    }
    
    /// Set the event ID (called by emitter)
    pub fn with_event_id(mut self, event_id: u64) -> Self {
        self.event_id = event_id;
        self
    }
    
    /// Convert ExecutionEvent to EvaluationDecision for canonical artifact construction
    /// This enables the async path to use the same artifact builder as the sync path
    pub fn to_evaluation_decision(&self) -> crate::server_policy::EvaluationResult {
        crate::server_policy::EvaluationResult {
            action: if self.decision == "DENY" {
                Some("DENY".to_string())
            } else {
                None
            },
            field: self.rule_trace.field.clone(),
            rule: self.rule_trace.rule_id.clone(),
            observed_value: self.rule_trace.observed_value.clone().parse().unwrap_or(0.0),
            observed_value_str: self.rule_trace.observed_value.clone(),
            policy_value: self.policy_value,
            evaluation_expression: format!("{} not_in policy", self.rule_trace.field),
            reason: self.reason.clone(),
        }
    }
    
    /// Convert a lightweight DecisionRecord to a full ExecutionEvent
    /// This is done in the async worker to keep the hot path lightweight
    pub fn from_record(record: DecisionRecord) -> Self {
        use sha2::Digest;
        
        // Generate action hash for replay metadata
        let action_hash = format!("{:x}", sha2::Sha256::digest(
            serde_json::to_string(&record.parameters).unwrap_or_default()
        ));
        
        // Create rule trace from evaluation result
        let rule_trace = RuleTrace {
            rule_id: "infra-cost-limit".to_string(),
            field: record.evaluation_result.field.clone(),
            observed_value: record.evaluation_result.observed_value_str.clone(),
            operator: record.evaluation_result.rule.clone(),
            violation: record.evaluation_result.action.is_some(),
            evaluation_result: record.evaluation_result.action.is_some(),
        };
        
        // Create replay metadata
        let replay_metadata = ReplayMetadata {
            policy_hash: record.policy_hash.clone(),
            action_hash,
            engine_version: "0.3.2".to_string(),
            sequence: 0,
        };
        
        // Create performance metrics
        let performance = PerformanceMetrics {
            evaluation_latency_us: record.evaluation_latency_us as u64,
            decision_latency_us: 4,
        };
        
        ExecutionEvent {
            event_id: 0,
            timestamp: SystemTime::now(),
            decision_id: record.decision_id,
            session_id: record.session_id,
            trace_id: record.trace_id,
            tool: record.tool,
            environment: record.environment,
            decision: record.decision,
            rule_trace,
            replay_metadata,
            performance,
            execution_status: record.execution_status,
            action_parameters: record.parameters,
            policy_value: record.evaluation_result.policy_value,
            reason: record.evaluation_result.reason,
        }
    }
}
