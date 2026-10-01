#!/usr/bin/env python3
"""Measure persistent-connection HTTP latency through the TRAXES execution boundary.

Run the TRAXES server separately in demo mode so presentation sleeps are disabled:

    cargo run -- --dev --demo-mode server --policy policies/file_write_agent_policy.yaml

Then run:

    python scripts/http_latency.py

The measurement includes HTTP request/response, JSON handling, policy evaluation,
permit enforcement, FILE_WRITE execution, coverage bookkeeping, and enqueueing the
artifact record. The HTTP server writes artifacts asynchronously, so durable artifact
I/O can complete after the response and is intentionally not counted here.
"""

import argparse
import http.client
import json
import statistics
import time
from pathlib import Path
from urllib.parse import urlparse

DEFAULT_URL = "http://127.0.0.1:8082/evaluate"
ALLOW_PATH = "temp_executed_agent_allowed.txt"
CONTENT = "http latency benchmark"


def percentile(samples_ns: list[int], q: float) -> float:
    ordered = sorted(samples_ns)
    idx = round((len(ordered) - 1) * q)
    return ordered[idx] / 1_000.0


def request_once(conn: http.client.HTTPConnection, endpoint: str, body: bytes) -> dict:
    conn.request(
        "POST",
        endpoint,
        body=body,
        headers={
            "Content-Type": "application/json",
            "Content-Length": str(len(body)),
        },
    )
    response = conn.getresponse()
    payload = response.read()
    if response.status != 200:
        raise RuntimeError(f"HTTP {response.status}: {payload.decode('utf-8', errors='replace')}")
    result = json.loads(payload)
    if result.get("decision") != "ALLOW":
        raise RuntimeError(f"expected ALLOW, got: {result}")
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default=DEFAULT_URL)
    parser.add_argument("--iterations", type=int, default=500)
    parser.add_argument("--warmup", type=int, default=25)
    args = parser.parse_args()

    parsed = urlparse(args.url)
    if parsed.scheme != "http":
        raise SystemExit("This benchmark currently expects an http:// URL")

    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 80
    endpoint = parsed.path or "/evaluate"

    action = {
        "tool": "FILE_WRITE",
        "session_id": "http-latency-benchmark",
        "environment": "sandbox",
        "parameters": {
            "path": ALLOW_PATH,
            "content": CONTENT,
        },
    }
    body = json.dumps(action, separators=(",", ":")).encode("utf-8")

    conn = http.client.HTTPConnection(host, port, timeout=10)

    for _ in range(args.warmup):
        request_once(conn, endpoint, body)

    samples_ns: list[int] = []
    wall_start = time.perf_counter_ns()
    for _ in range(args.iterations):
        start = time.perf_counter_ns()
        request_once(conn, endpoint, body)
        samples_ns.append(time.perf_counter_ns() - start)
    wall_ns = time.perf_counter_ns() - wall_start
    conn.close()

    target = Path(ALLOW_PATH)
    if not target.exists():
        raise RuntimeError("ALLOW benchmark did not create the target file")
    if target.read_text(encoding="utf-8") != CONTENT:
        raise RuntimeError("target file content mismatch")

    avg_us = statistics.fmean(samples_ns) / 1_000.0
    min_us = min(samples_ns) / 1_000.0
    max_us = max(samples_ns) / 1_000.0
    throughput = args.iterations / (wall_ns / 1_000_000_000)

    print("TRAXES HTTP execution-boundary latency")
    print(f"  samples    : {args.iterations}")
    print(f"  p50        : {percentile(samples_ns, 0.50):.3f} us")
    print(f"  p95        : {percentile(samples_ns, 0.95):.3f} us")
    print(f"  p99        : {percentile(samples_ns, 0.99):.3f} us")
    print(f"  avg        : {avg_us:.3f} us")
    print(f"  min / max  : {min_us:.3f} / {max_us:.3f} us")
    print(f"  throughput : {throughput:.1f} calls/sec")
    print("  includes   : HTTP + JSON + policy + permit + FILE_WRITE + coverage + artifact enqueue")
    print("  excludes   : asynchronous durable artifact write after response")
    print("  note       : run server with --demo-mode; normal mode has intentional presentation sleeps")


if __name__ == "__main__":
    main()
