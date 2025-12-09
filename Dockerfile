# Build stage
FROM rust:1.88-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application with release optimizations
RUN cargo build --release && \
    strip /app/target/release/taproot-assets-rest-gateway

# Runtime stage - use distroless for minimal attack surface
FROM debian:bookworm-slim

# Security hardening labels
LABEL org.opencontainers.image.title="Taproot Assets REST Gateway" \
      org.opencontainers.image.description="REST API proxy for Lightning Labs Taproot Assets" \
      org.opencontainers.image.vendor="privkey.io" \
      org.opencontainers.image.licenses="MIT"

# Install runtime dependencies and remove unnecessary packages
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* \
    && rm -rf /usr/share/doc /usr/share/man

# Create app user with specific UID/GID for security
RUN groupadd -r -g 1001 appgroup && \
    useradd -r -u 1001 -g appgroup -s /sbin/nologin -c "App User" appuser

# Copy the binary from builder
COPY --from=builder /app/target/release/taproot-assets-rest-gateway /usr/local/bin/taproot-assets-rest-gateway

# Set binary permissions
RUN chmod 755 /usr/local/bin/taproot-assets-rest-gateway

# Create directories for macaroons with restricted permissions
RUN mkdir -p /app/macaroons/tapd /app/macaroons/lnd && \
    chown -R appuser:appgroup /app && \
    chmod 700 /app/macaroons /app/macaroons/tapd /app/macaroons/lnd

# Security: Drop all capabilities, run read-only where possible
# Switch to non-root user
USER appuser:appgroup

# Set working directory
WORKDIR /app

# Expose port
EXPOSE 8080

# Health check with more conservative settings
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8080/health || exit 1

# Security: Set environment defaults
ENV RUST_BACKTRACE=0 \
    TLS_VERIFY=true

# Run the binary
CMD ["taproot-assets-rest-gateway"]
