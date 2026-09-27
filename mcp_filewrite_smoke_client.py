#!/usr/bin/env python3
"""End-to-end smoke test for the TRAXES MCP FILE_WRITE adapter.

The client never writes or deletes the target files directly. It calls the MCP
tool, then reads filesystem and artifact state to verify TRAXES behavior.
"""

import asyncio
import json
import os
import time
from pathlib import Path

from mcp import Client

MCP_SERVER = os.getenv("TRAXES_MCP_URL", "http://127.0.0.1:8000/mcp")
ALLOW_PATH = Path("temp_executed_mcp_allowed.txt")
DENY_PATH = Path("temp_executed_mcp_forbidden.txt")
CONTENT = "written only after TRAXES ALLOW through MCP"


def wait_for_artifact(path_text: str, timeout_seconds: float = 3.0) -> dict:
    path = Path(path_text)
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if path.exists():
            return json.loads(path.read_text(encoding="utf-8"))
        time.sleep(0.05)
    raise AssertionError(f"artifact did not appear: {path}")


async def call_file_write(client: Client, path: Path, label: str) -> dict:
    print(f"[{label}] MCP file_write -> {path}")
    result = await client.call_tool(
        "file_write",
        {"path": path.as_posix(), "content": CONTENT},
    )

    if result.is_error:
        raise AssertionError(f"MCP tool returned an error: {result.content}")

    data = result.structured_content
    if not isinstance(data, dict):
        raise AssertionError(f"missing structured MCP result: {data!r}")

    print(f"  decision      : {data['decision']}")
    print(f"  decision_id   : {data['decision_id']}")
    print(f"  artifact_path : {data['artifact_path']}")
    return data


async def main() -> None:
    print("=== TRAXES MCP FILE_WRITE proof ===")
    print(f"MCP server: {MCP_SERVER}")

    existing = [path for path in (ALLOW_PATH, DENY_PATH) if path.exists()]
    if existing:
        names = ", ".join(str(path) for path in existing)
        raise SystemExit(
            f"Proof target already exists: {names}. Remove it once before rerunning."
        )

    async with Client(MCP_SERVER) as client:
        allow = await call_file_write(client, ALLOW_PATH, "ALLOW")
        assert allow["decision"] == "ALLOW", allow
        assert ALLOW_PATH.exists(), "ALLOW did not create the target file"
        assert ALLOW_PATH.read_text(encoding="utf-8") == CONTENT
        allow_artifact = wait_for_artifact(allow["artifact_path"])
        assert allow_artifact["execution_status"] == "executed"
        print(
            "  proof         : exists=true, content_match=true, "
            "execution_status=executed"
        )

        deny = await call_file_write(client, DENY_PATH, "DENY")
        assert deny["decision"] == "DENY", deny
        assert not DENY_PATH.exists(), "DENY unexpectedly created the target file"
        deny_artifact = wait_for_artifact(deny["artifact_path"])
        assert deny_artifact["execution_status"] == "blocked"
        print("  proof         : exists=false, execution_status=blocked")

    print()
    print("PASS - MCP tool calls are governed by TRAXES before FILE_WRITE execution.")


if __name__ == "__main__":
    asyncio.run(main())
