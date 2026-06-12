//! Unified execution engine: same evaluate path for HTTP server and CLI.

use crate::action::ProposedAction;
use crate::cli;
use crate::policy_bundle::{evaluate_action_policy, read_policy_yaml};
use crate::server_policy::EvaluationResult;
use sha2::{Digest, Sha256};
use std::io;
use std::time::Instant;

pub const DEFAULT_POLICY_PATH: &str = "../demo/policies/aws_staging_guardrails.yaml";

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

impl EvaluationDecision {
    pub fn is_deny(&self) -> bool {
        self.decision == "DENY"
    }
}

impl Engine {
    pub fn load_default_policies() -> Result<Self, io::Error> {
        Self::load(DEFAULT_POLICY_PATH)
    }

    pub fn load(policy_path: &str) -> Result<Self, io::Error> {
        let policy_yaml = read_policy_yaml(policy_path)?;
        let policy_hash = calculate_policy_hash(policy_path)?;
        Ok(Self {
            bundle: PolicyBundle {
                policy_yaml,
                policy_hash,
            },
        })
    }

    pub fn evaluate(&self, action: &ProposedAction) -> EvaluationDecision {
        let start = Instant::now();
        let result = evaluate_action_policy(action, &self.bundle.policy_yaml);
        let evaluation_latency_us = start.elapsed().as_micros() as f64;
        let (decision, _) = cli::normalize_decision(&result);
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

pub fn calculate_policy_hash(file_path: &str) -> io::Result<String> {
    let raw_content = std::fs::read_to_string(file_path)?;
    let normalized = raw_content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches(|c| c == '\n' || c == '\r')
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}
