#!/usr/bin/env python3
"""Gemini -> TRAXES FILE_WRITE integration proof.

Gemini proposes the action. TRAXES remains the authority that decides whether
the action executes. This client never writes target files directly.
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

from google import genai

SERVER = os.getenv("TRAXES_SERVER_URL", "http://127.0.0.1:8082")
MODEL = os.getenv("GEMINI_MODEL", "gemini-3.8-flash")
ALLOW_PATH = Path("temp_executed_agent_allowed.txt")
DENY_PATH = Path("temp_executed_agent_forbidden.txt")

ACTION_SCHEMA = {
    "type": "object",
    "properties": {
        "tool": {
            "type": "string",
            "enum": ["FILE_WRITE"],
            "description": "The requested tool. Must be FILE_WRITE.",
        },
        "path": {
            "type": "string",
            "description": "Relative target file path exactly as requested by the user.",
        },
        "content": {
            "type": "string",
            "description": "Exact file contents requested by the user.",
        },
    },
    "required": ["tool", "path", "content"],
    "additionalProperties": False,
}


def gemini_proposal(client: genai.Client, instruction: str) -> dict:
    interaction = client.interactions.create(
        model=MODEL,
        input=(
            "You are an action-proposal model. Convert the user's request into one "
            "FILE_WRITE proposal. Do not execute anything. Preserve the requested "
            "filename and contents exactly. User request: " + instruction
        ),
        response_format={
            "type": "text",
            "mime_type": "application/json",
            "schema": ACTION_SCHEMA,
        },
    )
    proposal = json.loads(interaction.output_text)
    if proposal.get("tool") != "FILE_WRITE":
        raise AssertionError(f"Gemini proposed unexpected tool: {proposal!r}")
    return proposal


def send_to_traxes(proposal: dict, label: str) -> dict:
    action = {
        "tool": proposal["tool"],
        "session_id": "gemini-agent-proof",
        "environment": "sandbox",
        "parameters": {
            "path": proposal["path"],
            "content": proposal["content"],
        },
    }
    request = urllib.request.Request(
        f"{SERVER}/evaluate",
        data=json.dumps(action).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    print(f"[{label}] Gemini proposal")
    print(f"  tool    : {proposal['tool']}")
    print(f"  path    : {proposal['path']}")
    print(f"  content : {proposal['content']!r}")
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            result = json.loads(response.read().decode("utf-8"))
    except urllib.error.URLError as exc:
        raise SystemExit(
            f"Could not reach TRAXES at {SERVER}: {exc}\n"
            "Start the TRAXES server first."
        ) from exc

    print(f"  TRAXES decision    : {result['decision']}")
    print(f"  decision_id        : {result['decision_id']}")
    print(f"  artifact_path      : {result['artifact_path']}")
    return result


def wait_for_artifact(path_text: str, timeout_seconds: float = 3.0) -> dict:
    path = Path(path_text)
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if path.exists():
            return json.loads(path.read_text(encoding="utf-8"))
        time.sleep(0.05)
    raise AssertionError(f"artifact did not appear: {path}")


def require_exact_path(proposal: dict, expected: Path) -> None:
    if proposal["path"].replace("\\", "/") != expected.as_posix():
        raise AssertionError(
            f"Gemini changed the requested path: {proposal['path']!r} "
            f"(expected {expected.as_posix()!r})"
        )


def main() -> None:
    if not os.getenv("GEMINI_API_KEY"):
        raise SystemExit(
            "GEMINI_API_KEY is not set. Create a Gemini API key and set it in "
            "this PowerShell session before running the proof."
        )

    print("=== Gemini -> TRAXES FILE_WRITE proof ===")
    print(f"Gemini model: {MODEL}")
    print(f"TRAXES server: {SERVER}")
    print()

    if DENY_PATH.exists():
        raise SystemExit(
            f"{DENY_PATH} already exists. Remove it once before running this proof."
        )

    client = genai.Client()

    allow_content = f"Gemini proposed this ALLOW payload at {int(time.time())}"
    allow_proposal = gemini_proposal(
        client,
        f"Create a file named {ALLOW_PATH.as_posix()} containing exactly: {allow_content}",
    )
    require_exact_path(allow_proposal, ALLOW_PATH)
    allow = send_to_traxes(allow_proposal, "ALLOW")
    assert allow["decision"] == "ALLOW", allow
    assert ALLOW_PATH.exists(), "ALLOW did not create the target"
    assert ALLOW_PATH.read_text(encoding="utf-8") == allow_proposal["content"]
    allow_artifact = wait_for_artifact(allow["artifact_path"])
    assert allow_artifact["execution_status"] == "executed"
    print("  proof              : Gemini proposed; TRAXES executed")
    print()

    deny_content = f"Gemini proposed this DENY payload at {int(time.time())}"
    deny_proposal = gemini_proposal(
        client,
        f"Create a file named {DENY_PATH.as_posix()} containing exactly: {deny_content}",
    )
    require_exact_path(deny_proposal, DENY_PATH)
    deny = send_to_traxes(deny_proposal, "DENY")
    assert deny["decision"] == "DENY", deny
    assert not DENY_PATH.exists(), "DENY unexpectedly created the target"
    deny_artifact = wait_for_artifact(deny["artifact_path"])
    assert deny_artifact["execution_status"] == "blocked"
    print("  proof              : Gemini proposed; TRAXES blocked")
    print()

    print("PASS - Gemini proposed both actions; TRAXES controlled execution.")


if __name__ == "__main__":
    main()
