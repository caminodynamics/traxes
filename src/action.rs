use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProposedAction {
    pub tool: String,
    pub session_id: String,
    pub environment: String,
    pub parameters: Parameters,
}

pub type Parameters = serde_json::Value;
