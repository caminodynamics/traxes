# TRAXES

TRAXES is a deterministic pre-execution evaluation layer for automated systems.

It evaluates proposed actions against versioned policies before execution, producing structured, replayable decision artifacts that make system behavior auditable and reproducible.

---

## Quick Start

### Build

```bash
cargo build --release
```

### Run Demo

```bash
# Run interactive demo
cargo run --release --quiet --bin traxes-demo -- demo

# Run in fast mode (no delays)
cargo run --release --quiet --bin traxes-demo -- demo --demo-fast
```

### Evaluate Actions

```bash
# ALLOW case
cargo run --release --bin traxes-demo -- eval ../demo/payloads/allow_t3medium.json

# DENY case
cargo run --release --bin traxes-demo -- eval ../demo/payloads/deny_t3large.json
```

### View Artifacts

```bash
# View last artifact
cargo run --release --bin traxes-demo -- artifacts last

# View full artifact JSON
cargo run --release --bin traxes-demo -- artifacts last --full
```

---

## Core Concept

Modern automated systems increasingly execute real-world actions through:

- AI agents
- backend workflows
- infrastructure automation
- distributed systems

However, most systems only log what happened after execution — not whether the action should have happened in the first place, or why it was allowed.

TRAXES introduces a pre-execution decision boundary:

Every action is evaluated before it is allowed to execute.

---

## Execution Model

TRAXES evaluates each proposed action through a deterministic pipeline:

1. A system proposes an action
2. TRAXES evaluates it against a versioned policy
3. TRAXES returns a decision:
   - ALLOW
   - DENY
   - (optional) MODIFY
4. A structured audit artifact is generated for every evaluation

---

## Decision Output

Each evaluation produces a replayable artifact containing:

- decision outcome
- policy hash reference
- evaluation trace
- metadata (timing, inputs, rule matches)
- unique decision identifier

These artifacts are designed to be:

- reproducible
- machine-readable
- audit-ready
- suitable for post-incident reconstruction

---

## Replayability Guarantee

Given identical inputs and the same policy version:

TRAXES produces identical decision outputs and artifacts.

This enables deterministic reconstruction of system behavior over time.

---

## System Properties

- Pre-execution enforcement — decisions occur before execution
- Deterministic evaluation — identical input → identical output
- Policy-bound decisions — every output tied to a versioned policy hash
- Fail-closed behavior — invalid or missing policy results in DENY
- No side effects during evaluation

---

## Example Flow

### Input

```json
{
  "action": "create_instance",
  "type": "t3.micro"
}
```

### Output

```
ALLOW
policy_hash: 8f3a91...
artifact: decision_10291.json
```

### Input

```json
{
  "action": "create_instance",
  "type": "m5.large"
}
```

### Output

```
DENY
reason: instance type not allowed under policy constraints
policy_hash: 8f3a91...
artifact: decision_10292.json
```

---

## Where TRAXES Fits

TRAXES is designed for systems where actions are:

- automated
- distributed
- irreversible or costly
- safety or compliance sensitive

Common environments include:

- AI agent execution pipelines (tool use, autonomous workflows)
- backend automation systems (jobs, triggers, orchestration layers)
- infrastructure provisioning systems (cloud resource creation, scaling)
- robotics systems (real-world action execution with safety constraints)
- drone and fleet coordination systems (multi-agent physical execution environments)

In these environments, TRAXES acts as a pre-execution control boundary, ensuring actions are evaluated before they are executed.

---

## Important Clarification

TRAXES is NOT:

- a robotics system
- a drone system
- a geospatial engine
- a workflow orchestration platform

These are deployment environments, not the product definition.

The core abstraction is:

deterministic evaluation of proposed actions before execution.

---

## Design Principle

TRAXES is built around one principle:

If a system can act, it should be able to justify — deterministically — why that action was allowed.

---

## Summary

TRAXES is a deterministic execution gate that evaluates actions before execution and produces replayable audit artifacts for every decision.

---

## Documentation

### Example Policy

```yaml
version: "2026-06-16"

rules:
  - id: instance_size_limit
    action: create_instance
    allow:
      - t3.micro
      - t3.small
      - t3.medium
    deny_reason: "instance type exceeds allowed limit"
```

Policies define what actions are allowed before execution.

---

### Example Decision Artifact

```json
{
  "decision_id": "dec_7f92a1",
  "decision": "DENY",
  "policy_version": "2026-06-16",
  "policy_hash": "sha256:8f3a91c2",
  "action": {
    "type": "create_instance",
    "instance_type": "m5.large"
  },
  "matched_rules": [
    {
      "rule_id": "instance_size_limit",
      "result": "FAIL"
    }
  ],
  "timestamp": "2026-06-16T10:21:33Z",
  "replayable": true
}
```

Every decision is replayable and audit-safe.

---

### Integration Model

```
Agent / Workflow / System
        │
        ▼
     TRAXES
        │
  ALLOW / DENY + ARTIFACT
        │
        ▼
Execution Layer (AWS, APIs, robots, workflows)
```

- In-process library mode (embedded in runtime)
- Sidecar mode (external evaluation service)
- Middleware mode (pre-execution gate in tool pipeline)

TRAXES sits between intent and execution.

---

## Integration Examples

### Python In-Process Library

```python
import traxes

# Initialize engine with policy
engine = traxes.Engine(policy_path="policies/production.yaml")

# Evaluate action before execution
action = {
    "tool": "aws.ec2.provision",
    "environment": "staging",
    "parameters": {
        "instance_type": "t3.medium",
        "cost_per_hour": 0.04
    }
}

decision = engine.evaluate(action)
if decision.decision == "ALLOW":
    execute_action(action)
    log_artifact(decision.artifact)
else:
    log_denial(decision.reason)
```

### Python Sidecar (HTTP Service)

```python
import requests

# Traxes runs as HTTP service on port 8082
response = requests.post(
    "http://localhost:8082/evaluate",
    json={
        "tool": "aws.ec2.provision",
        "environment": "staging",
        "parameters": {
            "instance_type": "t3.medium",
            "cost_per_hour": 0.04
        }
    }
)

result = response.json()
if result["decision"] == "ALLOW":
    execute_action(action)
```

### Go WASM Module Sketch

```go
// Traxes compiled to WASM for edge deployment
package main

import "github.com/traxes/traxes-go"

func EvaluateAction(action []byte) (string, error) {
    engine := traxes.LoadEngine("policy.wasm")
    decision, _ := engine.Evaluate(action)
    return decision.Decision, nil
}
```

### Performance Baseline

**Local evaluation (Rust release build):**
- p50 latency: ~5µs
- p99 latency: ~12µs
- Throughput: ~200K ops/sec (single-threaded)

**HTTP service (sidecar mode):**
- p50 latency: ~5.3ms (including network round-trip)
- p99 latency: ~12ms
- Throughput: ~10K req/sec (concurrent)

Integration adds minimal latency to execution pipelines.

