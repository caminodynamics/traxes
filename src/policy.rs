use crate::engine::{EvaluationResponse, TraxesError};
use crate::workload::ToolCall;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
 
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    max_cost: f64,
    allowed_regions: HashSet<String>,
    max_scale_size: u64,
    allow_delete_database: bool,
}
 
impl PolicyEngine {
    pub fn default() -> Self {
        let mut allowed_regions = HashSet::with_capacity(2);
        allowed_regions.insert("us-east-1".into());
        allowed_regions.insert("us-west-2".into());
        Self {
            max_cost: 2.0,
            allowed_regions,
            max_scale_size: 4,
            allow_delete_database: false,
        }
    }
 
    pub fn evaluate(&self, call: &ToolCall) -> Result<EvaluationResponse, TraxesError> {
        let tool = call
            .payload
            .get("tool")
            .and_then(Value::as_str)
            .ok_or(TraxesError::MissingTargetField("tool"))?;
 
        let allowed = match tool {
            "deploy_instance" | "create_vm" => {
                let cost = call.payload.get("cost").and_then(Value::as_f64).unwrap_or(0.0);
                let region = call.payload.get("region").and_then(Value::as_str).unwrap_or("unknown");
                cost <= self.max_cost && self.allowed_regions.contains(region)
            }
            "scale_cluster" => {
                let scale_to = call.payload.get("scale_to").and_then(Value::as_u64).unwrap_or(0);
                scale_to > 0 && scale_to <= self.max_scale_size
            }
            "delete_database" | "delete_db" => self.allow_delete_database,
            "destroy_instance" => {
                let force = call.payload.get("force").and_then(Value::as_bool).unwrap_or(false);
                !force
            }
            _ => true,
        };
 
        if allowed {
            Ok(EvaluationResponse::Allow)
        } else {
            Ok(EvaluationResponse::Deny {
                reason: deny_reason(tool),
            })
        }
    }
}
 
#[inline]
fn deny_reason(tool: &str) -> Cow<'static, str> {
    match tool {
        "deploy_instance"  => Cow::Borrowed("policy denied tool=deploy_instance"),
        "create_vm"        => Cow::Borrowed("policy denied tool=create_vm"),
        "scale_cluster"    => Cow::Borrowed("policy denied tool=scale_cluster"),
        "delete_database"  => Cow::Borrowed("policy denied tool=delete_database"),
        "delete_db"        => Cow::Borrowed("policy denied tool=delete_db"),
        "destroy_instance" => Cow::Borrowed("policy denied tool=destroy_instance"),
        other              => Cow::Owned(format!("policy denied tool={}", other)),
    }
}