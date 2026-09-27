# MCP FILE_WRITE integration proof

This proof exposes one MCP tool, `file_write(path, content)`, and routes every
call through the existing TRAXES HTTP execution boundary.

```text
MCP client / agent
      |
      v
traxes_mcp_adapter.py
      |
      | POST /evaluate
      v
TRAXES
      |
      +-- ALLOW -> TRAXES performs FILE_WRITE
      |
      +-- DENY  -> no target write
```

The adapter itself never writes the requested target file and has no fallback
execution path. If TRAXES cannot be reached or returns an invalid response, the
MCP tool fails closed.

## Windows / PowerShell proof

From the repository root:

```powershell
py -m pip install -r requirements-mcp.txt
```

Terminal 1 - run TRAXES with the isolated MCP proof policy:

```powershell
cargo run -- --dev server --policy policies/file_write_mcp_policy.yaml --demo-mode
```

Terminal 2 - run the MCP adapter over Streamable HTTP:

```powershell
py traxes_mcp_adapter.py
```

Terminal 3 - run the MCP client proof:

```powershell
py mcp_filewrite_smoke_client.py
```

Expected behavior:

- `temp_executed_mcp_allowed.txt` is created only after TRAXES returns ALLOW.
- `temp_executed_mcp_forbidden.txt` is not created after TRAXES returns DENY.
- The ALLOW artifact reports `execution_status=executed`.
- The DENY artifact reports `execution_status=blocked`.
- The smoke client prints `PASS`.

If either proof target already exists from an earlier run, remove it manually
before rerunning the smoke test.
