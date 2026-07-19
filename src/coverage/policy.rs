use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct CoveragePolicyConfig {
    #[serde(flatten)]
    pub tools: HashMap<String, ToolConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolConfig {
    pub requires_traxes: bool,
}

pub fn load_coverage_policy() -> Result<CoveragePolicyConfig, Box<dyn std::error::Error>> {
    let policy_path = "coverage_policy.yaml";
    
    // If policy file doesn't exist, return empty policy
    if !std::path::Path::new(policy_path).exists() {
        return Ok(CoveragePolicyConfig {
            tools: HashMap::new(),
        });
    }
    
    let content = fs::read_to_string(policy_path)?;
    let policy: CoveragePolicyConfig = serde_yaml::from_str(&content)?;
    Ok(policy)
}

pub fn load_coverage_policy_from_path(path: &str) -> Result<CoveragePolicyConfig, Box<dyn std::error::Error>> {
    // If policy file doesn't exist, return empty policy
    if !std::path::Path::new(path).exists() {
        return Ok(CoveragePolicyConfig {
            tools: HashMap::new(),
        });
    }
    
    let content = fs::read_to_string(path)?;
    let policy: CoveragePolicyConfig = serde_yaml::from_str(&content)?;
    Ok(policy)
}

pub fn tool_requires_traxes(tool: &str, policy: &CoveragePolicyConfig) -> bool {
    policy.tools
        .get(tool)
        .map(|config| config.requires_traxes)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_load_coverage_policy() {
        // Create unique temporary directory for this test run
        let temp_dir = std::env::temp_dir().join(format!("traxes_policy_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        
        let policy_path = temp_dir.join("test_coverage_policy.yaml");
        
        // Create temporary policy file
        let policy_content = r#"
AWS_RDS_PROVISION:
  requires_traxes: true
AWS_S3_CREATE_BUCKET:
  requires_traxes: false
"#;
        
        fs::write(&policy_path, policy_content).unwrap();
        
        let policy = load_coverage_policy_from_path(policy_path.to_str().unwrap()).unwrap();
        
        assert!(tool_requires_traxes("AWS_RDS_PROVISION", &policy));
        assert!(!tool_requires_traxes("AWS_S3_CREATE_BUCKET", &policy));
        assert!(!tool_requires_traxes("UNKNOWN_TOOL", &policy));
        
        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_missing_policy() {
        let policy = load_coverage_policy_from_path("nonexistent_policy.yaml").unwrap();
        assert_eq!(policy.tools.len(), 0);
    }

    #[test]
    fn test_tool_requires_traxes() {
        let mut tools = HashMap::new();
        tools.insert("AWS_RDS_PROVISION".to_string(), ToolConfig { requires_traxes: true });
        tools.insert("AWS_S3_CREATE_BUCKET".to_string(), ToolConfig { requires_traxes: false });
        
        let policy = CoveragePolicyConfig { tools };
        
        assert!(tool_requires_traxes("AWS_RDS_PROVISION", &policy));
        assert!(!tool_requires_traxes("AWS_S3_CREATE_BUCKET", &policy));
        assert!(!tool_requires_traxes("UNKNOWN_TOOL", &policy));
    }
}
