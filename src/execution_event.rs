use serde::{Deserialize, Serialize};
use std::time::SystemTime;

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
        }
    }
    
    /// Set the event ID (called by emitter)
    pub fn with_event_id(mut self, event_id: u64) -> Self {
        self.event_id = event_id;
        self
    }
}
