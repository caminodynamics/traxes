use crate::action::ProposedAction;
use crate::artifact::AuditArtifact;
use crate::traxes_engine::Engine;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRequest {
    pub artifact_path: String,
    pub policy_yaml: Option<String>, // If None, use embedded policy
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    pub original_decision: String,
    pub replay_decision: String,
    pub decision_id: String,
    pub policy_hash: String,
    pub match_status: bool,
    pub replay_timestamp: String,
    pub evaluation_details: EvaluationDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationDetails {
    pub original_observed_value: serde_json::Value,
    pub replay_observed_value: serde_json::Value,
    pub original_policy_value: f64,
    pub replay_policy_value: f64,
    pub field: String,
    pub rule: String,
}

pub struct ReplayEngine {
    engine: Engine,
}

impl ReplayEngine {
    pub fn new(engine: Engine) -> Self {
        Self { engine }
    }

    pub fn with_default_policy() -> Result<Self, Box<dyn std::error::Error>> {
        let engine = Engine::load_default_policies()?;
        Ok(Self::new(engine))
    }

    pub fn replay_from_artifact(&self, artifact: &AuditArtifact) -> ReplayResult {
        // Verify policy hash consistency before replay
        let policy_hash_matches = self.verify_policy_consistency(artifact);
        
        // Reconstruct the ProposedAction from artifact data
        let action = self.reconstruct_action(artifact);

        // Re-evaluate using the same engine
        let replay_decision = self.engine.evaluate(&action);

        // Compare with original decision
        let match_status = policy_hash_matches && (artifact.decision == replay_decision.decision);

        // Extract evaluation details
        let evaluation_details = EvaluationDetails {
            original_observed_value: artifact.rule_evaluation.observed_value.clone(),
            replay_observed_value: if artifact.rule_evaluation.field.contains("instance_type") {
                replay_decision.result.observed_value_str.clone().into()
            } else {
                serde_json::Number::from_f64(replay_decision.result.observed_value)
                    .unwrap_or(serde_json::Number::from(0))
                    .into()
            },
            original_policy_value: artifact.rule_evaluation.policy_value,
            replay_policy_value: replay_decision.result.policy_value,
            field: artifact.rule_evaluation.field.clone(),
            rule: artifact.rule_evaluation.operator.clone(),
        };

        ReplayResult {
            original_decision: artifact.decision.clone(),
            replay_decision: replay_decision.decision,
            decision_id: artifact.decision_id.clone(),
            policy_hash: artifact.policy_hash.clone(),
            match_status,
            replay_timestamp: chrono::Utc::now().to_rfc3339(),
            evaluation_details,
        }
    }

    pub fn replay_from_file(&self, artifact_path: &str) -> Result<ReplayResult, Box<dyn std::error::Error>> {
        let artifact_content = fs::read_to_string(artifact_path)?;
        let artifact: AuditArtifact = serde_json::from_str(&artifact_content)?;
        Ok(self.replay_from_artifact(&artifact))
    }

    pub fn replay(&self, request: ReplayRequest) -> Result<ReplayResult, Box<dyn std::error::Error>> {
        if let Some(policy_yaml) = request.policy_yaml {
            // Use custom policy
            let engine = Engine::with_policy(policy_yaml);
            let custom_replay = ReplayEngine::new(engine);
            let artifact = self.load_artifact(&request.artifact_path)?;
            Ok(custom_replay.replay_from_artifact(&artifact))
        } else {
            // Use default policy
            self.replay_from_file(&request.artifact_path)
        }
    }

    pub fn reconstruct_action(&self, artifact: &AuditArtifact) -> ProposedAction {
        ProposedAction {
            tool: artifact.proposed_action.tool.clone(),
            session_id: artifact.execution_context.session_id.clone(),
            environment: artifact.proposed_action.environment.clone(),
            parameters: serde_json::json!({
                "instance_type": artifact.proposed_action.parameters.instance_type,
                "instance_cost_per_hour": artifact.proposed_action.parameters.instance_cost_per_hour
            }),
        }
    }

    fn load_artifact(&self, path: &str) -> Result<AuditArtifact, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let artifact: AuditArtifact = serde_json::from_str(&content)?;
        Ok(artifact)
    }

    pub fn verify_policy_consistency(&self, artifact: &AuditArtifact) -> bool {
        let current_policy_hash = self.engine.policy_hash();
        artifact.policy_hash == current_policy_hash
    }

    /// Complete replay verification workflow
    /// Returns a formatted verification report
    pub fn verify_artifact(&self, artifact_path: &str) -> Result<VerificationReport, Box<dyn std::error::Error>> {
        let replay_result = self.replay_from_file(artifact_path)?;
        
        let verification_status = if replay_result.match_status {
            "PASS".to_string()
        } else {
            "FAIL".to_string()
        };

        Ok(VerificationReport {
            decision_id: replay_result.decision_id,
            original_decision: replay_result.original_decision,
            replay_decision: replay_result.replay_decision,
            verification_status,
            policy_hash: replay_result.policy_hash,
            replay_timestamp: replay_result.replay_timestamp,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub decision_id: String,
    pub original_decision: String,
    pub replay_decision: String,
    pub verification_status: String,
    pub policy_hash: String,
    pub replay_timestamp: String,
}

impl VerificationReport {
    pub fn display(&self) -> String {
        format!(
            "TRAXES Replay Verification\n\nDecision ID:\n{}\n\nOriginal Decision:\n{}\n\nReplay Decision:\n{}\n\nVerification:\n{}",
            self.decision_id, self.original_decision, self.replay_decision, self.verification_status
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_replay_engine_creation() {
        let engine = ReplayEngine::with_default_policy();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_action_reconstruction() {
        let engine = ReplayEngine::with_default_policy().unwrap();
        
        // Create a mock artifact
        let artifact = AuditArtifact {
            artifact_version: "1.0.0".to_string(),
            artifact_type: "test".to_string(),
            decision_id: "test-123".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            decision: "ALLOW".to_string(),
            tool: "aws_ec2_provision".to_string(),
            environment: "staging".to_string(),
            reason: String::new(),
            policy_bundle: "test".to_string(),
            policy_hash: "test-hash".to_string(),
            sha256_hash: "test-sha256".to_string(),
            engine: crate::artifact::EngineInfo {
                name: "Traxes".to_string(),
                engine_version: "0.3.2".to_string(),
                policy_bundle_id: "test".to_string(),
            },
            execution_context: crate::artifact::ExecutionContext {
                session_id: "session-123".to_string(),
                trace_id: "trace-123".to_string(),
            },
            proposed_action: crate::artifact::ProposedActionInfo {
                tool: "aws_ec2_provision".to_string(),
                environment: "staging".to_string(),
                parameters: crate::artifact::ActionParameters {
                    instance_type: "t3.medium".to_string(),
                    instance_cost_per_hour: 1.50,
                },
            },
            rule_evaluation: crate::artifact::RuleEvaluationInfo {
                rule_id: "test".to_string(),
                field: "instance_cost_per_hour".to_string(),
                observed_value: json!(1.50),
                operator: "numeric_lte".to_string(),
                policy_value: 2.00,
                evaluation_expression: "test".to_string(),
                evaluation_result: false,
                rule_order: Some(0),
            },
            rules_evaluated: None,
            performance: crate::artifact::PerformanceInfo {
                evaluation_latency_us: 100.0,
                decision_latency_us: 50.0,
                artifact_write_latency_us: 25.0,
            },
            side_effect_prevention: crate::artifact::SideEffectPreventionInfo {
                decision_effect: "ALLOW".to_string(),
            },
            policy_info: None,
            governance_info: None,
            evaluation_trace: None,
            execution_status: "executed".to_string(),
        };

        let action = engine.reconstruct_action(&artifact);
        
        assert_eq!(action.tool, "aws_ec2_provision");
        assert_eq!(action.session_id, "session-123");
        assert_eq!(action.environment, "staging");
        assert_eq!(
            action.parameters.get("instance_type").and_then(|v| v.as_str()),
            Some("t3.medium")
        );
        assert_eq!(
            action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()),
            Some(1.50)
        );
    }

    #[test]
    fn test_replay_result_structure() {
        let result = ReplayResult {
            original_decision: "ALLOW".to_string(),
            replay_decision: "ALLOW".to_string(),
            decision_id: "test-123".to_string(),
            policy_hash: "hash-123".to_string(),
            match_status: true,
            replay_timestamp: chrono::Utc::now().to_rfc3339(),
            evaluation_details: EvaluationDetails {
                original_observed_value: json!(1.50),
                replay_observed_value: json!(1.50),
                original_policy_value: 2.00,
                replay_policy_value: 2.00,
                field: "instance_cost_per_hour".to_string(),
                rule: "numeric_lte".to_string(),
            },
        };

        assert_eq!(result.original_decision, "ALLOW");
        assert_eq!(result.replay_decision, "ALLOW");
        assert!(result.match_status);
        assert_eq!(result.decision_id, "test-123");

        // Test serialization
        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: ReplayResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.decision_id, result.decision_id);
        assert_eq!(deserialized.match_status, result.match_status);
    }
}
