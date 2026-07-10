# TRAXES v1 Quick Start Guide

This standalone release package requires no Rust installation. Extract the folder and run the demo.

## Getting Started

### 1. Run Interactive Demo
The demo runs embedded ALLOW and DENY examples with no external dependencies.

```powershell
.\traxes-demo.exe demo
```

### 2. Evaluate ALLOW Payload
```powershell
.\traxes-demo.exe eval payloads\allow_db.json
```

### 3. Evaluate DENY Payload
```powershell
.\traxes-demo.exe eval payloads\provision_db_deny.json
```

### 4. View Generated Artifacts
Artifacts are automatically written to the `artifacts/` directory. You can inspect them directly:

```powershell
dir artifacts\
```

## Package Contents

- `traxes-demo.exe` - Main binary (2.2 MB)
- `payloads/` - Example evaluation inputs
- `policies/` - Policy configuration files
- `README.md` - Full documentation

## Available Commands

```powershell
# Interactive demo
.\traxes-demo.exe demo

# Evaluate a payload file
.\traxes-demo.exe eval <payload-file>

# Run performance benchmarks
.\traxes-demo.exe benchmark

# Developer tools (requires --dev flag)
.\traxes-demo.exe --dev artifacts list
.\traxes-demo.exe --dev artifacts last
```

## Artifact Schema Version

This release uses TRAXES v1 artifact schema (version 2.0.0) with enhanced fields:
- `policy_info` - Policy metadata with version
- `governance_info` - Governance coverage tracking
- `rules_evaluated` - Multi-rule evaluation collection
- `rule_order` - Rule ordering for multi-rule support

## System Requirements

- Windows 10 or later
- No additional dependencies required
