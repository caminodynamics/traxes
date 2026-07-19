use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CoverageStatus {
    GOVERNED,
    UNGOVERNED,
    UNKNOWN,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageRecord {
    pub coverage_id: String,
    pub timestamp: String,
    pub action_tool: String,
    pub expected_control_path: String,
    pub observed_control_path: String,
    pub coverage_status: CoverageStatus,
    pub decision_id: Option<String>,
}

impl CoverageRecord {
    pub fn new(
        action_tool: String,
        expected_control_path: String,
        observed_control_path: String,
        coverage_status: CoverageStatus,
        decision_id: Option<String>,
    ) -> Self {
        Self {
            coverage_id: format!("cov_{}", Uuid::new_v4().to_string().replace("-", "")),
            timestamp: Utc::now().to_rfc3339(),
            action_tool,
            expected_control_path,
            observed_control_path,
            coverage_status,
            decision_id,
        }
    }

    pub fn write_to_file(&self) -> Result<String, Box<dyn std::error::Error>> {
        std::fs::create_dir_all("coverage")?;
        let file_path = format!("coverage/CoverageRecord_{}.json", self.coverage_id);
        std::fs::write(&file_path, serde_json::to_string_pretty(self)?)?;
        Ok(file_path)
    }

    pub fn write_to_dir(&self, dir: &str) -> Result<String, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(dir)?;
        let file_path = format!("{}/CoverageRecord_{}.json", dir, self.coverage_id);
        std::fs::write(&file_path, serde_json::to_string_pretty(self)?)?;
        Ok(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_coverage_record_creation() {
        let record = CoverageRecord::new(
            "AWS_RDS_PROVISION".to_string(),
            "traxes".to_string(),
            "traxes".to_string(),
            CoverageStatus::GOVERNED,
            Some("dec_123".to_string()),
        );
        
        assert!(record.coverage_id.starts_with("cov_"));
        assert_eq!(record.action_tool, "AWS_RDS_PROVISION");
        assert_eq!(record.expected_control_path, "traxes");
        assert_eq!(record.observed_control_path, "traxes");
        assert!(matches!(record.coverage_status, CoverageStatus::GOVERNED));
        assert_eq!(record.decision_id, Some("dec_123".to_string()));
    }

    #[test]
    fn test_coverage_record_serialization() {
        let record = CoverageRecord::new(
            "AWS_S3_CREATE_BUCKET".to_string(),
            "traxes".to_string(),
            "direct".to_string(),
            CoverageStatus::UNGOVERNED,
            None,
        );
        
        let serialized = serde_json::to_string(&record).unwrap();
        let deserialized: CoverageRecord = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(record.coverage_id, deserialized.coverage_id);
        assert_eq!(record.action_tool, deserialized.action_tool);
        assert_eq!(record.coverage_status, deserialized.coverage_status);
    }

    #[test]
    fn test_coverage_record_write_and_read() {
        // Create unique temporary directory for this test run
        let temp_dir = std::env::temp_dir().join(format!("traxes_record_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        
        let record = CoverageRecord::new(
            "AWS_EC2_PROVISION".to_string(),
            "traxes".to_string(),
            "traxes".to_string(),
            CoverageStatus::GOVERNED,
            Some("dec_456".to_string()),
        );
        
        let file_path = record.write_to_dir(temp_dir.to_str().unwrap()).unwrap();
        assert!(Path::new(&file_path).exists());
        
        let content = fs::read_to_string(&file_path).unwrap();
        let read_record: CoverageRecord = serde_json::from_str(&content).unwrap();
        
        assert_eq!(record.coverage_id, read_record.coverage_id);
        
        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
