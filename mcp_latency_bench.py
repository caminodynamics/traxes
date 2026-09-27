#!/usr/bin/env python3
"""Compare direct TRAXES HTTP latency with full MCP round-trip latency.

This benchmark intentionally uses a DENY path so it does not create or modify
the target file. Both paths still exercise TRAXES policy evaluation and artifact
generation. The MCP path additionally includes the Python MCP client/server and
Streamable HTTP transport.

Run while both servers are already running:
  1. TRAXES on http://127.0.0.1:8082
  2. traxes_mcp_adapter.py on http://127.0.0.1:8000/mcp
"""

import argparse
import asyncio
import json
import os
import statistics
import time
import urllib.request
from dataclasses import dataclass

from mcp import Client

TRAXES_SERVER = os.getenv("TRAXES_SERVER_URL", "http://127.0.0.1:8082")
MCP_SERVER = os.getenv("TRAXES_MCP_URL", "http://127.0.0.1:8000/mcp")
DENY_PATH = "temp_executed_mcp_forbidden.txt"
CONTENT = "latency benchmark payload"


@dataclass
class Stats:
    count: int
    mean_ms: float
    p50_ms: float
    p95_ms: float
    p99_ms: float
    min_ms: float
    max_ms: float
    requests_per_second: float


def percentile(values: list[float], percent: float) -> float:
    ordered = sorted(values)
    if not ordered:
        raise ValueError("no samples")
    rank = (len(ordered) - 1) * percent
    low = int(rank)
    high = min(low + 1, len(ordered) - 1)
    fraction = rank - low
    return ordered[low] + (ordered[high] - ordered[low]) * fraction


def summarize(samples_ms: list[float], elapsed_seconds: float) -> Stats:
    return Stats(
        count=len(samples_ms),
        mean_ms=statistics.mean(samples_ms),
        p50_ms=percentile(samples_ms, 0.50),
        p95_ms=percentile(samples_ms, 0.95),
        p99_ms=percentile(samples_ms, 0.99),
        min_ms=min(samples_ms),
        max_ms=max(samples_ms),
        requests_per_second=len(samples_ms) / elapsed_seconds,
    )


def direct_http_once() -> None:
    action = {
        "tool": "FILE_WRITE",
        "session_id": "mcp-latency-direct-http",
        "environment": "sandbox",
        "parameters": {
            "path": DENY_PATH,
            "content": CONTENT,
        },
    }
    request = urllib.request.Request(
        f"{TRAXES_SERVER}/evaluate",
        data=json.dumps(action).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=10) as response:
        result = json.loads(response.read().decode("utf-8"))
    if result.get("decision") != "DENY":
        raise RuntimeError(f"expected DENY from direct HTTP, got {result!r}")


def benchmark_direct_http(warmup: int, requests: int) -> Stats:
    for _ in range(warmup):
        direct_http_once()

    samples = []
    started = time.perf_counter()
    for _ in range(requests):
        t0 = time.perf_counter_ns()
        direct_http_once()
        samples.append((time.perf_counter_ns() - t0) / 1_000_000)
    elapsed = time.perf_counter() - started
    return summarize(samples, elapsed)


async def benchmark_mcp(warmup: int, requests: int) -> Stats:
    async with Client(MCP_SERVER) as client:
        for _ in range(warmup):
            result = await client.call_tool(
                "file_write",
                {"path": DENY_PATH, "content": CONTENT},
            )
            if result.is_error:
                raise RuntimeError(f"MCP warmup failed: {result.content}")

        samples = []
        started = time.perf_counter()
        for _ in range(requests):
            t0 = time.perf_counter_ns()
            result = await client.call_tool(
                "file_write",
                {"path": DENY_PATH, "content": CONTENT},
            )
            samples.append((time.perf_counter_ns() - t0) / 1_000_000)
            if result.is_error:
                raise RuntimeError(f"MCP request failed: {result.content}")
            data = result.structured_content
            if not isinstance(data, dict) or data.get("decision") != "DENY":
                raise RuntimeError(f"expected DENY from MCP, got {data!r}")

        elapsed = time.perf_counter() - started
        return summarize(samples, elapsed)


def print_stats(label: str, stats: Stats) -> None:
    print(label)
    print(f"  samples : {stats.count}")
    print(f"  mean    : {stats.mean_ms:.3f} ms")
    print(f"  p50     : {stats.p50_ms:.3f} ms")
    print(f"  p95     : {stats.p95_ms:.3f} ms")
    print(f"  p99     : {stats.p99_ms:.3f} ms")
    print(f"  min     : {stats.min_ms:.3f} ms")
    print(f"  max     : {stats.max_ms:.3f} ms")
    print(f"  seq rps : {stats.requests_per_second:.1f}")


async def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--requests", type=int, default=200)
    parser.add_argument("--warmup", type=int, default=20)
    args = parser.parse_args()

    if args.requests < 1 or args.warmup < 0:
        raise SystemExit("--requests must be >= 1 and --warmup must be >= 0")

    print("=== TRAXES MCP latency comparison ===")
    print(f"TRAXES : {TRAXES_SERVER}")
    print(f"MCP    : {MCP_SERVER}")
    print(f"mode   : DENY path ({DENY_PATH}); no target write")
    print()

    direct = benchmark_direct_http(args.warmup, args.requests)
    mcp = await benchmark_mcp(args.warmup, args.requests)

    print_stats("Direct TRAXES HTTP", direct)
    print()
    print_stats("Full MCP round trip", mcp)
    print()

    added_p50 = mcp.p50_ms - direct.p50_ms
    ratio = mcp.p50_ms / direct.p50_ms if direct.p50_ms > 0 else float("inf")
    print(f"MCP added p50 : {added_p50:.3f} ms")
    print(f"MCP/direct p50: {ratio:.2f}x")
    print()
    print("Note: this is sequential end-to-end latency, not raw engine-only latency.")
    print("Each request also generates a TRAXES decision artifact.")


if __name__ == "__main__":
    asyncio.run(main())
