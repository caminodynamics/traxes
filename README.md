# TRAXES

TRAXES is a deterministic pre-execution evaluation layer that validates proposed actions against versioned policy bundles and produces replayable decision artifacts.

It is designed for high-integrity workflows such as AI agent tool governance, infrastructure automation, and compliance-driven automation.

## The Problem

Autonomous systems and AI agents execute real-world actions (API calls, infrastructure changes, tool execution). Many systems log what happened, but cannot reproducibly explain *why* a specific action was permitted or blocked at a specific point in time. Logic is often scattered across application code, making it difficult to verify and review.

TRAXES decouples decision logic from execution logic, providing a deterministic evidence trail that can be verified and replayed.

## How It Works

TRAXES evaluates actions before they proceed to the execution layer:

1.  **Proposal**: A system (Agent/Service) proposes an action.
2.  **Evaluation**: TRAXES evaluates the action against a versioned policy bundle.
3.  **Gate**: TRAXES returns a strictly typed ALLOW or DENY decision.
4.  **Evidence**: TRAXES writes a replayable decision artifact containing full evaluation context and cryptographic policy hashes.

### Execution Flow

```text
┌─────────────────────────────────┐
│ Planner / Agent / Service       │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ TRAXES DECISION ENGINE          │
│ • Deterministic Policy Match    │
│ • ALLOW/DENY Decision           │
│ • Decision Artifact Generation  │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ Execution Layer                 │
│ (Triggered only on ALLOW)       │
└─────────────────────────────────┘
```

TRAXES sits at the boundary between intent and action. It does not execute the action itself but provides the deterministic "Yes/No" and the evidence to support it.

## Core Properties

*   **Deterministic Evaluation**: Given the same action and policy version, the engine produces an identical result.
*   **Replayable Artifacts**: Every decision produces an immutable record containing rule evaluation evidence.
*   **Policy Versioning**: Artifacts include SHA-256 policy hashes to ensure the exact policy version can be identified and re-run.
*   **Fail-Closed Design**: Malformed inputs, missing policies, or internal errors resolve to an explicit **DENY**.

## Quick Start (Demo)

### Prebuilt Binaries (Windows)

1.  Download `traxes-demo.exe` from [GitHub Releases](https://github.com/caminodynamics/traxes/releases).
2.  Run the interactive demo:
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

## Performance & Reliability

TRAXES is optimized for high-performance evaluation paths.

*   **Latency**: 0.17-0.61μs average evaluation (engine logic only, 10K iterations, varies across runs).
*   **Throughput**: 625K-811K ops/sec (single-threaded benchmark, 10K iterations, varies across runs).
*   **Reliability**: 100% pass rate (18/18 scenarios) in reliability validation, covering concurrency safety, fuzzing, and malformed input handling.

See [RELIABILITY.md](RELIABILITY.md) and [PERFORMANCE.md](../PERFORMANCE.md) for detailed metrics.

## Coverage Tracking

TRAXES tracks policy match coverage across evaluated endpoints (Tool + Environment). Each artifact includes:
*   **Coverage Status**: `GOVERNED` or `UNGOVERNED`.
*   **Endpoint Identification**: (e.g., `AWS_RDS_PROVISION::staging`).
*   **Policy Match Hit**: Verification that a rule actively matched the evaluation path.

This allows governance teams to identify gaps in policy coverage without requiring a manual inventory of every possible action.

## Requirements

- Rust toolchain for source builds
- Prebuilt binaries available in GitHub Releases

## Links

- Repository: https://github.com/caminodynamics/traxes
- Issues: https://github.com/caminodynamics/traxes/issues
