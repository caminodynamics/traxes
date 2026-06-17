FROM rust:1.75-slim as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/traxes-demo /app/traxes-demo
COPY --from=builder /app/target/release/traxes-bench /app/traxes-bench

# Copy demo payloads
COPY ../demo/payloads/ ./payloads/

# Create artifacts directory
RUN mkdir -p artifacts

# Set environment
ENV RUST_LOG=info

# Default command
CMD ["./traxes-demo", "demo"]
