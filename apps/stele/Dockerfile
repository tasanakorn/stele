# Build context: repo root (docker build -f apps/stele/Dockerfile .)
FROM rust:slim AS builder

# git + ca-certificates: the `stylos` shared crate is fetched over HTTPS from
# github.com/tasanakorn/stylos at build time (see crates/stele-server/Cargo.toml).
RUN apt-get update && apt-get install -y pkg-config git ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /build/stele
COPY apps/stele/Cargo.toml apps/stele/Cargo.lock* ./
COPY apps/stele/.cargo/ ./.cargo/
COPY apps/stele/crates/ ./crates/
COPY apps/stele/assets/ ./assets/

RUN cargo build --release -p stele-server --features headless --no-default-features

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/stele/target/release/stele-server /usr/local/bin/stele-server

ENV STELE_BIND=0.0.0.0:3100
ENV STELE_DB=/data/stele.db

VOLUME /data
EXPOSE 3100
EXPOSE 31747/tcp
EXPOSE 31747/udp

ENTRYPOINT ["stele-server"]
