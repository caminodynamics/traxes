use crate::engine::{EvaluationResponse, TraxesError};
use crate::workload::ToolCall;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    max_cost: f64,
    allowed_regions: Vec<String>,
    max_scale_size: u64,
    allow_delete_database: bool,
}

impl PolicyEngine {
    pub fn default() -> Self {
        Self {
            max_cost: 2.0,
            allowed_regions: vec!["us-east-1".into(), "us-west-2".into()],
            max_scale_size: 4,
            allow_delete_database: false,
        }
    }

    pub fn evaluate(&self, call: &ToolCall) -> Result<EvaluationResponse, TraxesError> {
        let tool = call
            .payload
            .get("tool")
            .and_then(Value::as_str)
            .ok_or_else(|| TraxesError::MissingTargetField("tool"))?;

        let cost = call
            .payload
            .get("cost")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let region = call
            .payload
            .get("region")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let scale_to = call
            .payload
            .get("scale_to")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let force = call
            .payload
            .get("force")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let allowed = match tool {
            "deploy_instance" | "create_vm" => {
                cost <= self.max_cost && self.allowed_regions.contains(&region.to_string())
            }
            "scale_cluster" => scale_to > 0 && scale_to <= self.max_scale_size,
            "delete_database" => self.allow_delete_database,
            "delete_db" => self.allow_delete_database,
            "destroy_instance" => !force,
            _ => true,
        };

        if allowed {
            Ok(EvaluationResponse::Allow)
        } else {
            Ok(EvaluationResponse::Deny {
                reason: format!("policy denied tool={}", tool).into(),
            })
        }
    }
}
