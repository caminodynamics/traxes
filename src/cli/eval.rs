use crate::evaluate::run_evaluate_with_emitter;
use crate::traxes_engine::Engine;
use traxes_demo::assets;

pub async fn run_async(args: &[String]) {
    let input = args.get(0).cloned().unwrap_or("allow".to_string());

    let path = match input.as_str() {
        "allow" => {
            // Write embedded payload to temp file for evaluation
            let temp_path = std::env::current_dir()
                .expect("Failed to get current directory")
                .join("temp_embedded_allow.json")
                .to_string_lossy()
                .to_string();
            std::fs::write(&temp_path, assets::ALLOW_PAYLOAD).expect("Failed to write embedded allow payload");
            temp_path
        }
        "deny" => {
            // Write embedded payload to temp file for evaluation
            let temp_path = std::env::current_dir()
                .expect("Failed to get current directory")
                .join("temp_embedded_deny.json")
                .to_string_lossy()
                .to_string();
            std::fs::write(&temp_path, assets::DENY_PAYLOAD).expect("Failed to write embedded deny payload");
            temp_path
        }
        other => other.to_string(),
    };

    let engine = match Engine::load_default_policies() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("ERROR: Failed to load policy bundle: {}", e);
            std::process::exit(1);
        }
    };
    let policy_hash = engine.policy_hash().to_string();
    let (event_emitter, event_receivers, event_counter) = crate::artifact_emitter::create_sharded_event_channels(2, 50);

    for event_rx in event_receivers {
        let emitter = crate::artifact_emitter::ArtifactEmitter::new(event_rx, policy_hash.clone(), event_counter.clone());
        tokio::spawn(async move {
            emitter.run().await;
        });
    }

    let result = run_evaluate_with_emitter(path.to_string(), event_emitter, &engine).await;

    // Clean up temp files if they were created
    if input == "allow" {
        let temp_path = std::env::current_dir()
            .expect("Failed to get current directory")
            .join("temp_embedded_allow.json");
        let _ = std::fs::remove_file(&temp_path);
    } else if input == "deny" {
        let temp_path = std::env::current_dir()
            .expect("Failed to get current directory")
            .join("temp_embedded_deny.json");
        let _ = std::fs::remove_file(&temp_path);
    }

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
        "allow" => {
            // Write embedded payload to temp file for evaluation
            let temp_path = std::env::current_dir()
                .expect("Failed to get current directory")
                .join("temp_embedded_allow.json")
                .to_string_lossy()
                .to_string();
            std::fs::write(&temp_path, assets::ALLOW_PAYLOAD)?;
            temp_path
        }
        "deny" => {
            // Write embedded payload to temp file for evaluation
            let temp_path = std::env::current_dir()
                .expect("Failed to get current directory")
                .join("temp_embedded_deny.json")
                .to_string_lossy()
                .to_string();
            std::fs::write(&temp_path, assets::DENY_PAYLOAD)?;
            temp_path
        }
        other => other.to_string(),
    };

    let engine = Engine::load_default_policies()?;
    let policy_hash = engine.policy_hash().to_string();
    let (event_emitter, event_receivers, event_counter) = crate::artifact_emitter::create_sharded_event_channels(2, 50);

    for event_rx in event_receivers {
        let emitter = crate::artifact_emitter::ArtifactEmitter::new(event_rx, policy_hash.clone(), event_counter.clone());
        tokio::spawn(async move {
            emitter.run().await;
        });
    }

    // Suppress stdout during evaluation
    let _guard = crate::cli_utils::suppress_stdout();

    let result = run_evaluate_with_emitter(path.to_string(), event_emitter, &engine).await;

    // Clean up temp files if they were created
    if input == "allow" {
        let temp_path = std::env::current_dir()
            .expect("Failed to get current directory")
            .join("temp_embedded_allow.json");
        let _ = std::fs::remove_file(&temp_path);
    } else if input == "deny" {
        let temp_path = std::env::current_dir()
            .expect("Failed to get current directory")
            .join("temp_embedded_deny.json");
        let _ = std::fs::remove_file(&temp_path);
    }

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
