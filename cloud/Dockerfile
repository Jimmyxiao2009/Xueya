# syntax=docker/dockerfile:1.7

FROM rust:1.97-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY migrations ./migrations

RUN cargo build --release --locked --workspace --bins

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home --shell /usr/sbin/nologin verruckt

COPY --from=builder /app/target/release/verruckt-cloud /usr/local/bin/verruckt-cloud
COPY --from=builder /app/target/release/cloud-admin /usr/local/bin/cloud-admin

USER verruckt
EXPOSE 8080

ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/verruckt-cloud"]
