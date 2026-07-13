//! Unified execution engine: same evaluate path for HTTP server and CLI.

use crate::action::ProposedAction;
use crate::cli_utils;
use crate::policy_bundle::{evaluate_action_policy_with_rules, parse_policy_rules};
use crate::server_policy::EvaluationResult;
use sha2::{Digest, Sha256};
use std::io;
use std::time::Instant;

pub const DEFAULT_POLICY_YAML: &str = include_str!("../policies/aws_staging_guardrails.yaml");

#[derive(Debug, Clone)]
pub struct ParsedRule {
    pub operator: String,
    pub field: String,
    pub allowed_values: Vec<String>,
    pub rule_id: String,
}

impl ParsedRule {
    pub fn new(operator: String, field: String, allowed_values: Vec<String>, rule_id: String) -> Self {
        Self {
            operator,
            field,
            allowed_values,
            rule_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PolicyBundle {
    pub policy_yaml: String,
    pub policy_hash: String,
    pub parsed_rules: Vec<ParsedRule>,
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
        // Only log diagnostic messages outside of demo mode or if debug is enabled
        let show_diagnostics = !crate::cli_utils::is_demo_mode() || crate::cli_utils::is_debug_mode();
        let policy_yaml = DEFAULT_POLICY_YAML.to_string();
        let policy_hash = calculate_hash_from_content(&policy_yaml);
        let parsed_rules = crate::policy_bundle::parse_policy_rules(&policy_yaml);
        if show_diagnostics {
            cli_utils::debug_log("[Traxes] Loading embedded policy (compile-time)");
            cli_utils::debug_log(format!("[Traxes] Policy loaded successfully. Hash: {}", policy_hash));
            cli_utils::debug_log(format!("[Traxes] Parsed {} rules from policy", parsed_rules.len()));
        }
        Ok(Self {
            bundle: PolicyBundle {
                policy_yaml,
                policy_hash,
                parsed_rules,
            },
        })
    }

    pub fn with_policy(policy_yaml: String) -> Self {
        let policy_hash = calculate_hash_from_content(&policy_yaml);
        let parsed_rules = crate::policy_bundle::parse_policy_rules(&policy_yaml);
        Self {
            bundle: PolicyBundle {
                policy_yaml,
                policy_hash,
                parsed_rules,
            },
        }
    }

    pub fn evaluate(&self, action: &ProposedAction) -> EvaluationDecision {
        let start = Instant::now();
        let result = evaluate_action_policy_with_rules(action, &self.bundle.parsed_rules);
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
        evaluate_action_policy_with_rules(action, &self.bundle.parsed_rules)
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
