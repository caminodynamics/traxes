# TRAXES

TRAXES is a deterministic pre-execution control layer that evaluates proposed actions against versioned policies and produces replayable decision artifacts.

Use cases include AI agent tool execution, infrastructure automation, robotics/device command validation, and compliance/governance workflows.

## The Problem

Autonomous systems and AI agents execute real actions in production environments. When an action is allowed or denied, many systems can record what happened, but cannot reproducibly explain why the decision was made. Pre-action validation logic is typically embedded in application code, duplicated across services, and scattered across infrastructure layers. As a result, decisions are difficult to reproduce after the fact, and audit trails are incomplete.

## How TRAXES Works

TRAXES evaluates actions before execution and produces replayable decision artifacts:

1. A system proposes an action
2. TRAXES evaluates the action against a versioned policy
3. TRAXES returns ALLOW or DENY
4. TRAXES writes a decision artifact capturing the evaluation context and evidence

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

TRAXES sits between decision-making systems and execution systems, providing a deterministic control point before actions occur. TRAXES does not execute actions—it evaluates them and produces a decision artifact that downstream systems consume.

### Core Properties

- **Deterministic evaluation**: Given the same action and policy version, TRAXES produces the same decision
- **Reproducible decision artifacts**: Every decision produces an artifact containing complete evaluation context and rule evaluation evidence
- **Policy version tracking**: Artifacts include policy hash to detect policy modifications
- **Fail-closed behavior**: Malformed inputs resolve to explicit DENY or controlled failure states

## Quick Start

### Windows (Prebuilt Release Binary)

```powershell
# Run the interactive demo
.\traxes-demo.exe demo

# Run performance benchmarks
.\traxes-demo.exe benchmark
```

### Linux/macOS (Source Build)

```bash
# Build from source
cargo build --release

# Run the interactive demo
./target/release/traxes-demo demo

# Run performance benchmarks
./target/release/traxes-demo benchmark
```

The demo runs embedded ALLOW and DENY examples with no external dependencies.

### Release

- **Binaries**: Download from [GitHub Releases](https://github.com/caminodynamics/traxes/releases)
- **Documentation**: See [README.md](README.md) for complete documentation
- **Release Notes**: See [RELEASE_NOTES_v1.md](RELEASE_NOTES_v1.md) for v1.0 details
- **Reliability**: See [RELIABILITY.md](RELIABILITY.md) for test results and methodology
- **Benchmark documentation**: See [PERFORMANCE.md](PERFORMANCE.md)

### CLI Demo Evaluation (Repository Required)

```bash
git clone https://github.com/caminodynamics/traxes
cd traxes
cargo build --release

# ALLOW evaluation
traxes-demo eval payloads/allow_db.json

# DENY evaluation
traxes-demo eval payloads/provision_db_deny.json

# Replay verification
traxes-demo replay <artifact_id>
```

## Example Output

### Evaluation

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

```text
instance_type: m5.large
policy_version: aws_staging_guardrails@v3
decision: DENY
reason: instance_type not permitted for staging environment
artifact: /artifacts/91bc.json
```

### Replay Verification

```bash
traxes-demo --dev replay <decision_id>
```

```text
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

## Coverage Tracking

TRAXES tracks governance coverage for evaluated endpoints. Each decision artifact includes:

- **Coverage Status**: `GOVERNED` or `UNGOVERNED`
- **Endpoint**: Tool and environment combination (e.g., `AWS_RDS_PROVISION::staging`)
- **Enforcement Hit**: Whether a policy rule matched the evaluation
- **Coverage Event Type**: Classification of the evaluation path

Coverage tracking enables governance teams to identify ungoverned endpoints and verify that critical actions are covered by policy definitions.

## Benchmarks

Performance measurements from reliability testing (workload-dependent, not universal guarantees):

- **Typical evaluation latency**: ~4μs
- **P99 evaluation latency**: ~10μs
- **Throughput**: ~194K operations/second
- **Large payload handling**: Linear scaling from 1KB to 1MB (17μs at 1MB)

Measurements represent TRAXES evaluation workloads only and do not include downstream execution time, network latency, or external system calls.

Benchmarks were run on a sustained 60-second load test with 6,000 iterations. See `RELIABILITY.md` for detailed testing methodology.

## Reliability

TRAXES has completed reliability and validation testing covering:

- **18/18 tests passing** (100% success rate)
- No panics or crashes across all test scenarios
- Robust error handling for malformed inputs and policy failures
- Safe concurrent execution with no race conditions
- Verified artifact integrity and tamper detection

Test categories include malformed input handling, policy failure modes, artifact integrity verification, concurrent execution safety, long-duration stability, large payload stress, persistence failure handling, and fuzz testing.

See `RELIABILITY.md` for complete test results and reproducibility instructions.

## Requirements

- Rust toolchain for source builds
- Prebuilt binaries available in GitHub Releases

## Links

- Repository: https://github.com/caminodynamics/traxes
- Issues: https://github.com/caminodynamics/traxes/issues
