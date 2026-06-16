use crate::action::ProposedAction;
use crate::server_policy::EvaluationResult;
use colored::Colorize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static DEMO_MODE: AtomicBool = AtomicBool::new(false);
static DEBUG_MODE: AtomicBool = AtomicBool::new(false);
static DEMO_FAST: AtomicBool = AtomicBool::new(false);
static SUPPRESS_OUTPUT: Mutex<bool> = Mutex::new(false);

const POLICY_BUNDLE: &str = "infra-cost-limit-v1";

pub fn set_demo_mode(enabled: bool) {
    DEMO_MODE.store(enabled, Ordering::Relaxed);
    if enabled {
        DEBUG_MODE.store(false, Ordering::Relaxed);
    }
}

pub fn set_debug_mode(enabled: bool) {
    DEBUG_MODE.store(enabled, Ordering::Relaxed);
}

pub fn set_demo_fast(enabled: bool) {
    DEMO_FAST.store(enabled, Ordering::Relaxed);
}

pub fn is_demo_mode() -> bool {
    DEMO_MODE.load(Ordering::Relaxed)
}

pub fn is_debug_mode() -> bool {
    DEBUG_MODE.load(Ordering::Relaxed)
}

pub fn is_demo_fast() -> bool {
    DEMO_FAST.load(Ordering::Relaxed)
}

/// Log policy/debug messages only outside demo mode (or when --debug is set).
pub fn debug_log(message: impl std::fmt::Display) {
    if !is_demo_mode() || is_debug_mode() {
        eprintln!("{}", message);
    }
}

pub struct StdoutGuard;

impl StdoutGuard {
    pub fn new() -> Self {
        *SUPPRESS_OUTPUT.lock().unwrap() = true;
        StdoutGuard
    }
}

impl Drop for StdoutGuard {
    fn drop(&mut self) {
        *SUPPRESS_OUTPUT.lock().unwrap() = false;
    }
}

pub fn suppress_stdout() -> StdoutGuard {
    StdoutGuard::new()
}

pub fn is_output_suppressed() -> bool {
    *SUPPRESS_OUTPUT.lock().unwrap()
}

pub struct RequestOutput<'a> {
    pub action: &'a ProposedAction,
    pub result: &'a EvaluationResult,
    pub evaluation_latency_us: f64,
    pub decision_latency_us: f64,
    pub artifact_write_latency_us: f64,
    pub artifact_path: &'a str,
    pub decision_id: &'a str,
    pub policy_hash: &'a str,
    pub show_run_separator: bool,
}


pub fn parse_demo_flags(args: Vec<String>) -> (bool, bool, Vec<String>) {
    let mut demo_mode = false;
    let mut debug_mode = false;
    let filtered: Vec<String> = args
        .into_iter()
        .filter(|arg| {
            match arg.as_str() {
                "--demo-mode" | "--demo-mode=true" => {
                    demo_mode = true;
                    false
                }
                "--demo-mode=false" => {
                    demo_mode = false;
                    false
                }
                "--debug" => {
                    debug_mode = true;
                    false
                }
                _ => true,
            }
        })
        .collect();
    (demo_mode, debug_mode, filtered)
}

pub fn print_run_separator() {
    if !is_demo_mode() {
        return;
    }
    // No separator in demo mode - output is self-terminating
}

pub fn print_request_output(out: RequestOutput<'_>) {
    if is_demo_mode() {
        print_request_demo(&out);
    } else {
        print_request_legacy(&out);
        if out.show_run_separator {
            print_run_separator();
        }
    }
}


fn print_request_legacy(out: &RequestOutput<'_>) {
    if is_output_suppressed() {
        return;
    }
    legacy_print_received_action(out.action);
    legacy_print_evaluation(out.action, out.result);
    legacy_print_performance(
        out.evaluation_latency_us,
        out.decision_latency_us,
        out.artifact_write_latency_us,
        out.artifact_path,
        out.decision_id,
    );
    legacy_print_hashes(out.policy_hash, out.decision_id);
}

fn print_request_demo(out: &RequestOutput<'_>) {
    if is_output_suppressed() {
        return;
    }
    let (decision, reason) = normalize_decision(out.result);
    let reason_line = one_line_reason(&reason, out.result);

    // SYSTEM HEADER
    println!("╭──────────────────────────────╮");
    println!("│  TRAXES POLICY ENGINE        │");
    println!("│  deterministic evaluation    │");
    println!("╰──────────────────────────────╮");
    println!();

    // INPUT SECTION
    println!("INPUT");
    println!("  tool: {}", out.action.tool);
    println!("  environment: {}", out.action.environment);
    let instance_type = out.action.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    println!("  instance_type: {}", instance_type);
    println!();

    // EVALUATION SECTION
    println!("EVALUATION");
    println!("  evaluation_latency_us: {:.1}", out.evaluation_latency_us);
    println!("  decision_latency_us: {:.1}", out.decision_latency_us);
    println!();

    // DECISION SECTION (most important)
    println!("DECISION → {}", colorize_decision(decision));
    if decision == "DENY" {
        println!("REASON → {}", reason_line);
    }
    println!();

    // CLI delay for artifact reveal
    std::thread::sleep(std::time::Duration::from_millis(600));

    // ARTIFACT SECTION
    println!("ARTIFACT");
    println!("  path: {}", out.artifact_path.dimmed());
    println!("  status: written");
    println!();

    // EXIT SECTION
    println!("EXIT");
    println!("  code: 0");
    println!("  decision: {}", decision);
}


fn colorize_decision(decision: &str) -> String {
    if decision == "ALLOW" {
        decision.green().to_string()
    } else {
        decision.red().to_string()
    }
}


fn legacy_print_received_action(action: &ProposedAction) {
    println!("[Traxes] received proposed action");
    println!("  {}              {}", "tool".dimmed(), action.tool);
    println!("  {}        {}", "session_id".dimmed(), action.session_id);
    println!(
        "  {}           {}",
        "environment".dimmed(),
        action.environment
    );
    let instance_type = action.parameters.get("instance_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let cost_per_hour = action.parameters.get("instance_cost_per_hour").and_then(|v| v.as_f64()).unwrap_or(0.0);
    println!(
        "  {}      {}",
        "instance_type".dimmed(),
        instance_type
    );
    println!(
        "  {}  {}",
        "cost_per_hour".dimmed(),
        cost_per_hour
    );
    println!(
        "  {}             {}",
        "policy_bundle".dimmed(),
        POLICY_BUNDLE
    );
    println!();
}

fn legacy_print_evaluation(_action: &ProposedAction, result: &EvaluationResult) {
    println!("[Traxes] {} policy", "evaluating".yellow());
    println!("  {}             {}", "field".dimmed(), result.field);
    println!("  {}              {}", "rule".dimmed(), result.rule);
    println!();
    if is_debug_mode() {
        println!(
            "  {}        {}",
            "evaluation".dimmed(),
            result.evaluation_expression
        );
    } else {
        println!("  {}", format_compact_rule(result).dimmed());
    }
    println!();
    let (decision, reason) = normalize_decision(result);
    println!("  {}            {}", "result".dimmed(), decision);
    println!(
        "  {}          {}",
        "decision".dimmed(),
        colorize_decision(decision)
    );
    if decision == "DENY" {
        println!(
            "  {}           {}",
            "reason".dimmed(),
            one_line_reason(&reason, result).red()
        );
    }
    println!();
}

fn legacy_print_performance(
    evaluation_latency_us: f64,
    decision_latency_us: f64,
    artifact_write_latency_us: f64,
    artifact_path: &str,
    decision_id: &str,
) {
    println!("[Traxes] {}", "performance".dimmed());
    println!(
        "  {}        {:.1} us",
        "evaluation".dimmed(),
        evaluation_latency_us
    );
    println!(
        "  {}          {:.1} us",
        "decision".dimmed(),
        decision_latency_us
    );
    println!(
        "  {}    {:.1} us",
        "artifact_write".dimmed(),
        artifact_write_latency_us
    );
    println!();
    println!("[Traxes] {}", "artifact written".dimmed());
    println!("  {}              {}", "path".dimmed(), artifact_path);
    println!("  {}       {}", "decision_id".dimmed(), decision_id);
    println!();
}

fn legacy_print_hashes(policy_hash: &str, artifact_id: &str) {
    println!("[Traxes] {}", "policy_hash".dimmed());
    println!("  {}", shorten_hash(policy_hash));
    println!("[Traxes] {}", "artifact_id".dimmed());
    println!("  {}", artifact_id);
    println!();
}

pub fn normalize_decision(result: &EvaluationResult) -> (&'static str, String) {
    match result.action.as_deref() {
        None => ("ALLOW", String::new()),
        Some("ALLOW") => ("ALLOW", String::new()),
        Some("DENY") => ("DENY", reason_from_result(result)),
        Some(msg) if msg.starts_with("DENY:") => {
            let detail = msg.strip_prefix("DENY:").unwrap_or(msg).trim();
            ("DENY", detail.to_string())
        }
        Some(msg) if msg.contains("DENY") => ("DENY", msg.to_string()),
        Some(_) => ("ALLOW", String::new()),
    }
}

fn reason_from_result(result: &EvaluationResult) -> String {
    human_reason_code(&result.reason)
}

pub fn one_line_reason(explicit: &str, result: &EvaluationResult) -> String {
    if !explicit.is_empty() {
        let first = explicit.lines().next().unwrap_or(explicit);
        if first.len() <= 120 {
            return first.to_string();
        }
        return format!("{}…", &first[..117]);
    }
    human_reason_code(&result.reason)
}

pub fn human_reason_code(code: &str) -> String {
    match code {
        "INSTANCE_TYPE_CONSTRAINT_CHECK" => {
            "instance_type is not allowed for this environment".to_string()
        }
        "INFRA_COST_LIMIT_CHECK" => "hourly cost exceeds policy threshold".to_string(),
        "POLICY_READ_ERROR" => "policy bundle could not be loaded".to_string(),
        "POLICY_MISSING_THRESHOLD" => "policy threshold missing; denied by default".to_string(),
        other => other.replace('_', " ").to_lowercase(),
    }
}

pub fn format_compact_rule(result: &EvaluationResult) -> String {
    if result.evaluation_expression.contains("not_in")
        || result.rule == "not_in"
        || result.evaluation_expression.contains("not in")
    {
        return "rule: instance_type ∉ allowed_instance_types".to_string();
    }
    if result.rule == "in_list" || result.evaluation_expression.contains(" in ") {
        return "rule: instance_type ∈ denied_instance_types".to_string();
    }
    if result.rule == "numeric_lte" {
        return format!(
            "rule: {} ≤ {:.2}/hr",
            result.field, result.policy_value
        );
    }
    format!("rule: {}", result.rule)
}

pub fn compact_evaluation_expression(field: &str, op: &str) -> String {
    match op {
        "not_in" if field == "instance_type" => {
            "instance_type ∉ allowed_instance_types".to_string()
        }
        "in_list" if field == "instance_type" => {
            "instance_type ∈ denied_instance_types".to_string()
        }
        "numeric_lte" => format!("{} ≤ policy_threshold", field),
        _ => format!("{} {} policy", field, op),
    }
}

fn shorten_hash(hash: &str) -> String {
    if hash.len() <= 12 {
        hash.to_string()
    } else {
        hash[..12].to_string()
    }
}
