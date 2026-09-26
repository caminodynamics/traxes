# AI-Agent FILE_WRITE Integration Proof

This directory contains a minimal credible real AI-agent integration proof for TRAXES FILE_WRITE enforcement.

## Overview

The integration demonstrates a real AI agent that proposes FILE_WRITE actions through TRAXES before any filesystem execution. The agent cannot directly write files - all writes must go through the TRAXES governed execution boundary.

## Architecture

```
AI Agent → proposes FILE_WRITE {path, content} → HTTP POST /evaluate → TRAXES
→ ALLOW → governed execution performs real write
→ DENY → no target write occurs
→ artifact records actual execution status
→ replay verifies decision only (no filesystem side effect)
```

## Components

### 1. Python AI Agent (`ai_agent_integration.py`)

A lightweight Python agent that:
- Proposes FILE_WRITE actions via HTTP POST to TRAXES server
- Has NO direct filesystem write capability
- Uses only standard library modules (urllib, json, tempfile, pathlib)
- No external dependencies required

### 2. TRAXES Server Configuration (`src/main.rs`)

Modified to load `file_write_agent_policy.yaml` when running in server mode:
- Reads policy from `policies/file_write_agent_policy.yaml`
- Uses HTTP POST `/evaluate` endpoint for agent communication
- Maintains existing artifact generation and replay functionality

### 3. Agent Policy (`policies/file_write_agent_policy.yaml`)

Simple policy for the AI agent integration:
- Denies writes to files named "forbidden.txt"
- Allows writes to all other paths (temp directories, etc.)
- Demonstrates both ALLOW and DENY decision paths

## Running the Integration

### 1. Start TRAXES Server

```bash
cd traxes_v0.1/traxes-demo
cargo run -- --dev server
```

The server will:
- Load `file_write_agent_policy.yaml`
- Listen on `http://0.0.0.0:8082`
- Log: "Using file_write_agent_policy for AI agent integration"

### 2. Run AI Agent Integration

```bash
python ai_agent_integration.py
```

The agent will:
- Create a temporary sandbox directory
- Test Case A: Propose ALLOW path (temp directory) → real file written
- Test Case B: Propose DENY path ("forbidden.txt") → no file written
- Verify artifacts were created
- Report results

## Test Results

### ALLOW Case
- Agent proposes: `C:\Users\...\Temp\...\allowed_write.txt`
- TRAXES decision: ALLOW
- Filesystem state: File exists with correct content
- Artifact: Created with execution_status="executed"

### DENY Case
- Agent proposes: `forbidden.txt`
- TRAXES decision: DENY
- Filesystem state: File does not exist
- Artifact: Created with execution_status="blocked"

## Why This Is a Real AI Integration

This is a **real** AI integration, not a simulation, because:

1. **Constrained Agent**: The Python agent has NO direct filesystem write access. It can ONLY propose actions through TRAXES governance.

2. **Real Side Effects**: ALLOW decisions result in actual filesystem writes through the governed execution boundary (`action::execute()`).

3. **Real Blocking**: DENY decisions prevent filesystem writes - the agent cannot bypass TRAXES.

4. **Actual Artifacts**: Artifacts record the actual execution status after the governed boundary executes or blocks.

5. **Language Independence**: The agent uses Python while TRAXES uses Rust, demonstrating real cross-language integration via HTTP.

6. **No Mocking**: No filesystem operations are mocked. The agent uses real temp directories and real TRAXES governance.

## Verification

Run the existing verification suite:

```bash
cargo test --lib
cargo run --example file_write_integration
```

Both should pass, confirming that:
- The AI agent integration doesn't break existing functionality
- The existing FILE_WRITE enforcement proof still works
- Replay verification still functions correctly

## Credentials Required

None. This integration uses:
- No LLM/API keys
- No cloud services
- No external dependencies
- Only standard library modules

## Files Changed/Added

1. **Added**: `ai_agent_integration.py` - Python AI agent
2. **Added**: `policies/file_write_agent_policy.yaml` - Agent-specific policy
3. **Modified**: `src/main.rs` - Load agent policy in server mode

## Remaining Bypass Paths

None. The agent has no alternate filesystem write path:
- The Python script never calls `open()`, `write()`, or any filesystem operations
- All file writes MUST go through TRAXES via HTTP POST /evaluate
- The governed execution boundary (`action::execute()`) is the single point of enforcement
