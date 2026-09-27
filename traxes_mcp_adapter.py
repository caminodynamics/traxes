#!/usr/bin/env python3
"""MCP adapter that routes FILE_WRITE tool calls through TRAXES.

This process never writes target files itself. It only sends ProposedAction
payloads to the TRAXES HTTP boundary and returns the resulting decision.
"""

import json
import os
import urllib.error
import urllib.request
from typing import Literal

from mcp.server import MCPServer
from pydantic import BaseModel

TRAXES_SERVER = os.getenv("TRAXES_SERVER_URL", "http://127.0.0.1:8082")
MCP_HOST = os.getenv("TRAXES_MCP_HOST", "127.0.0.1")
MCP_PORT = int(os.getenv("TRAXES_MCP_PORT", "8000"))


class TraxesDecision(BaseModel):
    decision: Literal["ALLOW", "DENY"]
    decision_id: str
    artifact_path: str


mcp = MCPServer("TRAXES FILE_WRITE Adapter")


def _evaluate_file_write(path: str, content: str) -> TraxesDecision:
    action = {
        "tool": "FILE_WRITE",
        "session_id": "mcp-filewrite-adapter",
        "environment": "sandbox",
        "parameters": {
            "path": path,
            "content": content,
        },
    }

    request = urllib.request.Request(
        f"{TRAXES_SERVER}/evaluate",
        data=json.dumps(action).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )

    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            result = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(
            f"TRAXES request failed closed with HTTP {exc.code}: {body}"
        ) from exc
    except urllib.error.URLError as exc:
        raise RuntimeError(
            f"TRAXES is unreachable at {TRAXES_SERVER}; no target action was performed"
        ) from exc

    if not isinstance(result, dict):
        raise RuntimeError("TRAXES returned a non-object response; failing closed")

    decision = result.get("decision")
    if decision not in {"ALLOW", "DENY"}:
        raise RuntimeError(
            f"TRAXES returned an invalid decision {decision!r}; failing closed"
        )

    decision_id = result.get("decision_id")
    artifact_path = result.get("artifact_path")
    if not isinstance(decision_id, str) or not isinstance(artifact_path, str):
        raise RuntimeError(
            "TRAXES response is missing decision_id/artifact_path; failing closed"
        )

    return TraxesDecision(
        decision=decision,
        decision_id=decision_id,
        artifact_path=artifact_path,
    )


@mcp.tool()
def file_write(path: str, content: str) -> TraxesDecision:
    """Request a governed FILE_WRITE through TRAXES.

    The MCP adapter does not write the file itself and has no fallback execution
    path. TRAXES remains the authority and performs the side effect only on ALLOW.
    """

    return _evaluate_file_write(path, content)


if __name__ == "__main__":
    mcp.run(
        transport="streamable-http",
        host=MCP_HOST,
        port=MCP_PORT,
        stateless_http=True,
        json_response=True,
    )
