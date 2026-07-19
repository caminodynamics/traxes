use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
struct Warning {
    category: String,
    severity: String,
    file: String,
    message: String,
}

#[derive(Debug)]
struct AuditResult {
    warnings: Vec<Warning>,
}

fn main() {
    println!("[ARCHITECTURE AUDIT - PHASE 2]");
    
    let result = run_audit();
    
    if result.warnings.is_empty() {
        println!("✓ No architectural warnings detected");
    } else {
        println!("⚠ Found {} architectural warning(s):", result.warnings.len());
        println!();
        
        for warning in &result.warnings {
            println!("  [{}] {} - {}", warning.severity, warning.category, warning.message);
            println!("    File: {}", warning.file);
            println!();
        }
    }
    
    // IMPORTANT: Do not fail build, only report
    std::process::exit(0);
}

fn run_audit() -> AuditResult {
    let src_dir = Path::new("src");
    
    let mut warnings = Vec::new();
    
    warnings.extend(check_duplicate_artifact_builders(src_dir));
    warnings.extend(check_decision_logic_leakage(src_dir));
    warnings.extend(check_fallback_divergence_risk(src_dir));
    warnings.extend(check_async_sync_mismatch(src_dir));
    
    AuditResult { warnings }
}

fn check_duplicate_artifact_builders(src_dir: &Path) -> Vec<Warning> {
    let mut warnings = Vec::new();
    
    // Check for multiple artifact construction methods
    // Canonical builder: AuditArtifact::from_evaluation()
    // Other builders should not exist outside of from_evaluation implementation
    
    let artifact_rs = src_dir.join("artifact.rs");
    let artifact_emitter_rs = src_dir.join("artifact_emitter.rs");
    
    if !artifact_rs.exists() || !artifact_emitter_rs.exists() {
        return warnings; // Can't check if files don't exist
    }
    
    let _artifact_content = fs::read_to_string(&artifact_rs).unwrap_or_default();
    let emitter_content = fs::read_to_string(&artifact_emitter_rs).unwrap_or_default();
    
    // Check if emitter uses canonical builder
    let emitter_uses_canonical = emitter_content.contains("from_evaluation");
    
    // Check if emitter has custom artifact construction (struct literal)
    let emitter_has_custom = emitter_content.contains("AuditArtifact {") && !emitter_uses_canonical;
    
    // If emitter has custom construction without using canonical builder, flag it
    if emitter_has_custom {
        warnings.push(Warning {
            category: "ARTIFACT_CONSTRUCTION".to_string(),
            severity: "HIGH".to_string(),
            file: "src/artifact_emitter.rs".to_string(),
            message: "Custom artifact construction detected. Should use canonical builder AuditArtifact::from_evaluation()".to_string(),
        });
    }
    
    warnings
}

fn check_decision_logic_leakage(src_dir: &Path) -> Vec<Warning> {
    let mut warnings = Vec::new();
    
    // Check for decision logic outside engine
    // Engine files: traxes_engine.rs, server_policy.rs, policy_bundle.rs
    // Forbidden decision logic in: CLI, tests, hooks, adapters
    
    let _engine_files = vec![
        "traxes_engine.rs",
        "server_policy.rs",
        "policy_bundle.rs",
    ];
    
    let non_engine_files = vec![
        "main.rs",
        "cli",
        // "evaluate.rs", // Allowed - evaluation orchestration, not decision logic
        // "action.rs", // Allowed - execution gate
        // "artifact_emitter.rs", // Allowed - artifact generation
        "artifact.rs",
    ];
    
    // Check non-engine files for decision logic patterns
    for file_pattern in non_engine_files {
        if file_pattern.contains("cli") {
            // Check CLI directory
            let cli_dir = src_dir.join("cli");
            if cli_dir.exists() {
                for entry in fs::read_dir(&cli_dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                        let content = fs::read_to_string(&path).unwrap_or_default();
                        if let Some(warning) = has_decision_logic(&content, &path) {
                            warnings.push(warning);
                        }
                    }
                }
            }
        } else {
            let file_path = src_dir.join(file_pattern);
            if file_path.exists() {
                let content = fs::read_to_string(&file_path).unwrap_or_default();
                if let Some(warning) = has_decision_logic(&content, &file_path) {
                    warnings.push(warning);
                }
            }
        }
    }
    
    warnings
}

fn has_decision_logic(content: &str, file_path: &Path) -> Option<Warning> {
    // Look for decision-making patterns outside engine
    // Allow: display formatting, test assertions, adapter forwarding
    // Forbidden: actual decision logic (if decision == "ALLOW" then execute logic that decides)
    
    let filename = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    
    // Skip test files (they have assertions, not decision logic)
    if filename.contains("test") || filename.contains("integration") {
        return None;
    }
    
    // Skip allowed files
    if filename == "action.rs" {
        return None; // Allowed - execution gate
    }
    if filename == "artifact_emitter.rs" {
        return None; // Allowed - artifact generation
    }
    if filename == "artifact.rs" {
        return None; // Allowed - artifact construction
    }
    if filename == "evaluate.rs" {
        return None; // Allowed - evaluation orchestration
    }
    if filename == "cli_utils.rs" {
        return None; // Allowed - display formatting
    }
    if filename.contains("demo.rs") {
        return None; // Allowed - display formatting
    }
    if filename.contains("eval.rs") {
        return None; // Allowed - CLI evaluation wrapper
    }
    
    // Check for suspicious patterns
    let suspicious_patterns = vec![
        ("if decision == \"ALLOW\"", "decision-based conditional"),
        ("if decision == \"DENY\"", "decision-based conditional"),
        ("match decision {", "decision-based match"),
        ("decision = \"ALLOW\"", "decision assignment"),
        ("decision = \"DENY\"", "decision assignment"),
    ];
    
    for (pattern, description) in suspicious_patterns {
        if content.contains(pattern) {
            // Check if it's in a display/formatting context (allowed)
            let context = get_line_context(content, pattern);
            if is_display_context(&context) {
                continue; // Allowed
            }
            // If not in display context and not in allowed file, flag it
            return Some(Warning {
                category: "DECISION_LOGIC_LEAKAGE".to_string(),
                severity: "HIGH".to_string(),
                file: file_path.to_string_lossy().to_string(),
                message: format!("Found {} outside engine. Decision logic should only exist in src/engine/", description),
            });
        }
    }
    
    None
}

fn get_line_context(content: &str, pattern: &str) -> String {
    // Get the line containing the pattern
    content.lines()
        .find(|line| line.contains(pattern))
        .unwrap_or("")
        .to_string()
}

fn is_display_context(line: &str) -> bool {
    // Check if line is display/formatting related
    let display_keywords = vec![
        "println!",
        "print!",
        "format!",
        "log::",
        "debug_log",
        "info!",
        "warn!",
        "error!",
        "color",
        "green",
        "red",
    ];
    
    display_keywords.iter().any(|keyword| line.contains(keyword))
}

fn check_fallback_divergence_risk(src_dir: &Path) -> Vec<Warning> {
    let mut warnings = Vec::new();
    
    // Check if fallback uses same artifact builder as async path
    
    let main_rs = src_dir.join("main.rs");
    let evaluate_rs = src_dir.join("evaluate.rs");
    
    if !main_rs.exists() || !evaluate_rs.exists() {
        return warnings; // Can't determine
    }
    
    let main_content = fs::read_to_string(&main_rs).unwrap_or_default();
    let evaluate_content = fs::read_to_string(&evaluate_rs).unwrap_or_default();
    
    // Check if fallback exists
    let has_fallback = main_content.contains("fallback") || evaluate_content.contains("fallback");
    
    if !has_fallback {
        return warnings; // No fallback, no divergence risk
    }
    
    // Check if fallback uses canonical builder
    let main_uses_canonical = main_content.contains("ArtifactLogger::generate_artifact");
    let evaluate_uses_canonical = evaluate_content.contains("ArtifactLogger::generate_artifact");
    
    if !main_uses_canonical {
        warnings.push(Warning {
            category: "FALLBACK_DIVERGENCE".to_string(),
            severity: "HIGH".to_string(),
            file: "src/main.rs".to_string(),
            message: "Fallback path does not use canonical artifact builder (ArtifactLogger::generate_artifact)".to_string(),
        });
    }
    
    if !evaluate_uses_canonical {
        warnings.push(Warning {
            category: "FALLBACK_DIVERGENCE".to_string(),
            severity: "HIGH".to_string(),
            file: "src/evaluate.rs".to_string(),
            message: "Fallback path does not use canonical artifact builder (ArtifactLogger::generate_artifact)".to_string(),
        });
    }
    
    warnings
}

fn check_async_sync_mismatch(src_dir: &Path) -> Vec<Warning> {
    let mut warnings = Vec::new();
    
    // Check if async and sync paths produce identical artifacts
    
    let artifact_emitter_rs = src_dir.join("artifact_emitter.rs");
    
    if !artifact_emitter_rs.exists() {
        return warnings; // Can't determine
    }
    
    let emitter_content = fs::read_to_string(&artifact_emitter_rs).unwrap_or_default();
    
    // Check if async path uses canonical builder
    let uses_canonical = emitter_content.contains("from_evaluation");
    
    if !uses_canonical {
        warnings.push(Warning {
            category: "ASYNC_SYNC_DIVERGENCE".to_string(),
            severity: "HIGH".to_string(),
            file: "src/artifact_emitter.rs".to_string(),
            message: "Async path does not use canonical artifact builder (from_evaluation). Risk of artifact divergence between async and sync paths".to_string(),
        });
    }
    
    warnings
}
