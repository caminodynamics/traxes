# Traxes

Traxes is a deterministic execution gate for autonomous systems that evaluates proposed actions against versioned policies before execution and produces replayable, auditable decision artifacts.

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

Traxes provides three operational capabilities:

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

**ALLOW**

```text
instance_type: t3.medium
policy_version: aws_staging_guardrails@v3
decision: ALLOW
artifact: /artifacts/8f3a.json
```

**DENY**

```text
instance_type: m5.large
policy_version: aws_staging_guardrails@v3
decision: DENY
reason: instance_type not permitted
artifact: /artifacts/91bc.json
```

## Why Traxes

Traxes is designed for systems where actions must be:

- Evaluated before execution
- Reproducible under replay
- Auditable after the fact
- Deterministic across runs
- Independent from execution side effects

Examples include AI agents, infrastructure automation, robotics, and safety-critical systems.

## Quick Start

```bash
# Run the interactive demo (no arguments required)
./traxes-demo demo

# Evaluate your own payload file
./traxes-demo eval my_payload.json

# Run performance benchmarks
./traxes-demo benchmark
```

The demo runs embedded ALLOW and DENY examples with no external dependencies.

## Requirements

- Prebuilt binaries available in GitHub Releases
- Rust toolchain for source builds
- Optional Docker support for containerized execution

## Links

- Releases: https://github.com/caminodynamics/traxes/releases/latest
- Issues: https://github.com/caminodynamics/traxes/issues
- Demo video: https://github.com/caminodynamics/traxes/releases/download/v0.1.0/bettertraxesvid.mp4

## Next step

Run the demo, inspect the decision artifact, then adapt a policy to your own action schema.

---
