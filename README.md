# TRAXES

TRAXES is a deterministic pre-execution evaluation layer for automated systems.

It evaluates proposed actions against versioned policies before execution, producing structured, replayable decision artifacts that make system behavior auditable and reproducible.

---

## Demo

Watch the 25-second TRAXES demonstration:

[bettertraxesvid.mp4](https://github.com/caminodynamics/traxes/releases/download/v0.1.0/bettertraxesvid.mp4)

---

## Installation

### Recommended (Fastest Evaluation Path)

Download the prebuilt binary and run directly:

```bash
# Download from GitHub Releases
# Visit: https://github.com/caminodynamics/traxes/releases/latest

# Download traxes-demo.exe for Windows (or equivalent for your platform)

# Run the binary directly
./traxes-demo demo
```

**Who should use it:** First-time evaluators and end users. **Setup effort:** ~1 minute (no build required).

---

### Developer Path

For developers who want to modify the codebase or contribute:

```bash
# Clone the repository
git clone https://github.com/caminodynamics/traxes.git
cd traxes/traxes-demo

# Build release binary
cargo build --release

# Run directly
cargo run --release --bin traxes-demo -- demo
```

**Who should use it:** Developers modifying TRAXES. **Setup effort:** ~5 minutes (requires Rust toolchain).

---

### Container Path

Run TRAXES in a Docker container:

```bash
# Build Docker image
docker build -t traxes -f Dockerfile .

# Run demo
docker run --rm traxes demo

# Evaluate payload
docker run --rm -v $(pwd)/demo/payloads:/app/payloads traxes eval /app/payloads/allow_t3medium.json
```

**Who should use it:** Teams requiring containerized deployments. **Setup effort:** ~10 minutes (requires Docker).

---

## Releases

Prebuilt binaries are available on [GitHub Releases](https://github.com/caminodynamics/traxes/releases/latest).

**Latest release (v0.1.1) includes:**
- `traxes-demo.exe` - Primary user-facing demo binary
- `traxes-bench.exe` - Benchmarking tool for performance measurement

Download the appropriate binary for your platform and run it directly. No build required.

---

## Quick Start

### Run Demo

```bash
# Interactive demo
./traxes-demo demo

# Fast mode (no delays)
./traxes-demo demo --demo-fast
```

### Evaluate Actions

```bash
# ALLOW case
./traxes-demo eval ../demo/payloads/allow_t3medium.json

# DENY case  
./traxes-demo eval ../demo/payloads/deny_t3large.json
```

### View Artifacts

```bash
# View last artifact
./traxes-demo artifacts last

# View full artifact JSON
./traxes-demo artifacts last --full
```

---

## Integration (Start Here)

TRAXES is designed as a deterministic execution gate between a system and its execution layer.

### Common integration points:

- Agent workflows → validate tool calls before execution
- API services → gate requests before side effects
- Automation pipelines → validate jobs before execution

### Typical placement:

Planner → TRAXES → Executor → System

TRAXES does not replace your system. It evaluates proposed actions before they execute and returns a decision + artifact.

---

## Failure Modes & Recovery

TRAXES is designed to fail safely.

- If evaluation fails → default behavior is DENY (fail-closed)
- If policy is invalid → evaluation is rejected with error
- If artifact writing fails → decision still returned, artifact marked as failed
- If timeout occurs → request is terminated and logged as failure

All failures produce a structured decision artifact for debugging and auditability.

---

## Policy Authoring

Policies define rules for ALLOW / DENY decisions.

### Basic structure:

- Match fields in the proposed action
- Apply conditions (equality, ranges, or logical rules)
- Return ALLOW or DENY

### Example progression:

**Simple rule:**
- Allow instance_type = t3.medium

**Medium rule:**
- Deny if environment = production AND instance_type = large

**Complex rule:**
- Combine multiple fields with AND/OR logic for environment-specific constraints

---

## Performance Expectations

Benchmarks reflect core evaluation performance only.

In real deployments, end-to-end latency depends on integration mode:

- In-process: minimal overhead (core engine only)
- Sidecar / HTTP: additional network + serialization overhead
- Distributed systems: depends on policy fetch + logging + artifact storage

TRAXES evaluation remains deterministic regardless of deployment mode.

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

## Planned Integration Targets

The following examples illustrate intended deployment patterns and are not currently implemented in this repository.

Do not imply that non-existent integrations are available today.

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

### Benchmark Snapshot

Scenario: rogue-infra-agent

* Allowed: 5,497
* Denied: 4,503
* Policy bypasses: 0

Core evaluation benchmark:

| Threads | p50    | p95    | p99     | Throughput    |
| ------- | ------ | ------ | ------- | ------------- |
| 1       | 300 ns | 700 ns | 1100 ns | 2.85M ops/sec |
| 4       | 300 ns | 800 ns | 1200 ns | 2.48M ops/sec |
| 16      | 300 ns | 900 ns | 1400 ns | 2.20M ops/sec |

Measured using the traxes-bench benchmark harness.

Important note:

These measurements represent core policy evaluation performance and should not be confused with full end-to-end application latency, artifact persistence, network transport, or external service overhead.

---

## Operational Guarantees

### Fail-Closed Behavior
- Malformed or invalid input → DENY
- Missing or corrupted policy → DENY
- Unknown rule operators → DENY
- Policy evaluation errors → DENY

### Deterministic Evaluation
- Identical input + same policy version → identical output
- No randomness in decision logic
- Reproducible across executions

### No Side Effects
- Evaluation does not modify external state
- No network calls during policy evaluation
- No database writes during decision process
- Artifact persistence occurs after decision is finalized

### Timeout Behavior
- Default evaluation timeout: 100ms (configurable)
- Timeout → DENY decision
- Timeout logged in artifact with reason
- No partial decisions returned

### Artifact Durability
- Artifacts written to local disk (JSON)
- Synchronous write before response return
- File path: `artifacts/AuditArtifact_{decision_id}.json`
- No async persistence (current implementation)
- Storage failure logged but does not block decision

---

## Policy Authoring & Lifecycle

### Policy Structure
- **Rules**: Define conditions and actions (ALLOW/DENY)
- **Fields**: Targeted action parameters (e.g., `instance_type`, `cost_per_hour`)
- **Operators**: Matching logic (`numeric_lte`, `in_list`, `not_in`)
- **Actions**: Result when rule matches

### Versioning Model
- Hash-based policy identification (SHA256)
- Policy hash embedded in every artifact
- Enables deterministic replay of historical decisions
- Git-based policy repository recommended

### Rollout Strategy
- Test policy in staging environment first
- Use `traxes-bench` to validate against historical payloads
- Compare artifact outputs between old and new policies
- Deploy to production after validation

### Rollback Strategy
- Revert to previous policy version in repository
- Redeploy with known-good policy hash
- Historical artifacts remain valid for audit
- No data migration required

### Policy Testing
- Use `traxes-demo eval` with test payloads
- Run `traxes-bench` for regression testing
- Verify ALLOW/DENY decisions match expectations
- Check artifact contents for correct metadata

---

## Integration Guidance

### Decision Matrix

| Mode | When to Use | Latency | Complexity | Isolation |
|------|-------------|---------|------------|-----------|
| **In-process** | Single application, low latency required | Lowest | Low | None |
| **Sidecar** | Microservices, shared policy enforcement | Medium | Medium | High |
| **Middleware** | API gateway, centralized enforcement | Higher | Higher | High |

### Tradeoffs

**In-process Mode**
- Pros: Lowest latency, simplest deployment
- Cons: Per-application policy updates, no isolation

**Sidecar Mode** (HTTP on port 8082)
- Pros: Centralized policy management, isolation
- Cons: Network overhead, additional infrastructure

**Middleware Mode**
- Pros: Enforcement at boundary, policy consistency
- Cons: Highest latency, requires integration point

---

## OPA Comparison

### What is OPA
Open Policy Agent (OPA) is a general-purpose policy engine that uses Rego language for policy definition and evaluation.

### TRAXES Differentiation

**Pre-Execution Deterministic Decision Gate**
- TRAXES: Decisions occur BEFORE action execution
- OPA: Can be used at various points in request lifecycle

**Replayable Artifacts**
- TRAXES: Every decision produces a structured, replayable artifact with full context
- OPA: Decision logging is optional, not built-in

**Execution Boundary Enforcement**
- TRAXES: Designed as a hard boundary between intent and execution
- OPA: General-purpose policy evaluation, not execution-specific

**Focus**
- TRAXES: Deterministic pre-execution validation for automated systems
- OPA: General policy engine for cloud-native workloads

---

## Debugging & Observability

### Why Was Action Denied?
- Check artifact `reason` field
- Review `rule_evaluation` section in artifact
- Examine `observed_value` vs `policy_value`
- Verify `evaluation_expression` logic

### Inspect Decision Artifacts
```bash
# View last artifact
traxes-demo artifacts last

# View full artifact JSON
traxes-demo artifacts last --full

# Replay specific artifact
traxes-demo replay artifacts/AuditArtifact_{decision_id}.json
```

### Replay Decision from Artifact
- Artifact contains full input and policy hash
- Replay reproduces identical decision
- Useful for post-incident analysis
- Validates policy behavior over time

### Artifact Storage Failure
- Decision still returned to caller
- Failure logged to stderr
- Artifact path marked as failed in output
- No impact on decision logic

---

## Artifact Querying Pattern

### Retrieve Past Decisions

**By Decision ID**
```bash
# Direct file access
cat artifacts/AuditArtifact_{decision_id}.json
```

**By Timestamp**
```bash
# List artifacts sorted by time
ls -lt artifacts/
```

**By Policy Version**
```bash
# Search artifacts by policy hash
grep -r "policy_hash" artifacts/ | grep "{hash}"
```

**By Decision Type**
```bash
# Find all DENY decisions
grep -l '"decision": "DENY"' artifacts/*.json
```

### Query Patterns

**CLI Examples**
```bash
# Count ALLOW vs DENY
grep -c '"decision": "ALLOW"' artifacts/*.json
grep -c '"decision": "DENY"' artifacts/*.json

# Find decisions for specific tool
grep -l '"tool": "AWS_RDS_PROVISION"' artifacts/*.json

# Extract decision IDs
jq -r '.decision_id' artifacts/*.json
```

**Conceptual Queries** (for custom tooling)
- Filter by time range
- Group by policy version
- Aggregate by tool/environment
- Search by reason text

