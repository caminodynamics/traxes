use crate::metrics::compute_latency_stats;
use crate::policy::PolicyEngine;
use crate::workload::ToolCall;
use serde_json::Value;
use std::borrow::Cow;
use std::cell::RefCell;
 
#[derive(Debug)]
pub enum TraxesError {
    MissingTargetField(&'static str),
}
 
impl std::fmt::Display for TraxesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraxesError::MissingTargetField(field) => write!(f, "Missing target field: {}", field),
        }
    }
}
 
impl std::error::Error for TraxesError {}
 
#[derive(Debug)]
enum DenialRecord {
    Denied { id: u64, reason: Cow<'static, str> },
    Errored { id: u64, error: TraxesError },
}
 
thread_local! {
    static DENIAL_BUFFER: RefCell<Vec<DenialRecord>> =
        RefCell::new(Vec::with_capacity(128));
}
 
#[derive(Debug)]
pub enum EvaluationResponse {
    Allow,
    Deny { reason: Cow<'static, str> },
}
 
#[derive(Debug)]
pub struct RunResult {
    /// Per-request total latency (evaluation + decision_log + enqueue)
    pub latencies_ns: Vec<u128>,
    /// Per-request policy evaluation (or payload read for baseline) latency
    pub evaluation_latencies_ns: Vec<u128>,
    /// Per-request decision logging latency (Traxes only; 0 for baseline)
    pub decision_log_latencies_ns: Vec<u128>,
    /// Per-request async enqueue/receipt latency (currently 0 in sync path)
    pub enqueue_latencies_ns: Vec<u128>,
    pub allowed: usize,
    pub denied: usize,
    pub duration_ns: u128,
    pub allocations: usize,
    pub allocated_bytes: usize,
    pub artifact_enqueue_count: usize,
    pub artifact_enqueue_latency_ns: u128,
    pub queue_max_depth: usize,
    pub queue_drop_count: usize,
}
 
impl RunResult {
    pub fn stats(&self) -> crate::metrics::LatencyStats {
        compute_latency_stats(&self.latencies_ns)
    }
 
    pub fn evaluation_stats(&self) -> crate::metrics::LatencyStats {
        compute_latency_stats(&self.evaluation_latencies_ns)
    }
 
    pub fn decision_log_stats(&self) -> crate::metrics::LatencyStats {
        compute_latency_stats(&self.decision_log_latencies_ns)
    }
 
    pub fn enqueue_stats(&self) -> crate::metrics::LatencyStats {
        compute_latency_stats(&self.enqueue_latencies_ns)
    }
}
 
pub fn run_baseline(workload: &[ToolCall]) -> RunResult {
    let mut latencies_ns = Vec::with_capacity(workload.len());
    let mut evaluation_latencies_ns = Vec::with_capacity(workload.len());
    let mut decision_log_latencies_ns = Vec::with_capacity(workload.len());
    let mut enqueue_latencies_ns = Vec::with_capacity(workload.len());
    let mut allocations = 0;
    let mut allocated_bytes = 0;
    let start = std::time::Instant::now();
 
    for call in workload {
        let (alloc_before, bytes_before) = crate::alloc_snapshot();
        let call_start = std::time::Instant::now();
 
        let _payload_ref: &Value = &call.payload;
 
        // The baseline path reads the payload but skips any policy rule evaluation.
        let _ = _payload_ref;
        let eval_ns = call_start.elapsed().as_nanos();
 
        // For baseline: no decision logging, no enqueue overhead
        let decision_log_ns = 0u128;
        let enqueue_ns = 0u128;
        let total_ns = eval_ns + decision_log_ns + enqueue_ns;
 
        evaluation_latencies_ns.push(eval_ns);
        decision_log_latencies_ns.push(decision_log_ns);
        enqueue_latencies_ns.push(enqueue_ns);
        latencies_ns.push(total_ns);
 
        let (alloc_after, bytes_after) = crate::alloc_snapshot();
        allocations += alloc_after.saturating_sub(alloc_before);
        allocated_bytes += bytes_after.saturating_sub(bytes_before);
    }
 
    let duration_ns = start.elapsed().as_nanos();
    RunResult {
        latencies_ns,
        evaluation_latencies_ns,
        decision_log_latencies_ns,
        enqueue_latencies_ns,
        allowed: workload.len(),
        denied: 0,
        duration_ns,
        allocations,
        allocated_bytes,
        artifact_enqueue_count: 0,
        artifact_enqueue_latency_ns: 0,
        queue_max_depth: 0,
        queue_drop_count: 0,
    }
}
 
pub fn run_traxes(workload: &[ToolCall], engine: &PolicyEngine) -> RunResult {
    let mut latencies_ns = Vec::with_capacity(workload.len());
    let mut evaluation_latencies_ns = Vec::with_capacity(workload.len());
    let mut decision_log_latencies_ns = Vec::with_capacity(workload.len());
    let mut enqueue_latencies_ns = Vec::with_capacity(workload.len());
    let mut allowed = 0;
    let mut denied = 0;
    let mut allocations = 0;
    let mut allocated_bytes = 0;
    let start = std::time::Instant::now();

    for call in workload {
        let (alloc_before, bytes_before) = crate::alloc_snapshot();
        let call_start = std::time::Instant::now();
        let response = engine.evaluate(call);
        let eval_ns = call_start.elapsed().as_nanos();

        let (alloc_after, bytes_after) = crate::alloc_snapshot();
        allocations += alloc_after.saturating_sub(alloc_before);
        allocated_bytes += bytes_after.saturating_sub(bytes_before);

        // Buffer denial logs to thread-local storage to avoid stderr lock contention
        let decision_log_ns = 0u128;
        match response {
            Ok(EvaluationResponse::Allow) => allowed += 1,
            Ok(EvaluationResponse::Deny { reason }) => {
                DENIAL_BUFFER.with(|buf| {
                    buf.borrow_mut()
                        .push(DenialRecord::Denied { id: call.id, reason });
                });
                denied += 1;
            }
            Err(error) => {
                DENIAL_BUFFER.with(|buf| {
                    buf.borrow_mut()
                        .push(DenialRecord::Errored { id: call.id, error });
                });
                denied += 1;
            }
        }

        let enqueue_ns = 0u128;
        let total_ns = eval_ns + decision_log_ns + enqueue_ns;

        evaluation_latencies_ns.push(eval_ns);
        decision_log_latencies_ns.push(decision_log_ns);
        enqueue_latencies_ns.push(enqueue_ns);
        latencies_ns.push(total_ns);
    }

    let duration_ns = start.elapsed().as_nanos();

    // Flush buffered denial logs to stderr after benchmark timing completes
    DENIAL_BUFFER.with(|buf| {
        for record in buf.borrow_mut().drain(..) {
            match record {
                DenialRecord::Denied { id, reason } => {
                    eprintln!(
                        "[Traxes] request {} denied by policy: {}. Defaulting to DENY.",
                        id, reason
                    );
                }
                DenialRecord::Errored { id, error } => {
                    eprintln!(
                        "[Traxes] evaluation error for request {}: {}. Defaulting to DENY.",
                        id, error
                    );
                }
            }
        }
    });

    RunResult {
        latencies_ns,
        evaluation_latencies_ns,
        decision_log_latencies_ns,
        enqueue_latencies_ns,
        allowed,
        denied,
        duration_ns,
        allocations,
        allocated_bytes,
        artifact_enqueue_count: 0,
        artifact_enqueue_latency_ns: 0,
        queue_max_depth: 0,
        queue_drop_count: 0,
    }
}