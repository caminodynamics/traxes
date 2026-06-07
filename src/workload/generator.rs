use rand::{Rng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum Scenario {
    RogueInfraAgent,
    FinopsGuardrails,
    DangerousDbOps,
    CleanAllowAgent,
}

impl Scenario {
    pub fn from_str(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "finops-guardrails" => Scenario::FinopsGuardrails,
            "dangerous-db-ops" => Scenario::DangerousDbOps,
            "clean-allow-agent" => Scenario::CleanAllowAgent,
            _ => Scenario::RogueInfraAgent,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Scenario::RogueInfraAgent => "rogue-infra-agent",
            Scenario::FinopsGuardrails => "finops-guardrails",
            Scenario::DangerousDbOps => "dangerous-db-ops",
            Scenario::CleanAllowAgent => "clean-allow-agent",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: u64,
    pub tool: String,
    pub payload: Value,
}

pub struct WorkloadConfig {
    pub request_count: usize,
    pub seed: u64,
    pub scenario: Scenario,
}

pub struct WorkloadGenerator;

impl WorkloadGenerator {
    pub fn generate(config: &WorkloadConfig) -> Vec<ToolCall> {
        let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
        let total_requests = config.request_count as u64;

        (0..config.request_count)
            .map(|index| {
                let id = index as u64;
                let tool = Self::choose_tool(&config.scenario, &mut rng);
                let payload = Self::random_payload(id, &tool, config, total_requests, &mut rng);
                ToolCall { id, tool, payload }
            })
            .collect()
    }

    fn choose_tool(scenario: &Scenario, rng: &mut ChaCha8Rng) -> String {
        let sample = rng.gen_range(0..100);

        match scenario {
            Scenario::RogueInfraAgent => {
                if sample < 50 {
                    "deploy_instance"
                } else if sample < 75 {
                    "create_vm"
                } else if sample < 90 {
                    "scale_cluster"
                } else {
                    "delete_database"
                }
            }
            Scenario::FinopsGuardrails => {
                if sample < 40 {
                    "deploy_instance"
                } else if sample < 80 {
                    "create_vm"
                } else if sample < 95 {
                    "scale_cluster"
                } else {
                    "delete_database"
                }
            }
            Scenario::DangerousDbOps => {
                if sample < 35 {
                    "delete_database"
                } else if sample < 60 {
                    "inspect_database"
                } else if sample < 80 {
                    "modify_schema"
                } else if sample < 90 {
                    "scale_cluster"
                } else {
                    "deploy_instance"
                }
            }
            Scenario::CleanAllowAgent => {
                if sample < 40 {
                    "deploy_instance"
                } else if sample < 80 {
                    "create_vm"
                } else if sample < 95 {
                    "scale_cluster"
                } else if sample < 97 {
                    "inspect_database"
                } else {
                    "modify_schema"
                }
            }
        }
        .to_string()
    }

    fn random_payload(
        request_id: u64,
        tool: &str,
        config: &WorkloadConfig,
        total_requests: u64,
        rng: &mut ChaCha8Rng,
    ) -> Value {
        let scenario = &config.scenario;
        let cost = Self::random_cost(tool, scenario, request_id, total_requests, rng);
        let region = Self::random_region(tool, scenario, rng);
        let instance_type = Self::random_instance_type(rng);

        let mut payload = json!({
            "request_id": request_id,
            "resource_id": format!("res-{:016x}", rng.next_u64()),
            "tool": tool,
            "instance_type": instance_type,
            "region": region,
            "cost": cost,
            "description": format!("{} request", tool),
        });

        if tool == "delete_database" {
            let force_chance = match scenario {
                Scenario::RogueInfraAgent => 0.25,
                Scenario::FinopsGuardrails => 0.10,
                Scenario::DangerousDbOps => 0.45,
                _ => 0.0,
            };
            payload["force"] = Value::Bool(rng.gen_bool(force_chance));
        }

        if tool == "deploy_instance" || tool == "create_vm" {
            payload["provisioned"] = Value::Bool(rng.gen_bool(0.85));
        }

        if tool == "scale_cluster" {
            let max_scale = match scenario {
                Scenario::CleanAllowAgent => 4,
                _ => 8,
            };
            payload["scale_to"] = Value::Number((rng.gen_range(1..=max_scale)).into());
        }

        if tool == "inspect_database" {
            let queries = ["describe_tables", "check_replication", "list_users"];
            payload["query"] = Value::String(queries[rng.gen_range(0..queries.len())].to_string());
        }

        if tool == "modify_schema" {
            let changes = ["add_index", "drop_column", "alter_table"];
            payload["change_type"] =
                Value::String(changes[rng.gen_range(0..changes.len())].to_string());
        }

        payload
    }

    fn random_cost(
        tool: &str,
        scenario: &Scenario,
        request_id: u64,
        total_requests: u64,
        rng: &mut ChaCha8Rng,
    ) -> f64 {
        if tool == "delete_database" || tool == "inspect_database" || tool == "modify_schema" {
            return 0.0;
        }

        match scenario {
            Scenario::RogueInfraAgent => {
                if rng.gen_bool(0.20) {
                    (rng.gen_range(250..601) as f64) / 100.0
                } else {
                    (rng.gen_range(70..201) as f64) / 100.0
                }
            }
            Scenario::FinopsGuardrails => {
                let trend = 0.5 + (request_id as f64 / (total_requests.max(1) as f64)) * 4.5;
                let noise = (rng.gen_range(-10..=10) as f64) / 100.0;
                (trend + noise).clamp(0.5, 6.0)
            }
            Scenario::DangerousDbOps => (rng.gen_range(10..=200) as f64) / 100.0,
            Scenario::CleanAllowAgent => {
                if tool == "deploy_instance" || tool == "create_vm" {
                    (rng.gen_range(50..=200) as f64) / 100.0
                } else {
                    0.0
                }
            }
        }
    }

    fn random_region(tool: &str, scenario: &Scenario, rng: &mut ChaCha8Rng) -> &'static str {
        let allowed = ["us-east-1", "us-west-2"];
        let forbidden = ["eu-central-1", "ap-southeast-2"];

        if tool == "modify_schema" || tool == "inspect_database" {
            return "us-east-1";
        }

        match scenario {
            Scenario::RogueInfraAgent => {
                if rng.gen_bool(0.8) {
                    allowed[rng.gen_range(0..allowed.len())]
                } else {
                    forbidden[rng.gen_range(0..forbidden.len())]
                }
            }
            Scenario::FinopsGuardrails => {
                if rng.gen_bool(0.9) {
                    allowed[rng.gen_range(0..allowed.len())]
                } else {
                    forbidden[rng.gen_range(0..forbidden.len())]
                }
            }
            Scenario::DangerousDbOps => {
                if rng.gen_bool(0.5) {
                    allowed[rng.gen_range(0..allowed.len())]
                } else {
                    forbidden[rng.gen_range(0..forbidden.len())]
                }
            }
            Scenario::CleanAllowAgent => allowed[rng.gen_range(0..allowed.len())],
        }
    }

    fn random_instance_type(rng: &mut ChaCha8Rng) -> &'static str {
        let types = ["t3.micro", "t3.small", "t3.medium", "m5.large", "c6i.large"];
        types[rng.gen_range(0..types.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PolicyEngine;

    #[test]
    fn scenario_from_str_parses_known_values_and_defaults() {
        assert_eq!(
            Scenario::from_str("rogue-infra-agent"),
            Scenario::RogueInfraAgent
        );
        assert_eq!(
            Scenario::from_str("finops-guardrails"),
            Scenario::FinopsGuardrails
        );
        assert_eq!(
            Scenario::from_str("dangerous-db-ops"),
            Scenario::DangerousDbOps
        );
        assert_eq!(
            Scenario::from_str("clean-allow-agent"),
            Scenario::CleanAllowAgent
        );
        assert_eq!(
            Scenario::from_str("unknown-scenario"),
            Scenario::RogueInfraAgent
        );
    }

    #[test]
    fn clean_allow_agent_generates_only_allowing_payloads() {
        let config = WorkloadConfig {
            request_count: 100,
            seed: 42,
            scenario: Scenario::CleanAllowAgent,
        };

        let workload = WorkloadGenerator::generate(&config);
        let engine = PolicyEngine::default();

        for call in &workload {
            let result = engine.evaluate(call);
            assert!(
                matches!(result, Ok(crate::engine::EvaluationResponse::Allow)),
                "payload {:?} was denied",
                call
            );
        }
    }

    #[test]
    fn deterministic_workload_is_reproducible_for_fixed_seed_and_scenario() {
        let config = WorkloadConfig {
            request_count: 1000,
            seed: 42,
            scenario: Scenario::FinopsGuardrails,
        };

        let first = WorkloadGenerator::generate(&config);
        let second = WorkloadGenerator::generate(&config);

        assert_eq!(
            first, second,
            "workloads must be identical for the same seed and scenario"
        );
        assert_eq!(first.len(), second.len());

        let engine = PolicyEngine::default();
        let first_allowed = first
            .iter()
            .filter(|call| {
                matches!(
                    engine.evaluate(call),
                    Ok(crate::engine::EvaluationResponse::Allow)
                )
            })
            .count();
        let second_allowed = second
            .iter()
            .filter(|call| {
                matches!(
                    engine.evaluate(call),
                    Ok(crate::engine::EvaluationResponse::Allow)
                )
            })
            .count();
        assert_eq!(first_allowed, second_allowed);
        assert_eq!(
            first_allowed + (first.len() - first_allowed),
            config.request_count
        );
    }
}
