# Traxes

Traxes is a deterministic execution engine that evaluates proposed actions against versioned policies before execution and produces replayable decision artifacts.

## The Problem

Autonomous systems and AI agents execute real actions in production environments. When an action is allowed or denied, many systems can record what happened, but cannot reproducibly explain why the decision was made. Pre-action validation logic is typically:

- Embedded in application code
- Duplicated across services
- Scattered across infrastructure layers

As a result, decisions are difficult to reproduce after the fact, and audit trails are incomplete.

## Core Model

Traxes solves this by evaluating actions before execution and producing replayable decision artifacts.

1. A system proposes an action.
2. Traxes evaluates the action against a versioned policy.
3. Traxes returns ALLOW or DENY.
4. Traxes writes a deterministic audit artifact capturing the decision.

### Execution Flow

```
[Planner / Agent / Service]
          ↓
       Traxes
 (ALLOW / DENY + artifact)
          ↓
     Execution Layer
```

Traxes sits between decision-making systems and execution systems, providing a deterministic control point before actions occur.

Traxes does not execute actions. It evaluates them and produces a deterministic decision artifact that downstream systems consume.

## AI Agent Example

An AI agent proposes provisioning cloud infrastructure:

```json
{
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "parameters": {
    "instance_type": "m5.large",
    "instance_cost_per_hour": 0.52
  }
}
```

Traxes evaluates the proposed action against the versioned policy:

```text
instance_type: m5.large
policy_version: aws_staging_guardrails@v3
decision: DENY
reason: instance_type not permitted for staging environment
artifact: /artifacts/91bc.json
```

The artifact contains the complete decision context: the proposed action, the policy version, the evaluation result, and the reasoning. This artifact is replayable—given the same action and policy version, Traxes produces the same decision.

Without Traxes:
Decision logic may be distributed across application code, infrastructure policies, orchestration systems, and operational procedures.

With Traxes:
The decision becomes a single deterministic evaluation with a replayable artifact.

## Operational Guarantees

Traxes provides three operational properties:

**Auditability**: Every decision produces a deterministic artifact that captures the complete evaluation context. You can replay any decision to verify why an action was allowed or denied.

**Deterministic Evaluation**: Given the same action and policy version, Traxes produces the same decision. This reproducibility is enforced through behavioral invariants.

**Reproducibility**: Artifact structure is stable across runs for a given policy version. This enables post-hoc analysis, debugging, and compliance verification.

## Invariants

Traxes enforces a fixed set of behavioral invariants across all evaluations:

- Given the same action and policy version, Traxes produces the same decision.
- Every decision produces a deterministic, replayable audit artifact.
- Policy evaluation is isolated from execution and has no side effects.
- All malformed inputs resolve to explicit DENY or controlled failure states.
- Artifact structure is stable across runs for a given policy version.

These invariants are intended to make system behavior reproducible under replay.

## Integration

Traxes operates at the boundary between decision-making and execution. It is typically embedded or deployed as a pre-execution gate in existing systems:

- AI agent tool execution pipelines
- Infrastructure provisioning workflows
- Backend service action validation
- Robotics and device command execution layers

## Example Decisions

**Decision Artifact (runtime output)**

```json
{
  "artifact_version": "1.0.0",
  "artifact_type": "pre_execution_decision",
  "decision_id": "dec_84b52d8256e3414f914f3047f2dd2cd0",
  "timestamp": "2026-06-29T18:04:51.924457800+00:00",
  "decision": "ALLOW",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "policy_bundle": "infra-cost-limit-v1",
  "policy_hash": "785e022b9921ee6a43ab5044cc8e4a67930c936a1a521f4479269e47eacc52e8",
  "sha256_hash": "e3665d8564c753be7334b9e77c6585d2d4bbd5d74255619111d385b880bd62f0",
  "engine": {
    "name": "Traxes",
    "engine_version": "0.3.2",
    "policy_bundle_id": "infra-cost-limit-v1"
  },
  "proposed_action": {
    "tool": "AWS_RDS_PROVISION",
    "environment": "staging",
    "parameters": {
      "instance_type": "t3.micro",
      "instance_cost_per_hour": 0.015
    }
  },
  "rule_evaluation": {
    "rule_id": "infra-cost-limit",
    "field": "proposed_action.parameters.instance_type",
    "observed_value": "t3.micro",
    "operator": "infra-cost-limit",
    "evaluation_expression": "proposed_action.parameters.instance_type not_in policy",
    "evaluation_result": false
  },
  "performance": {
    "evaluation_latency_us": 230.0,
    "decision_latency_us": 4.8,
    "artifact_write_latency_us": 41.7
  },
  "side_effect_prevention": {
    "decision_effect": "ALLOW"
  },
  "execution_status": "executed"
}
```

Artifacts are written to `artifacts/AuditArtifact_{decision_id}.json`.

## Why Traxes

Traxes is designed for systems where actions must be:

- Evaluated before execution
- Reproducible under replay
- Auditable after the fact
- Deterministic across runs
- Independent from execution side effects

Examples include AI agents, infrastructure automation, robotics, and safety-critical systems.

## Quick Start

### Windows (Prebuilt Release Binary)

```powershell
# Run the interactive demo (no arguments required)
.\traxes-demo.exe demo

# Run performance benchmarks
.\traxes-demo.exe benchmark
```

### Linux/macOS (Source Build)

```bash
# Build from source
cargo build --release

# Run the interactive demo (no arguments required)
./target/release/traxes-demo demo

# Run performance benchmarks
./target/release/traxes-demo benchmark
```

The demo runs embedded ALLOW and DENY examples with no external dependencies.

The prebuilt release binary contains embedded demo assets and benchmark payloads. The `eval` command operates on user-supplied CLI demo inputs and therefore requires either a repository checkout or your own input file.

## CLI Demo Evaluation Inputs

Example CLI demo inputs are available after cloning the repository:

```bash
git clone https://github.com/caminodynamics/traxes
cd traxes
cargo build --release
./target/release/traxes-demo eval payloads/allow_db.json
```

The `payloads/` directory contains example CLI demo inputs used to demonstrate ALLOW and DENY outcomes.

## Requirements

- Prebuilt binaries available in GitHub Releases
- Rust toolchain for source builds
- Optional Docker support for reproducible CLI demo execution

## Links

- Releases: https://github.com/caminodynamics/traxes/releases/tag/v0.1.1
- Issues: https://github.com/caminodynamics/traxes/issues
- Demo video: https://github.com/caminodynamics/traxes/releases/tag/demo_video

## Next step

Run the demo, inspect the decision artifact, then modify the Rust policy definitions to evaluate your own action schema.

---
