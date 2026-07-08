use crate::replay::ReplayResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Match,
    Mismatch,
    PolicyVersionMismatch,
    ArtifactCorrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub replay_result: ReplayResult,
    pub details: String,
}

impl VerificationResult {
    pub fn new(replay_result: ReplayResult, status: VerificationStatus, details: String) -> Self {
        Self {
            status,
            replay_result,
            details,
        }
    }

    pub fn is_match(&self) -> bool {
        self.status == VerificationStatus::Match
    }

    pub fn is_mismatch(&self) -> bool {
        self.status == VerificationStatus::Mismatch
    }
}

pub struct Verifier;

impl Verifier {
    pub fn verify(replay_result: &ReplayResult, expected_policy_hash: Option<&str>) -> VerificationResult {
        // Check policy version consistency if provided
        if let Some(expected_hash) = expected_policy_hash {
            if replay_result.policy_hash != expected_hash {
                return VerificationResult::new(
                    replay_result.clone(),
                    VerificationStatus::PolicyVersionMismatch,
                    format!(
                        "Policy hash mismatch: expected {}, got {}",
                        expected_hash, replay_result.policy_hash
                    ),
                );
            }
        }

        // Check decision match
        if replay_result.match_status {
            VerificationResult::new(
                replay_result.clone(),
                VerificationStatus::Match,
                "Replay decision matches original decision".to_string(),
            )
        } else {
            VerificationResult::new(
                replay_result.clone(),
                VerificationStatus::Mismatch,
                format!(
                    "Decision mismatch: original={}, replay={}",
                    replay_result.original_decision, replay_result.replay_decision
                ),
            )
        }
    }

    pub fn verify_with_tolerance(
        replay_result: &ReplayResult,
        value_tolerance: f64,
    ) -> VerificationResult {
        // First check decision match
        if !replay_result.match_status {
            return VerificationResult::new(
                replay_result.clone(),
                VerificationStatus::Mismatch,
                format!(
                    "Decision mismatch: original={}, replay={}",
                    replay_result.original_decision, replay_result.replay_decision
                ),
            );
        }

        // Check if observed values are within tolerance
        let details = &replay_result.evaluation_details;
        
        // For numeric fields, check tolerance
        if !details.field.contains("instance_type") {
            let original_val = details.original_observed_value.as_f64().unwrap_or(0.0);
            let replay_val = details.replay_observed_value.as_f64().unwrap_or(0.0);
            
            if (original_val - replay_val).abs() > value_tolerance {
                return VerificationResult::new(
                    replay_result.clone(),
                    VerificationStatus::Mismatch,
                    format!(
                        "Value tolerance exceeded: original={}, replay={}, tolerance={}",
                        original_val, replay_val, value_tolerance
                    ),
                );
            }
        }

        VerificationResult::new(
            replay_result.clone(),
            VerificationStatus::Match,
            "Replay decision matches with value tolerance".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::replay::EvaluationDetails;
    use crate::replay::ReplayResult;
    use serde_json::json;

    fn create_test_replay_result(match_status: bool) -> ReplayResult {
        ReplayResult {
            original_decision: if match_status { "ALLOW" } else { "DENY" }.to_string(),
            replay_decision: "ALLOW".to_string(),
            decision_id: "test-123".to_string(),
            policy_hash: "hash-123".to_string(),
            match_status,
            replay_timestamp: chrono::Utc::now().to_rfc3339(),
            evaluation_details: EvaluationDetails {
                original_observed_value: json!(1.50),
                replay_observed_value: json!(1.50),
                original_policy_value: 2.00,
                replay_policy_value: 2.00,
                field: "instance_cost_per_hour".to_string(),
                rule: "numeric_lte".to_string(),
            },
        }
    }

    #[test]
    fn test_verification_match() {
        let replay_result = create_test_replay_result(true);
        let verification = Verifier::verify(&replay_result, None);

        assert_eq!(verification.status, VerificationStatus::Match);
        assert!(verification.is_match());
        assert!(!verification.is_mismatch());
    }

    #[test]
    fn test_verification_mismatch() {
        let replay_result = create_test_replay_result(false);
        let verification = Verifier::verify(&replay_result, None);

        assert_eq!(verification.status, VerificationStatus::Mismatch);
        assert!(!verification.is_match());
        assert!(verification.is_mismatch());
    }

    #[test]
    fn test_policy_version_mismatch() {
        let replay_result = create_test_replay_result(true);
        let verification = Verifier::verify(&replay_result, Some("different-hash"));

        assert_eq!(verification.status, VerificationStatus::PolicyVersionMismatch);
        assert!(!verification.is_match());
    }

    #[test]
    fn test_verification_with_tolerance_match() {
        let replay_result = create_test_replay_result(true);
        let verification = Verifier::verify_with_tolerance(&replay_result, 0.01);

        assert_eq!(verification.status, VerificationStatus::Match);
    }

    #[test]
    fn test_verification_with_tolerance_exceeded() {
        let mut replay_result = create_test_replay_result(true);
        replay_result.evaluation_details.replay_observed_value = json!(2.00); // Different from original 1.50
        
        let verification = Verifier::verify_with_tolerance(&replay_result, 0.01);

        assert_eq!(verification.status, VerificationStatus::Mismatch);
        assert!(verification.details.contains("Value tolerance exceeded"));
    }

    #[test]
    fn test_verification_result_serialization() {
        let replay_result = create_test_replay_result(true);
        let verification = Verifier::verify(&replay_result, None);

        let serialized = serde_json::to_string(&verification).unwrap();
        let deserialized: VerificationResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.status, verification.status);
        assert_eq!(deserialized.details, verification.details);
    }

    #[test]
    fn test_instance_type_field_skips_tolerance() {
        let mut replay_result = create_test_replay_result(true);
        replay_result.evaluation_details.field = "instance_type".to_string();
        replay_result.evaluation_details.original_observed_value = json!("t3.medium");
        replay_result.evaluation_details.replay_observed_value = json!("t3.small"); // Different but should skip tolerance check
        
        let verification = Verifier::verify_with_tolerance(&replay_result, 0.01);

        // Should still match because instance_type fields skip tolerance checks
        assert_eq!(verification.status, VerificationStatus::Match);
    }
}
