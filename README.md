# Traxes

TRAXES is a deterministic pre-execution control layer that evaluates proposed actions against versioned policies and produces replayable decision evidence.

TRAXES validates proposed actions before execution by evaluating them against versioned policies. Use cases include:

- AI agent tool execution
- Infrastructure automation
- Robotics/device command validation
- Compliance/governance workflows

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
4. Traxes writes a decision artifact capturing the evaluation context and evidence.

### Execution Flow

```
┌─────────────────────────────────┐
│ Planner / Agent / Service       │
└────────────┬────────────────────┘
             │
             ↓
┌─────────────────────────────────┐
│ TRAXES                           │
│ • Evaluate action against policy │
│ • Return ALLOW/DENY             │
│ • Write decision artifact        │
│ • Support replay verification   │
└────────────┬────────────────────┘
             │
             ↓
┌─────────────────────────────────┐
│ Execution Layer                 │
│ (if ALLOW)                      │
└─────────────────────────────────┘
```

TRAXES sits between decision-making systems and execution systems, providing a deterministic control point before actions occur.

TRAXES does not execute actions. It evaluates them and produces a decision artifact that downstream systems consume.

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

The artifact contains the complete decision context: the proposed action, the policy version, the evaluation result, and the decision context and rule evaluation evidence. This artifact is replayable—given the same action and policy version, Traxes produces the same decision.

Traxes does not determine agent intent or generate plans. It validates whether a proposed action satisfies defined execution constraints before execution.

Without Traxes:
Decision logic may be distributed across application code, infrastructure policies, orchestration systems, and operational procedures.

With Traxes:
The decision becomes a single deterministic evaluation with a replayable artifact.

## Replay Verification

TRAXES can replay historical decision artifacts and verify that the same input context and policy version produce the same decision result. Historical artifacts can be replayed at any time—the same action and policy version can be evaluated again to verify the original decision.

Replay verification enables:

- **Decision verification**: Confirm that ALLOW/DENY decisions are reproducible
- **Policy integrity**: Verify policy ID, version, and hash match the original evaluation
- **Governance validation**: Confirm governance coverage status is consistent
- **Rule evaluation**: Verify individual rule evaluation results match

### Replay Command

Replay a specific artifact using its decision ID:

```bash
traxes-demo --dev replay <decision_id>
```

### Example Output

```
TRAXES Replay Verification

Loading artifact: dec_4a89bb0975394012a6c9bc091313e217
Original decision: DENY
Policy hash: 785e022b9921ee6a43ab5044cc8e4a67930c936a1a521f4479269e47eacc52e8
Policy ID: infra-cost-limit-v1
Policy version: 1.0.0
Governance status: GOVERNED
Endpoint: AWS_RDS_PROVISION::staging

Re-evaluating policy...
Replay decision: DENY

VERIFICATION RESULT:
REPLAY MATCH

✓ Decision matches: DENY == DENY
✓ Policy hash matches: 785e022b9921ee6a43ab5044cc8e4a67930c936a1a521f4479269e47eacc52e8
✓ Governance status: GOVERNED
✓ Rule evaluation: infra-cost-limit (true)
```

If any verification check fails, TRAXES outputs `REPLAY MISMATCH` with specific details about what changed. The original artifact is never modified during replay verification.

## Operational Guarantees

Traxes provides three operational properties:

**Auditability**: Every decision produces an artifact that captures the complete evaluation context and rule evaluation evidence. You can replay any decision to verify why an action was allowed or denied.

**Deterministic Evaluation**: Given the same action and policy version, Traxes produces the same decision. This reproducibility is enforced through behavioral invariants.

**Reproducibility**: Artifact structure is stable across runs for a given policy version. Artifacts contain the evidence needed to reproduce and verify the decision. This enables post-hoc analysis, debugging, and compliance verification.

## Invariants

Traxes enforces a fixed set of behavioral invariants across all evaluations:

- Given the same action and policy version, Traxes produces the same decision.
- Every decision produces a replayable audit artifact containing decision context and rule evaluation evidence.
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
  "artifact_version": "2.0.0",
  "artifact_type": "pre_execution_decision",
  "decision_id": "dec_4a89bb0975394012a6c9bc091313e217",
  "timestamp": "2026-07-09T00:43:39.822410400+00:00",
  "decision": "DENY",
  "tool": "AWS_RDS_PROVISION",
  "environment": "staging",
  "reason": "instance_type is not allowed for this environment",
  "policy_bundle": "infra-cost-limit-v1",
  "policy_hash": "785e022b9921ee6a43ab5044cc8e4a67930c936a1a521f4479269e47eacc52e8",
  "sha256_hash": "968c0bb3445217be5f89084ca449e79bbefb7f67a645bf94efd2fba4bf24b7a4",
  "engine": {
    "name": "Traxes",
    "engine_version": "0.3.2",
    "policy_bundle_id": "infra-cost-limit-v1"
  },
  "policy_info": {
    "policy_id": "infra-cost-limit-v1",
    "policy_version": "1.0.0",
    "policy_hash": "785e022b9921ee6a43ab5044cc8e4a67930c936a1a521f4479269e47eacc52e8"
  },
  "execution_context": {
    "session_id": "demo-001",
    "trace_id": "dec_4a89bb0975394012a6c9bc091313e217"
  },
  "proposed_action": {
    "tool": "AWS_RDS_PROVISION",
    "environment": "staging",
    "parameters": {
      "instance_type": "m5.large",
      "instance_cost_per_hour": 0.5
    }
  },
  "rule_evaluation": {
    "rule_id": "infra-cost-limit",
    "field": "proposed_action.parameters.instance_type",
    "observed_value": "m5.large",
    "operator": "infra-cost-limit",
    "policy_value": 0.0,
    "evaluation_expression": "proposed_action.parameters.instance_type not_in policy",
    "evaluation_result": true,
    "rule_order": 0
  },
  "rules_evaluated": [
    {
      "rule_id": "infra-cost-limit",
      "field": "proposed_action.parameters.instance_type",
      "observed_value": "m5.large",
      "operator": "infra-cost-limit",
      "policy_value": 0.0,
      "evaluation_expression": "proposed_action.parameters.instance_type not_in policy",
      "evaluation_result": true,
      "rule_order": 0
    }
  ],
  "performance": {
    "evaluation_latency_us": 19.0,
    "decision_latency_us": 4.8,
    "artifact_write_latency_us": 41.7
  },
  "side_effect_prevention": {
    "decision_effect": "DENY"
  },
  "governance_info": {
    "coverage_status": "GOVERNED",
    "endpoint": "AWS_RDS_PROVISION::staging",
    "enforcement_hit": true,
    "coverage_event_type": "EnforcedPath"
  },
  "evaluation_trace": null,
  "execution_status": "blocked"
}
```

Artifacts are written to `artifacts/AuditArtifact_{decision_id}.json`.

Performance measurements are workload-dependent engineering benchmarks and are not universal latency guarantees.

## Why Traxes

Traxes is designed for systems where actions must be:

- Evaluated before execution
- Reproducible under replay
- Auditable after the fact
- Deterministic across runs
- Independent from execution side effects

Examples include AI agents, infrastructure automation, robotics, and systems requiring controlled execution boundaries.

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

Example CLI demo inputs are available after cloning the repository. Complete end-to-end walkthrough:

```bash
git clone https://github.com/caminodynamics/traxes
cd traxes
cargo build --release

# Step 1: Evaluate ALLOW payload
./target/release/traxes-demo eval payloads/allow_db.json

# Step 2: Evaluate DENY payload
./target/release/traxes-demo eval payloads/provision_db_deny.json

# Step 3: Inspect the most recent artifact
./target/release/traxes-demo --dev artifacts last

# Step 4: Replay the artifact and verify MATCH
./target/release/traxes-demo --dev replay <decision_id>
```

The `payloads/` directory contains example CLI demo inputs used to demonstrate ALLOW and DENY outcomes.

**Expected Output:**

ALLOW evaluation:
```
→ DECISION: ALLOW
```

DENY evaluation:
```
→ DECISION: DENY
```

Artifact inspection:
```
decision: DENY
policy: infra-cost-limit-v1
artifact: AuditArtifact_dec_4a89bb0975394012a6c9bc091313e217.json
```

Replay verification:
```
VERIFICATION RESULT:
REPLAY MATCH

✓ Decision matches: DENY == DENY
✓ Policy hash matches: 785e022b9921ee6a43ab5044cc8e4a67930c936a1a521f4479269e47eacc52e8
✓ Governance status: GOVERNED
✓ Rule evaluation: infra-cost-limit (true)
```

**Decision Evidence Artifacts**

TRAXES writes comprehensive decision artifacts to `artifacts/AuditArtifact_{decision_id}.json` for every evaluation. Each artifact includes:

- **Decision context**: `decision_id`, `timestamp`, ALLOW/DENY outcome
- **Policy information**: `policy_id`, `policy_version`, `policy_hash`
- **Proposed action**: `tool`, `environment`, `parameters` evaluated
- **Rule evaluation**: Per-rule evaluation results with observed values
- **Governance status**: `governance_status`, endpoint, enforcement hit
- **Performance metrics**: Evaluation, decision, and artifact write latency
- **Execution context**: `session_id` and `trace_id` for distributed tracing
- **Evaluation trace**: Optional detailed evaluation steps (if enabled)
- **Replay verification**: Verification result from replay operations

This evidence enables post-hoc analysis, compliance verification, and audit trails without requiring the original evaluation context.

## Requirements

- Rust toolchain for source builds
- Prebuilt binaries available in GitHub Releases

## Links

- Repository: https://github.com/caminodynamics/traxes
- Issues: https://github.com/caminodynamics/traxes/issues

## Next step

Run the demo, inspect the decision artifact, then modify the Rust policy definitions to evaluate your own action schema.

---
