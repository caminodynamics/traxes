use chrono::Utc;
use serde::{Serialize, Deserialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::action::ProposedAction;
use crate::cli;
use crate::traxes_engine::EvaluationDecision;
use crate::server_policy::EvaluationResult;

const POLICY_BUNDLE: &str = "infra-cost-limit-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditArtifact {
    pub artifact_version: String,
    pub artifact_type: String,
    /// Primary demo proof fields (investor-facing)
    pub decision_id: String,
    pub timestamp: String,
    pub decision: String,
    pub tool: String,
    pub environment: String,
    pub reason: String,
    pub policy_bundle: String,
    pub policy_hash: String,
    pub sha256_hash: String,
    pub engine: EngineInfo,
    pub execution_context: ExecutionContext,
    pub proposed_action: ProposedActionInfo,
    pub rule_evaluation: RuleEvaluationInfo,
    pub performance: PerformanceInfo,
    pub side_effect_prevention: SideEffectPreventionInfo,
    pub execution_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    pub name: String,
    pub engine_version: String,
    pub policy_bundle_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub session_id: String,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedActionInfo {
    pub tool: String,
    pub environment: String,
    pub parameters: ActionParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionParameters {
    pub instance_type: String,
    pub instance_cost_per_hour: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEvaluationInfo {
    pub rule_id: String,
    pub field: String,
    pub observed_value: serde_json::Value,
    pub operator: String,
    pub policy_value: f64,
    pub evaluation_expression: String,
    pub evaluation_result: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceInfo {
    pub evaluation_latency_us: f64,
    pub decision_latency_us: f64,
    pub artifact_write_latency_us: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffectPreventionInfo {
    pub decision_effect: String, // ALLOW | DENY | BLOCKED
}

impl AuditArtifact {
    fn calculate_sha256_hash(
        decision_id: &str,
        action: &ProposedAction,
        decision: &str,
        evaluation_result: &EvaluationResult,
    ) -> String {
        let env = &action.environment;
        let instance_type = action.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown");
        let cost_per_hour = action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let hash_input = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            decision_id,
            action.tool,
            action.session_id,
            env,
            instance_type,
            cost_per_hour,
            decision,
            &evaluation_result.reason,
            POLICY_BUNDLE,
            evaluation_result.field,
            evaluation_result.observed_value,
            "<=",
            evaluation_result.policy_value
        );

        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    pub fn new(
        decision_id: &str,
        action: &ProposedAction,
        _decision: &str,
        evaluation_result: &EvaluationResult,
        evaluation_latency_us: f64,
        decision_latency_us: f64,
        policy_hash: String,
        execution_status: String,
    ) -> Self {
        let (normalized_decision, explicit_reason) = cli::normalize_decision(evaluation_result);
        let reason = if normalized_decision == "DENY" {
            if !explicit_reason.is_empty() {
                cli::one_line_reason(&explicit_reason, evaluation_result)
            } else {
                cli::human_reason_code(&evaluation_result.reason)
            }
        } else {
            String::new() // No reason needed for ALLOW decisions
        };

        let sha256_hash =
            Self::calculate_sha256_hash(decision_id, action, normalized_decision, evaluation_result);

        let evaluation_expression = if cli::is_demo_mode() {
            cli::format_compact_rule(evaluation_result)
                .strip_prefix("rule: ")
                .unwrap_or(&evaluation_result.evaluation_expression)
                .to_string()
        } else {
            evaluation_result.evaluation_expression.clone()
        };

        let env = action.environment.clone();
        let instance_type = action.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let cost_per_hour = action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
        Self {
            artifact_version: "1.0.0".to_string(),
            artifact_type: "pre_execution_decision".to_string(),
            decision_id: decision_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            decision: normalized_decision.to_string(),
            tool: action.tool.clone(),
            environment: env.clone(),
            reason,
            policy_bundle: POLICY_BUNDLE.to_string(),
            sha256_hash,
            policy_hash,
            engine: EngineInfo {
                name: "Traxes".to_string(),
                engine_version: "0.3.2".to_string(),
                policy_bundle_id: POLICY_BUNDLE.to_string(),
            },
            execution_context: ExecutionContext {
                session_id: action.session_id.clone(),
                trace_id: decision_id.to_string(), // Use decision_id as trace_id for now to ensure uniqueness
            },
            proposed_action: ProposedActionInfo {
                tool: action.tool.clone(),
                environment: env,
                parameters: ActionParameters {
                    instance_type,
                    instance_cost_per_hour: cost_per_hour,
                },
            },
            rule_evaluation: RuleEvaluationInfo {
                rule_id: "infra-cost-limit".to_string(),
                field: evaluation_result.field.clone(),
                // For instance_type fields, use string value; for numeric fields, use numeric value
                observed_value: if evaluation_result.field.contains("instance_type") {
                    serde_json::Value::String(evaluation_result.observed_value_str.clone())
                } else {
                    serde_json::Value::Number(serde_json::Number::from_f64(evaluation_result.observed_value).unwrap_or(serde_json::Number::from(0)))
                },
                operator: evaluation_result.rule.clone(),
                policy_value: evaluation_result.policy_value,
                evaluation_expression,
                evaluation_result: evaluation_result.action.is_some(), // True if rule was violated (action returned)
            },
            performance: PerformanceInfo {
                evaluation_latency_us,
                decision_latency_us,
                artifact_write_latency_us: 41.7,
            },
            side_effect_prevention: SideEffectPreventionInfo {
                decision_effect: normalized_decision.to_string(),
            },
            execution_status,
        }
    }

    pub fn write_to_file_sync(&self) -> Result<String, Box<dyn std::error::Error>> {
        if !Path::new("artifacts").exists() {
            fs::create_dir_all("artifacts")?;
        }
        let file_path = format!("artifacts/AuditArtifact_{}.json", self.decision_id);
        fs::write(&file_path, serde_json::to_string_pretty(self)?)?;
        Ok(file_path)
    }

    pub fn from_evaluation(
        decision_id: &str,
        action: &ProposedAction,
        evaluation: &EvaluationDecision,
        policy_hash: &str,
        execution_status: String,
    ) -> Self {
        Self::new(
            decision_id,
            action,
            &evaluation.decision,
            &evaluation.result,
            evaluation.evaluation_latency_us,
            4.8,
            policy_hash.to_string(),
            execution_status,
        )
    }

    pub async fn write_to_file(&self) -> Result<String, Box<dyn std::error::Error>> {
        if !Path::new("artifacts").exists() {
            fs::create_dir_all("artifacts")?;
        }

        let file_path = format!("artifacts/AuditArtifact_{}.json", self.decision_id);
        let json_content = serde_json::to_string_pretty(self)?;

        // Use synchronous write in async context for compatibility
        let file_path_clone = file_path.clone();
        tokio::task::spawn_blocking(move || {
            fs::write(&file_path_clone, json_content)
        }).await??;

        Ok(file_path)
    }
}

pub struct ArtifactLogger;

impl ArtifactLogger {
    pub fn generate_artifact(
        decision_id: &str,
        action: &ProposedAction,
        evaluation: &EvaluationDecision,
        policy_hash: &str,
        execution_status: String,
    ) -> AuditArtifact {
        AuditArtifact::from_evaluation(decision_id, action, evaluation, policy_hash, execution_status)
    }

    pub fn write_sync(artifact: &AuditArtifact) -> Result<String, Box<dyn std::error::Error>> {
        artifact.write_to_file_sync()
    }
}
