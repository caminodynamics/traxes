import json
import subprocess
import sys
from pathlib import Path

# Example middleware for a tool-calling agent that routes tool payloads to Traxes.
# This simulates how an enterprise agent can invoke Traxes as a pre-execution policy gate.

TRAXES_CLI = Path(__file__).resolve().parents[1] / "target" / "debug" / "Traxes-bench"

payload = {
    "request_id": 12345,
    "tool": "create_ec2_instance",
    "resource": {
        "instance_type": "m5.24xlarge",
        "region": "us-east-1",
        "ami": "ami-0abcdef1234567890",
        "volume_size_gb": 200,
    },
    "cost_estimate_usd_per_hour": 22.75,
    "intent": "provision_compute_for_data_pipeline",
}

# Convert the mock tool payload into a JSON string for Traxes.
payload_json = json.dumps(payload)

try:
    result = subprocess.run(
        [str(TRAXES_CLI), "--mode", "Traxes", "--requests", "1", "--warmup", "0", "--seed", "42", "--scenario", "rogue-infra-agent"],
        input=payload_json.encode("utf-8"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=5,
    )
except FileNotFoundError:
    print("ERROR: Traxes CLI not found. Build the Rust binary before running this example.")
    sys.exit(1)
except subprocess.TimeoutExpired:
    print("ERROR: Traxes policy evaluation timed out.")
    sys.exit(1)

if result.returncode != 0:
    print("ERROR: Traxes CLI failed:")
    print(result.stderr.decode("utf-8", errors="replace"))
    sys.exit(1)

# Attempt to parse Traxes output as JSON or fallback to plain text.
output_text = result.stdout.decode("utf-8", errors="replace").strip()

try:
    response = json.loads(output_text)
except json.JSONDecodeError:
    print("ERROR: Unexpected Traxes response format.")
    print(output_text)
    sys.exit(1)

# The agent middleware expects Traxes to return a structured allow/deny verdict.
verdict = response.get("verdict", "deny").upper()
reason = response.get("reason", "blocked by policy")

if verdict == "DENY":
    print("403 Forbidden: Blocked by Traxes Policy")
    print(f"Reason: {reason}")
    sys.exit(0)

print("Traxes approved the request. Continuing agent execution...")
print(json.dumps(response, indent=2))
