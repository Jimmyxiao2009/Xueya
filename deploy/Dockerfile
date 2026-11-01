FROM node:24.19.0-bookworm-slim AS frontend
WORKDIR /build
COPY package*.json ./
RUN npm ci
COPY tsconfig.json vite.config.ts index.html ./
COPY src ./src
COPY public ./public
RUN npm run build

FROM rust:1.97.1-bookworm AS core
RUN apt-get update && apt-get install -y --no-install-recommends cmake clang && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY src-tauri ./src-tauri
COPY crates ./crates
RUN cargo build --locked --release --manifest-path src-tauri/Cargo.toml --no-default-features --features web-server --bin orbit-server -j 1

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home orbit && mkdir /data && chown orbit /data
COPY --from=core /build/src-tauri/target/release/orbit-server /usr/local/bin/orbit-server
COPY --from=frontend /build/dist /app/dist
USER orbit
ENV ORBIT_BIND=0.0.0.0:1430 ORBIT_DATA_DIR=/data ORBIT_STATIC_DIR=/app/dist ORBIT_REQUIRE_AI=1
EXPOSE 1430
VOLUME /data
ENTRYPOINT ["orbit-server"]
