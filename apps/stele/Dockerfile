FROM rust:slim AS builder

RUN apt-get update && apt-get install -y pkg-config && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src/ src/
COPY assets/ assets/

RUN cargo build --release --features headless --no-default-features

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/stele /usr/local/bin/stele

ENV STELE_BIND=0.0.0.0:3100
ENV STELE_DB=/data/stele.db

VOLUME /data
EXPOSE 3100

ENTRYPOINT ["stele"]
