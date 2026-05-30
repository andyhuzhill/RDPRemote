# RDPRemote Docker Image
# Multi-stage build for optimized release binaries

# Stage 1: Build environment
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev

# Set working directory
WORKDIR /build

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./
COPY common/common/Cargo.toml ./common/
COPY server/server/Cargo.toml ./server/
COPY agent/agent/Cargo.toml ./agent/
COPY client/client/Cargo.toml ./client/

# Create dummy source files to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY common/src ./common/src/
COPY server/src ./server/src/
COPY agent/src ./agent/src/
COPY client/src ./client/src/

# Build all binaries
RUN cargo build --release --bin rdp-server --bin rdp-client

# Stage 2: Runtime environment
FROM alpine:3.19 AS runtime

# Install runtime dependencies
RUN apk add --no-cache ca-certificates openssl

# Create non-root user
RUN adduser -D -g '' appuser

# Create directories
RUN mkdir -p /app/data && \
    chown -R appuser:appuser /app

# Copy binaries from builder
COPY --from=builder /build/target/release/rdp-server /app/
COPY --from=builder /build/target/release/rdp-client /app/

# Set ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Set working directory
WORKDIR /app

# Default command (can be overridden)
CMD ["./rdp-server"]

# Expose signaling server port
EXPOSE 8765

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8765/ || exit 1

# Labels
LABEL maintainer="RDPRemote Team"
LABEL version="1.0.0"
LABEL description="RDPRemote - Cross-platform remote desktop control system"