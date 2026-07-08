//! Unified execution engine: same evaluate path for HTTP server and CLI.

use crate::action::ProposedAction;
use crate::cli_utils;
use crate::policy_bundle::evaluate_action_policy;
use crate::server_policy::EvaluationResult;
use sha2::{Digest, Sha256};
use std::io;
use std::time::Instant;

pub const DEFAULT_POLICY_YAML: &str = include_str!("../policies/aws_staging_guardrails.yaml");

#[derive(Debug, Clone)]
pub struct PolicyBundle {
    pub policy_yaml: String,
    pub policy_hash: String,
}

#[derive(Debug, Clone)]
pub struct Engine {
    bundle: PolicyBundle,
}

#[derive(Debug, Clone)]
pub struct EvaluationDecision {
    pub result: EvaluationResult,
    pub decision: String,
    pub evaluation_latency_us: f64,
}

/// Lightweight decision record for hot path
/// Contains minimal data needed for async artifact construction
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    pub decision: String,
    pub session_id: String,
    pub tool: String,
    pub environment: String,
    pub parameters: serde_json::Value,
    pub policy_hash: String,
    pub evaluation_result: EvaluationResult,
    pub evaluation_latency_us: f64,
    pub decision_id: String,
    pub trace_id: String,
    pub execution_status: String,
}

impl EvaluationDecision {
    #[allow(dead_code)]
    pub fn is_deny(&self) -> bool {
        self.decision == "DENY"
    }
}

impl Engine {
    pub fn load_default_policies() -> Result<Self, io::Error> {
        eprintln!("[Traxes] Loading embedded policy (compile-time)");
        let policy_yaml = DEFAULT_POLICY_YAML.to_string();
        let policy_hash = calculate_hash_from_content(&policy_yaml);
        eprintln!("[Traxes] Policy loaded successfully. Hash: {}", policy_hash);
        Ok(Self {
            bundle: PolicyBundle {
                policy_yaml,
                policy_hash,
            },
        })
    }

    pub fn with_policy(policy_yaml: String) -> Self {
        let policy_hash = calculate_hash_from_content(&policy_yaml);
        Self {
            bundle: PolicyBundle {
                policy_yaml,
                policy_hash,
            },
        }
    }

    pub fn evaluate(&self, action: &ProposedAction) -> EvaluationDecision {
        let start = Instant::now();
        let result = evaluate_action_policy(action, &self.bundle.policy_yaml);
        let evaluation_latency_us = start.elapsed().as_micros() as f64;
        let (decision, _): (&str, String) = cli_utils::normalize_decision(&result);
        EvaluationDecision {
            result,
            decision: decision.to_string(),
            evaluation_latency_us,
        }
    }

    /// Raw evaluation that directly calls evaluate_action_policy without any overhead.
    /// This is used for benchmarking to measure only the core evaluation logic.
    pub fn evaluate_raw(&self, action: &ProposedAction) -> EvaluationResult {
        evaluate_action_policy(action, &self.bundle.policy_yaml)
    }

    pub fn policy_hash(&self) -> &str {
        &self.bundle.policy_hash
    }
}

pub fn calculate_hash_from_content(content: &str) -> String {
    let normalized = content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches(|c| c == '\n' || c == '\r')
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}
