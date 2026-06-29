# ===== Stage 1: build =====
# Full Rust toolchain — used only to compile, then discarded.
FROM rust:1-bookworm AS builder

WORKDIR /app

# Tell SQLx to use the offline .sqlx cache instead of a live database.
ENV SQLX_OFFLINE=true

# Copy the dependency manifests first and fetch deps. Doing this before
# copying source means Docker caches the dependency layer — so editing your
# code later doesn't re-download every crate, only recompiles your code.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Now copy the real source and the offline query cache, and build for real.
COPY src ./src
COPY .sqlx ./.sqlx
RUN cargo build --release

# ===== Stage 2: runtime =====
# Slim image with just enough to run the binary. No compiler, no cargo.
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# The binary needs a couple of runtime libraries (TLS, certs).
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy ONLY the compiled binary out of the builder stage.
COPY --from=builder /app/target/release/timesheet-backend /app/timesheet-backend

EXPOSE 3000
CMD ["/app/timesheet-backend"]