use rmcp::{
    model::CallToolRequestParams,
    object,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::Value;
use std::fs;
use std::path::Path;
use tokio::process::Command;

const ALLOW_PATH: &str = "temp_executed_agent_allowed.txt";
const DENY_PATH: &str = "temp_executed_agent_forbidden.txt";
const ALLOW_CONTENT: &str = "written through automated MCP smoke test";
const DENY_CONTENT: &str = "this should never be written";

fn tool_payload<T: serde::Serialize>(result: &T) -> Value {
    let wire = serde_json::to_value(result).expect("tool result must serialize");
    let text = wire
        .get("content")
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|entry| entry.get("text"))
        .and_then(Value::as_str)
        .expect("tool result must contain text content");

    serde_json::from_str(text).expect("tool text must contain JSON")
}

#[tokio::test]
async fn mcp_file_write_boundary_allow_deny_and_replay() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let allow_path = manifest_dir.join(ALLOW_PATH);
    let deny_path = manifest_dir.join(DENY_PATH);

    // Ensure a previous manual/demo run cannot produce a false positive.
    let _ = fs::remove_file(&allow_path);
    let _ = fs::remove_file(&deny_path);
    assert!(!allow_path.exists());
    assert!(!deny_path.exists());

    let server_binary = env!("CARGO_BIN_EXE_traxes-mcp");
    let transport = TokioChildProcess::new(Command::new(server_binary).configure(|cmd| {
        cmd.current_dir(manifest_dir)
            .env("TRAXES_POLICY", "policies/file_write_agent_policy.yaml");
    }))?;
    let client = ().serve(transport).await?;

    let tools = client.list_all_tools().await?;
    assert!(tools.iter().any(|tool| tool.name.as_ref() == "write_file"));
    assert!(tools
        .iter()
        .any(|tool| tool.name.as_ref() == "replay_decision"));

    let allow_result = client
        .call_tool(
            CallToolRequestParams::new("write_file").with_arguments(object!({
                "path": ALLOW_PATH,
                "content": ALLOW_CONTENT,
            })),
        )
        .await?;
    let allow = tool_payload(&allow_result);

    assert_eq!(allow["decision"], "ALLOW");
    assert_eq!(allow["execution_status"], "executed");
    assert_eq!(allow["execution_outcome"], "Executed");
    assert!(allow_path.exists(), "ALLOW must create the target file");
    assert_eq!(fs::read_to_string(&allow_path)?, ALLOW_CONTENT);

    let allow_decision_id = allow["decision_id"]
        .as_str()
        .expect("ALLOW result must include decision_id")
        .to_string();

    let allow_replay_result = client
        .call_tool(
            CallToolRequestParams::new("replay_decision").with_arguments(object!({
                "decision_id": allow_decision_id,
            })),
        )
        .await?;
    let allow_replay = tool_payload(&allow_replay_result);
    assert_eq!(allow_replay["match"], true);
    assert_eq!(allow_replay["original_decision"], "ALLOW");
    assert_eq!(allow_replay["replay_decision"], "ALLOW");

    let deny_result = client
        .call_tool(
            CallToolRequestParams::new("write_file").with_arguments(object!({
                "path": DENY_PATH,
                "content": DENY_CONTENT,
            })),
        )
        .await?;
    let deny = tool_payload(&deny_result);

    assert_eq!(deny["decision"], "DENY");
    assert_eq!(deny["execution_status"], "blocked");
    assert_eq!(deny["execution_outcome"], "Unauthorized");
    assert!(!deny_path.exists(), "DENY must not create the target file");

    let deny_decision_id = deny["decision_id"]
        .as_str()
        .expect("DENY result must include decision_id")
        .to_string();

    let deny_replay_result = client
        .call_tool(
            CallToolRequestParams::new("replay_decision").with_arguments(object!({
                "decision_id": deny_decision_id,
            })),
        )
        .await?;
    let deny_replay = tool_payload(&deny_replay_result);
    assert_eq!(deny_replay["match"], true);
    assert_eq!(deny_replay["original_decision"], "DENY");
    assert_eq!(deny_replay["replay_decision"], "DENY");

    client.cancel().await?;

    let _ = fs::remove_file(&allow_path);
    let _ = fs::remove_file(&deny_path);

    Ok(())
}
