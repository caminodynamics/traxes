use crate::workload::ToolCall;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LatencyStats {
    pub count: usize,
    pub total_ns: u128,
    pub average_ns: u128,
    pub p50_ns: u128,
    pub p95_ns: u128,
    pub p99_ns: u128,
    pub throughput_ops_per_sec: f64,
}

pub fn compute_latency_stats(latencies_ns: &[u128]) -> LatencyStats {
    let mut sorted = latencies_ns.to_owned();
    sorted.sort_unstable();

    let count = sorted.len();
    let total_ns: u128 = sorted.iter().copied().sum();
    let average_ns = if count > 0 {
        total_ns / count as u128
    } else {
        0
    };
    let p50_ns = percentile(&sorted, 50);
    let p95_ns = percentile(&sorted, 95);
    let p99_ns = percentile(&sorted, 99);

    let throughput_ops_per_sec = if total_ns > 0 {
        count as f64 / (total_ns as f64 / 1_000_000_000.0)
    } else {
        0.0
    };

    LatencyStats {
        count,
        total_ns,
        average_ns,
        p50_ns,
        p95_ns,
        p99_ns,
        throughput_ops_per_sec,
    }
}

fn percentile(sorted: &[u128], percent: usize) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((percent as f64 / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

pub fn estimate_workload_memory(workload: &[ToolCall]) -> usize {
    let base_size = std::mem::size_of::<ToolCall>() * workload.len();
    let payload_bytes: usize = workload
        .iter()
        .map(|call| {
            let payload_size = call.payload.to_string().len();
            call.tool.capacity() + payload_size
        })
        .sum();

    base_size + payload_bytes
}
