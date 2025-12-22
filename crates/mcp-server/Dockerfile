# Use the official Rust image as the builder
FROM rust:1.83-slim-bookworm as builder

# Set the working directory
WORKDIR /app

# Copy the entire workspace
COPY . .

# Build the mcp-server
# We need to point to the specific package since it's in a workspace
RUN cargo build --release -p landscape2-mcp-server

# Runtime stage
FROM debian:bookworm-slim

# Install necessary dependencies (e.g., ca-certificates for HTTPS)
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/landscape2-mcp-server /app/landscape2-mcp-server

# Expose the port (if needed, though MCP usually runs over stdio)
# MCP servers typically communicate via Stdio, so no EXPOSE is strictly necessary for that mode.
# However, if we were to add SSE support later, we might need it. For now, we'll leave it out.

# Set the entrypoint
ENTRYPOINT ["/app/landscape2-mcp-server"]
