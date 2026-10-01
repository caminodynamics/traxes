use rmcp::{
    handler::server::wrapper::Parameters,
    schemars,
    tool,
    tool_router,
    transport::stdio,
    ServiceExt,
};
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use traxes_demo::action::{self, ExecutionOutcome, ProposedAction};
use traxes_demo::artifact::ArtifactLogger;
use traxes_demo::replay::ReplayEngine;
use traxes_demo::traxes_engine::Engine;
use uuid::Uuid;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct WriteFileParams {
    /// Target path to write.
    path: String,
    /// Exact file contents to write when TRAXES authorizes the action.
    content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReplayParams {
    /// TRAXES decision ID whose artifact should be replayed.
    decision_id: String,
}

#[derive(Clone)]
struct TraxesMcpServer {
    engine: Arc<Engine>,
    policy_yaml: String,
}

impl TraxesMcpServer {
    fn new(policy_yaml: String) -> Self {
        Self {
            engine: Arc::new(Engine::with_policy(policy_yaml.clone())),
            policy_yaml,
        }
    }
}

#[tool_router(server_handler)]
impl TraxesMcpServer {
    #[tool(
        description = "Write a file through the TRAXES pre-execution boundary. TRAXES evaluates the exact FILE_WRITE action first; only an ALLOW decision can produce the action-bound authorization required for the write."
    )]
    fn write_file(&self, Parameters(params): Parameters<WriteFileParams>) -> String {
        let action = ProposedAction {
            tool: "FILE_WRITE".to_string(),
            session_id: format!("mcp-{}", Uuid::new_v4()),
            environment: "sandbox".to_string(),
            parameters: json!({
                "path": params.path,
                "content": params.content,
            }),
        };

        let decision_id = format!("dec_{}", Uuid::new_v4().simple());
        let (evaluation, permit) = self.engine.evaluate_with_permit(&action, &decision_id);

        // The MCP adapter never writes the file directly. Every governed side effect
        // flows through the same hardened execution boundary used by the existing proof.
        let execution_outcome = action::execute(&action, permit);
        let execution_status = match &execution_outcome {
            ExecutionOutcome::Executed => "executed",
            ExecutionOutcome::ExecutionFailed(_) => "failed",
            ExecutionOutcome::Unauthorized => "blocked",
        };
        let execution_outcome_text = format!("{:?}", execution_outcome);

        let artifact = ArtifactLogger::generate_artifact_with_outcome(
            &decision_id,
            &action,
            &evaluation,
            self.engine.policy_hash(),
            execution_status.to_string(),
            Some(execution_outcome_text.clone()),
        );

        match artifact.write_to_file_sync() {
            Ok(artifact_path) => json!({
                "decision": evaluation.decision,
                "decision_id": decision_id,
                "execution_status": execution_status,
                "execution_outcome": execution_outcome_text,
                "artifact_path": artifact_path,
            })
            .to_string(),
            Err(error) => json!({
                "decision": evaluation.decision,
                "decision_id": decision_id,
                "execution_status": execution_status,
                "execution_outcome": execution_outcome_text,
                "artifact_error": error.to_string(),
            })
            .to_string(),
        }
    }

    #[tool(
        description = "Replay and verify a TRAXES decision artifact using the same policy loaded by this MCP server."
    )]
    fn replay_decision(&self, Parameters(params): Parameters<ReplayParams>) -> String {
        let artifact_path = format!("artifacts/AuditArtifact_{}.json", params.decision_id);
        let replay_engine = ReplayEngine::new(Engine::with_policy(self.policy_yaml.clone()));

        match replay_engine.replay_from_file(&artifact_path) {
            Ok(result) => json!({
                "decision_id": result.decision_id,
                "original_decision": result.original_decision,
                "replay_decision": result.replay_decision,
                "match": result.match_status,
                "policy_hash": result.policy_hash,
            })
            .to_string(),
            Err(error) => json!({
                "decision_id": params.decision_id,
                "match": false,
                "error": error.to_string(),
            })
            .to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy_path = std::env::var("TRAXES_POLICY")
        .unwrap_or_else(|_| "policies/file_write_agent_policy.yaml".to_string());
    let policy_yaml = fs::read_to_string(&policy_path)?;

    // stdout belongs exclusively to MCP's stdio transport.
    eprintln!("TRAXES MCP server using policy: {policy_path}");

    let service = TraxesMcpServer::new(policy_yaml).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
