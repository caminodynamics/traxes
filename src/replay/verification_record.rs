use chrono::Utc;
use serde::{Serialize, Deserialize};

/// Separate verification record for replay results
/// Keeps original decision artifacts immutable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayVerificationRecord {
    pub decision_id: String,
    pub artifact_path: String,
    pub replay_timestamp: String,
    pub original_decision: String,
    pub replay_decision: String,
    pub verification_status: String,  // "VERIFIED" | "FAILED" | "NOT_ATTEMPTED"
    pub match_result: String,  // "MATCH" | "MISMATCH"
    pub policy_hash: String,
    pub replay_count: u32,
    pub verification_details: Option<VerificationDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationDetails {
    pub original_observed_value: serde_json::Value,
    pub replay_observed_value: serde_json::Value,
    pub original_policy_value: f64,
    pub replay_policy_value: f64,
    pub field: String,
    pub operator: String,
}

impl ReplayVerificationRecord {
    pub fn new(
        decision_id: &str,
        artifact_path: &str,
        original_decision: &str,
        replay_decision: &str,
        policy_hash: &str,
        verification_details: Option<VerificationDetails>,
    ) -> Self {
        let match_result = if original_decision == replay_decision {
            "MATCH".to_string()
        } else {
            "MISMATCH".to_string()
        };

        let verification_status = if match_result == "MATCH" {
            "VERIFIED".to_string()
        } else {
            "FAILED".to_string()
        };

        Self {
            decision_id: decision_id.to_string(),
            artifact_path: artifact_path.to_string(),
            replay_timestamp: Utc::now().to_rfc3339(),
            original_decision: original_decision.to_string(),
            replay_decision: replay_decision.to_string(),
            verification_status,
            match_result,
            policy_hash: policy_hash.to_string(),
            replay_count: 1,
            verification_details,
        }
    }

    pub fn increment_replay_count(&mut self) {
        self.replay_count += 1;
        self.replay_timestamp = Utc::now().to_rfc3339();
    }

    pub fn write_to_file(&self) -> Result<String, Box<dyn std::error::Error>> {
        let file_path = format!("artifacts/VerificationRecord_{}.json", self.decision_id);
        std::fs::write(&file_path, serde_json::to_string_pretty(self)?)?;
        Ok(file_path)
    }

    pub fn display(&self) -> String {
        format!(
            "TRAXES Replay Verification Record\n\nDecision ID:\n{}\n\nOriginal Decision:\n{}\n\nReplay Decision:\n{}\n\nVerification Status:\n{}\n\nMatch Result:\n{}\n\nReplay Count:\n{}",
            self.decision_id, self.original_decision, self.replay_decision, 
            self.verification_status, self.match_result, self.replay_count
        )
    }
}
