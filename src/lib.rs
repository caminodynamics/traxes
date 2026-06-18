//! Library crate for traxes-demo policy evaluation engine
//! 
//! This library provides the core policy evaluation functionality for the
//! Traxes decision runtime, including:
//! - Policy loading and parsing
//! - Action evaluation against policies
//! - Artifact generation for audit trails
//! - Evaluation engine orchestration

pub mod action;
pub mod artifact;
pub mod artifact_emitter;
pub mod async_logger;
pub mod cli_utils;
pub mod engine;
pub mod evaluate;
pub mod execution_event;
pub mod metrics;
pub mod policy;
pub mod policy_bundle;
pub mod policy_old;
pub mod server_policy;
pub mod traxes_engine;
pub mod workload;

// Memory allocation tracking utilities
#[cfg(feature = "metrics")]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "metrics")]
static TOTAL_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "metrics")]
static TOTAL_ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "metrics")]
pub fn alloc_snapshot() -> (usize, usize) {
    (
        TOTAL_ALLOCATIONS.load(Ordering::Relaxed),
        TOTAL_ALLOCATED_BYTES.load(Ordering::Relaxed),
    )
}

#[cfg(not(feature = "metrics"))]
pub fn alloc_snapshot() -> (usize, usize) {
    (0, 0)
}

// Re-export commonly used types for convenience
pub use action::ProposedAction;
pub use artifact::{AuditArtifact, ArtifactLogger};
pub use traxes_engine::{DecisionRecord, Engine, EvaluationDecision};
pub use server_policy::{EvaluationResult, PolicyEvaluator, Rule};
