# Build context: repo root (docker build -f apps/stele/Dockerfile .)
FROM rust:slim AS builder

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /build/stele
COPY apps/stele/Cargo.toml apps/stele/Cargo.lock* ./
COPY apps/stele/.cargo/ ./.cargo/
COPY apps/stele/crates/ ./crates/
COPY apps/stele/assets/ ./assets/

COPY apps/stylos/Cargo.toml apps/stylos/Cargo.lock* /build/stylos/
COPY apps/stylos/crates/ /build/stylos/crates/

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
