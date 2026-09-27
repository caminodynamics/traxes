#!/usr/bin/env python3
"""External agent-client proof for the TRAXES HTTP execution boundary.

The client never writes or deletes target files directly. It only proposes
FILE_WRITE actions to TRAXES and then reads filesystem/artifact state to verify
what TRAXES actually did.
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

SERVER = os.getenv("TRAXES_SERVER_URL", "http://127.0.0.1:8082")
ALLOW_PATH = Path("temp_executed_agent_allowed.txt")
DENY_PATH = Path("temp_executed_agent_forbidden.txt")
CONTENT = "written only after TRAXES ALLOW"


def propose(path: Path, label: str) -> dict:
    action = {
        "tool": "FILE_WRITE",
        "session_id": "external-agent-client-proof",
        "environment": "sandbox",
        "parameters": {
            "path": path.as_posix(),
            "content": CONTENT,
        },
    }
    request = urllib.request.Request(
        f"{SERVER}/evaluate",
        data=json.dumps(action).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    print(f"[{label}] propose FILE_WRITE -> {path}")
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            result = json.loads(response.read().decode("utf-8"))
    except urllib.error.URLError as exc:
        raise SystemExit(
            f"Could not reach TRAXES at {SERVER}: {exc}\n"
            "Start the server first with the command shown in this script's README output."
        ) from exc

    print(f"  decision      : {result['decision']}")
    print(f"  decision_id   : {result['decision_id']}")
    print(f"  artifact_path : {result['artifact_path']}")
    return result


def wait_for_artifact(path_text: str, timeout_seconds: float = 3.0) -> dict:
    path = Path(path_text)
    deadline = time.time() + timeout_seconds
    while time.time() < deadline:
        if path.exists():
            return json.loads(path.read_text(encoding="utf-8"))
        time.sleep(0.05)
    raise AssertionError(f"artifact did not appear: {path}")


def main() -> None:
    print("=== TRAXES external agent-client FILE_WRITE proof ===")
    print(f"server: {SERVER}")

    if DENY_PATH.exists():
        raise SystemExit(
            f"{DENY_PATH} already exists. Remove it once before running this proof."
        )

    allow = propose(ALLOW_PATH, "ALLOW")
    assert allow["decision"] == "ALLOW", allow
    assert ALLOW_PATH.exists(), "ALLOW did not create the target file"
    assert ALLOW_PATH.read_text(encoding="utf-8") == CONTENT
    allow_artifact = wait_for_artifact(allow["artifact_path"])
    assert allow_artifact["execution_status"] == "executed", allow_artifact["execution_status"]
    print("  proof         : exists=true, content_match=true, execution_status=executed")

    deny = propose(DENY_PATH, "DENY")
    assert deny["decision"] == "DENY", deny
    assert not DENY_PATH.exists(), "DENY unexpectedly created the target file"
    deny_artifact = wait_for_artifact(deny["artifact_path"])
    assert deny_artifact["execution_status"] == "blocked", deny_artifact["execution_status"]
    print("  proof         : exists=false, execution_status=blocked")

    print()
    print("PASS - external client can propose actions, but TRAXES controls execution.")
    print("This proof is model-agnostic: any agent can propose actions through the same boundary.")


if __name__ == "__main__":
    main()
