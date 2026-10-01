#!/usr/bin/env python3
"""External agent-client proof for the hardened TRAXES FILE_WRITE boundary.

The client never writes or deletes target files directly. It only proposes
FILE_WRITE actions to TRAXES and then reads filesystem/artifact state to verify
what TRAXES actually did. The proof also replays each resulting artifact with
the same policy used by the server.
"""

import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

SERVER = os.getenv("TRAXES_SERVER_URL", "http://127.0.0.1:8082")
POLICY = os.getenv("TRAXES_POLICY", "policies/file_write_agent_policy.yaml")
ALLOW_PATH = Path("temp_executed_agent_allowed.txt")
DENY_PATH = Path("temp_executed_agent_forbidden.txt")
CONTENT = "written only after TRAXES ALLOW"
TRAXES_BIN = Path(
    os.getenv(
        "TRAXES_BIN",
        "target/debug/traxes-demo.exe" if os.name == "nt" else "target/debug/traxes-demo",
    )
)


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
            "Start the TRAXES server first, then run this proof again."
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


def replay(decision_id: str, label: str) -> None:
    if not TRAXES_BIN.exists():
        raise SystemExit(
            f"TRAXES binary not found at {TRAXES_BIN}. Build the project first with `cargo build`."
        )

    completed = subprocess.run(
        [str(TRAXES_BIN), "--dev", "replay", decision_id, "--policy", POLICY],
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        print(completed.stdout)
        print(completed.stderr, file=sys.stderr)
        raise AssertionError(f"{label} replay failed for {decision_id}")
    assert "REPLAY VERIFIED" in completed.stdout, completed.stdout
    print("  replay        : verified")


def main() -> None:
    print("=== TRAXES hardened external-agent FILE_WRITE proof ===")
    print(f"server: {SERVER}")
    print(f"policy: {POLICY}")

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
    assert allow_artifact["execution_outcome"] == "Executed", allow_artifact["execution_outcome"]
    print(
        "  proof         : exists=true, content_match=true, "
        "execution_status=executed, execution_outcome=Executed"
    )
    replay(allow["decision_id"], "ALLOW")

    deny = propose(DENY_PATH, "DENY")
    assert deny["decision"] == "DENY", deny
    assert not DENY_PATH.exists(), "DENY unexpectedly created the target file"
    deny_artifact = wait_for_artifact(deny["artifact_path"])
    assert deny_artifact["execution_status"] == "blocked", deny_artifact["execution_status"]
    assert deny_artifact["execution_outcome"] == "Unauthorized", deny_artifact["execution_outcome"]
    print(
        "  proof         : exists=false, execution_status=blocked, "
        "execution_outcome=Unauthorized"
    )
    replay(deny["decision_id"], "DENY")

    print()
    print("PASS - external client can propose actions, but TRAXES controls execution.")
    print("PASS - ALLOW executed, DENY produced no side effect, and both artifacts replayed.")
    print("This proof is model-agnostic: any agent can propose actions through the same boundary.")


if __name__ == "__main__":
    main()
