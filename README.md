# TRAXES

TRAXES is a deterministic pre-execution evaluation layer that validates proposed actions against versioned policy bundles and produces replayable decision artifacts.

It is designed for high-integrity workflows such as AI agent tool governance, infrastructure automation, and compliance-driven automation.

## The Problem

Autonomous systems and AI agents execute real-world actions (API calls, infrastructure changes, tool execution). Many systems log what happened, but cannot reproducibly explain *why* a specific action was permitted or blocked at a specific point in time. Logic is often scattered across application code, making it difficult to verify and review.

TRAXES decouples decision logic from execution logic, providing a deterministic evidence trail that can be verified and replayed.

## How It Works

TRAXES evaluates actions before they proceed to the execution layer:

1. **Proposal**: A system (Agent/Service) proposes an action.
2. **Evaluation**: TRAXES evaluates the action against a versioned policy bundle.
3. **Gate**: ALLOW can produce an internal, action-bound execution permit; DENY produces no permit.
4. **Execution**: Governed executors perform the side effect only when presented with valid authorization.
5. **Evidence**: TRAXES writes a replayable decision artifact containing evaluation context, execution outcome, and cryptographic policy hashes.

### Execution Flow

```text
┌─────────────────────────────────┐
│ Planner / Agent / Service       │
└────────────┬────────────────────┘
             │ proposed action
             ▼
┌─────────────────────────────────┐
│ TRAXES DECISION ENGINE          │
│ • Deterministic Policy Match    │
│ • ALLOW / DENY                  │
│ • Action-bound authorization    │
└────────────┬────────────────────┘
             │ permit only on ALLOW
             ▼
┌─────────────────────────────────┐
│ Governed Execution Layer        │
│ • Valid permit -> side effect   │
│ • No permit -> blocked          │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ Artifact + Replay               │
└─────────────────────────────────┘
```

TRAXES sits at the boundary between intent and action. The core engine separates policy evaluation from execution, while governed integrations can consume TRAXES authorization before performing a side effect.

## Core Properties

* **Deterministic Evaluation**: Given the same action and policy version, the engine produces an identical result.
* **Replayable Artifacts**: Decisions produce records containing rule evaluation evidence and, where execution is attempted, the observed execution outcome.
* **Policy Versioning**: Artifacts include SHA-256 policy hashes to identify the policy used for the decision.
* **Fail-Closed Design**: Missing authorization prevents governed execution.
* **Action-Bound Authorization**: External callers cannot directly mint execution permits; the intended public workflow obtains authorization through the engine.

## Quick Start (Demo)

### Prebuilt Binaries (Windows)

1. Download `traxes-demo.exe` from [GitHub Releases](https://github.com/caminodynamics/traxes/releases).
2. Run the interactive demo:

```powershell
.\traxes-demo.exe demo
```

### Source Build (All Platforms)

```bash
cargo build --release

# Run interactive demo
./target/release/traxes-demo demo

# Evaluate a specific payload
./target/release/traxes-demo eval payloads/allow_db.json

# Run performance benchmark
./target/release/traxes-demo benchmark

# Verify a decision artifact
./target/release/traxes-demo --dev replay <artifact_id>
```

## Hardened External-Agent FILE_WRITE Proof

This proof uses an external Python client that can only *propose* FILE_WRITE actions over HTTP. The client does not write the target files itself. TRAXES evaluates each proposed action, performs an allowed write through the governed execution path, blocks a denied write, emits artifacts, and replays both decisions with the same policy.

Build the project:

```powershell
cargo build
```

Start the TRAXES server with the dedicated FILE_WRITE policy:

```powershell
cargo run -- --dev server --policy policies/file_write_agent_policy.yaml
```

In a second terminal, run:

```powershell
python agent_filewrite_client.py
```

Expected result:

```text
[ALLOW] propose FILE_WRITE -> temp_executed_agent_allowed.txt
  decision      : ALLOW
  proof         : exists=true, content_match=true, execution_status=executed, execution_outcome=Executed
  replay        : verified

[DENY] propose FILE_WRITE -> temp_executed_agent_forbidden.txt
  decision      : DENY
  proof         : exists=false, execution_status=blocked, execution_outcome=Unauthorized
  replay        : verified

PASS - external client can propose actions, but TRAXES controls execution.
PASS - ALLOW executed, DENY produced no side effect, and both artifacts replayed.
```

The proof is model-agnostic: any agent or service able to propose the same action payload can use the same boundary.

## MCP Execution-Boundary Proof

TRAXES also exposes the hardened FILE_WRITE boundary as an MCP stdio server. An MCP client discovers `write_file` and `replay_decision`; the adapter routes FILE_WRITE requests through the same `evaluate_with_permit -> execute -> artifact` path used by the hardened integration rather than writing files directly.

Build the MCP server:

```powershell
cargo build --bin traxes-mcp
```

A manual MCP client can then call `write_file`. With `policies/file_write_agent_policy.yaml`, the allowed demo path executes while the forbidden path is blocked. Both resulting decision artifacts can be replayed through the `replay_decision` MCP tool.

The automated smoke test launches the MCP server as a child process and verifies tool discovery, ALLOW plus real file creation, DENY plus no file creation, and deterministic replay of both decisions:

```powershell
cargo test --test mcp_execution_boundary -- --nocapture
```

Expected result:

```text
test mcp_file_write_boundary_allow_deny_and_replay ... ok

test result: ok. 1 passed; 0 failed
```

## Local Verification

Until cloud CI is available, the core local verification set is:

```powershell
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo run --example file_write_integration
cargo test --test mcp_execution_boundary -- --nocapture
git diff --check
```

## Performance & Reliability

TRAXES is optimized for high-performance evaluation paths.

* **Latency**: 0.17-0.61μs average evaluation (engine logic only, 10K iterations, varies across runs).
* **Throughput**: 625K-811K ops/sec (single-threaded benchmark, 10K iterations, varies across runs).
* **Reliability**: 100% pass rate (18/18 scenarios) in reliability validation, covering concurrency safety, fuzzing, and malformed input handling.

See [RELIABILITY.md](RELIABILITY.md) and [PERFORMANCE.md](../PERFORMANCE.md) for detailed metrics.

## Coverage Tracking

TRAXES tracks policy match coverage across evaluated endpoints (Tool + Environment). Each artifact includes:
* **Coverage Status**: `GOVERNED` or `UNGOVERNED`.
* **Endpoint Identification**: (e.g., `AWS_RDS_PROVISION::staging`).
* **Policy Match Hit**: Verification that a rule actively matched the evaluation path.

This allows governance teams to identify gaps in policy coverage without requiring a manual inventory of every possible action.

## Requirements

- Rust toolchain for source builds
- Python 3 for the external-agent FILE_WRITE proof
- Prebuilt binaries available in GitHub Releases

## Links

- Repository: https://github.com/caminodynamics/traxes
- Issues: https://github.com/caminodynamics/traxes/issues
