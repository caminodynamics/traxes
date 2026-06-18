# Traxes

Traxes is a deterministic execution gate for automated systems.

It sits between intent and execution, evaluating proposed actions against versioned policies and producing ALLOW / DENY decisions with replayable audit artifacts.

## Why this exists

Modern automated systems execute real actions, but their pre-action validation logic is typically:

- embedded in application code
- duplicated across services
- or scattered across infrastructure layers

As a result, decisions are difficult to reproduce after the fact.

Traxes replaces this pattern with a single deterministic gate that makes every decision reproducible and auditable.

## Core model

1. A system proposes an action.
2. Traxes evaluates the action against a versioned policy.
3. Traxes returns ALLOW or DENY.
4. Traxes writes a deterministic audit artifact capturing the decision.

This creates a strict separation between decision and execution.

## Invariants

Traxes enforces a fixed set of behavioral invariants across all evaluations:

- Given the same action and policy version, Traxes produces the same decision.
- Every decision produces a deterministic, replayable audit artifact.
- Policy evaluation is isolated from execution and has no side effects.
- All malformed inputs resolve to explicit DENY or controlled failure states.
- Artifact structure is stable across runs for a given policy version.

These invariants are intended to make system behavior reproducible under replay.

## Integration

Traxes operates at the boundary between decision-making and execution.

It is typically embedded or deployed as a pre-execution gate in existing systems:

- AI agent tool execution pipelines
- infrastructure provisioning workflows
- backend service action validation
- robotics and device command execution layers

### Execution flow

```
[Planner / Agent / Service]
          ↓
       Traxes
 (ALLOW / DENY + artifact)
          ↓
     Execution Layer
```

Traxes does not execute actions. It evaluates them and produces a deterministic decision artifact that downstream systems consume.

## What Traxes provides

- Deterministic pre-execution evaluation
- Versioned policy decisions
- Replayable audit artifacts
- Fail-closed behavior on malformed input or policy errors
- Clear boundary between planning and execution

## Where it fits

Traxes is typically used as a pre-action control layer for systems that execute real operations, such as:

- AI agent tool execution
- infrastructure provisioning workflows
- backend automation systems
- robotics and device command validation

## Example

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

## Quick start

```bash
./traxes-demo demo

./traxes-demo eval ../demo/payloads/allow_t3medium.json

./traxes-demo artifacts last --full
```

The last command returns the full deterministic audit artifact for the most recent decision.

## Requirements

- Prebuilt binaries available in GitHub Releases
- Rust toolchain for source builds
- Optional Docker support for containerized execution

## What it is not

- Not a workflow engine
- Not a general agent framework
- Not an observability system
- Not a policy DSL product

## Links

- Releases: https://github.com/caminodynamics/traxes/releases/latest
- Issues: https://github.com/caminodynamics/traxes/issues
- Demo video: https://github.com/caminodynamics/traxes/releases/download/v0.1.0/bettertraxesvid.mp4

## Next step

Run the demo, inspect the decision artifact, then adapt a policy to your own action schema.
