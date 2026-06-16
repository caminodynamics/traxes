use crate::evaluate::run_evaluate_with_emitter;
use crate::traxes_engine::Engine;

pub async fn run_async(args: &[String]) {
    let input = args.get(0).cloned().unwrap_or("allow".to_string());

    let path = match input.as_str() {
        "allow" => "../demo/payloads/allow_t3medium.json",
        "deny" => "../demo/payloads/deny_t3large.json",
        other => other,
    };

    let engine = Engine::load_default_policies().unwrap();
    let policy_hash = engine.policy_hash().to_string();
    let (event_emitter, event_rx) = crate::artifact_emitter::create_event_channel(100);

    let emitter = crate::artifact_emitter::ArtifactEmitter::new(event_rx, policy_hash);
    tokio::spawn(emitter.run());

    let result = run_evaluate_with_emitter(&path, event_emitter, &engine).await;

    match result {
        Ok(res) => {
            println!("→ DECISION: {}", res.decision);
        }
        Err(e) => {
            println!("ERROR: {:?}", e);
            std::process::exit(1);
        }
    }
}

pub async fn run_async_silent(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let input = args.get(0).cloned().unwrap_or("allow".to_string());

    let path = match input.as_str() {
        "allow" => "../demo/payloads/allow_t3medium.json",
        "deny" => "../demo/payloads/deny_t3large.json",
        other => other,
    };

    let engine = Engine::load_default_policies().unwrap();
    let policy_hash = engine.policy_hash().to_string();
    let (event_emitter, event_rx) = crate::artifact_emitter::create_event_channel(100);

    let emitter = crate::artifact_emitter::ArtifactEmitter::new(event_rx, policy_hash);
    tokio::spawn(emitter.run());

    // Suppress stdout during evaluation
    let _guard = crate::cli_utils::suppress_stdout();

    let result = run_evaluate_with_emitter(&path, event_emitter, &engine).await;

    match result {
        Ok(res) => Ok(res.decision),
        Err(e) => Err(e),
    }
}

#[allow(dead_code)]
pub fn run(args: &[String]) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(run_async(args));
}
