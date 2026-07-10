# TRAXES v1.0.0

TRAXES is a deterministic pre-execution control layer that evaluates proposed actions against versioned policies and produces replayable decision artifacts. It sits between decision-making systems and execution systems, providing a deterministic control point before actions occur.

## Key Highlights

- **Deterministic Policy Evaluation**: Evaluates proposed actions against versioned policies with ALLOW/DENY decisions
- **Replayable Decision Artifacts**: Every decision produces an artifact containing complete evaluation context and rule evaluation evidence
- **Policy Hash Verification**: Artifacts include SHA256-based policy hash to detect policy modifications
- **Coverage Tracking**: Governance coverage status (GOVERNED/UNGOVERNED) with endpoint identification
- **Reliability Validation**: 18/18 tests passing covering malformed input handling, policy failure modes, artifact integrity verification, concurrent execution safety, and fuzz testing

## Download

Download the Windows binary from [GitHub Releases](https://github.com/caminodynamics/traxes/releases/tag/v1.0.0)

## Quick Start

```bash
# ALLOW evaluation
traxes-demo eval payloads/allow_db.json

# DENY evaluation
traxes-demo eval payloads/provision_db_deny.json

# Replay verification
traxes-demo --dev replay <decision_id>
```

## Documentation

- [README.md](../README.md) - Complete documentation
- [RELEASE_NOTES_v1.md](../RELEASE_NOTES_v1.md) - Detailed v1.0 release notes
- [RELIABILITY.md](../RELIABILITY.md) - Reliability test results and methodology
