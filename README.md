# Traxes (Developer Preview)

A deterministic pre-execution policy engine that evaluates actions before execution and returns:

**ALLOW / DENY + a replayable audit artifact**

---

## What this is

Traxes is a lightweight policy evaluation engine that sits before execution of an action (API call, infrastructure change, agent tool use, etc).

It evaluates the action against a policy and produces:

- A deterministic decision (ALLOW / DENY)
- A structured audit artifact for replay and inspection

---

## Core Model

Action → Evaluate → Decision → Artifact

Every evaluation is deterministic and produces a replayable result.

---

## Example

### Input (Action)

```json
{
  "tool": "aws.rds.provision",
  "environment": "staging",
  "instance": "db.t3.medium"
}
```

### Policy

Defined in policies/

Example:

```yaml
allow:
  environment: staging
  instance: db.t3.medium
```

### Output

**ALLOW**

```json
{
  "decision": "ALLOW",
  "policy_version": "v1",
  "action_hash": "abc123",
  "timestamp": 1710000000
}
```

---

## Why this exists

Modern systems increasingly rely on:

- distributed services
- infrastructure automation
- AI agents taking actions

This creates a need for:

- deterministic pre-execution validation of actions before they are executed

Traxes provides a simple, explicit layer for that decision step.

---

## Features

- Deterministic policy evaluation
- Pre-execution decision enforcement
- Structured audit artifacts
- Simple YAML-based policies
- CLI + demo execution flow
- Designed for agent / infra / API guardrails

---

## Run the demo

1. Build

```bash
cargo build
```

2. Run example evaluation

```bash
cargo run --bin demo
```

3. Or use the script

```bash
./demo.bat
```

---

## Project Structure

- `policies/` → policy definitions (YAML)
- `payloads/` → example actions
- `src/` → core engine
- `examples/` → sample integrations
- `docs/` → extended docs

---

## Output format

Every evaluation produces:

- decision (ALLOW / DENY)
- policy context
- action metadata
- replayable artifact

---

## Status

Developer preview / early system prototype.

Core evaluation logic is stable. APIs and structure may evolve.

---

## License

TBD

