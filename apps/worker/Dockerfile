# Built from the repository root (docker build -f apps/worker/Dockerfile .):
# the whole Cargo workspace is needed, not just this crate, since
# apps/worker depends on the other workspace members by path.
FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p safe-cameroon-worker

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/safe-cameroon-worker /usr/local/bin/safe-cameroon-worker
CMD ["/usr/local/bin/safe-cameroon-worker"]
