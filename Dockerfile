FROM rust:latest as builder
WORKDIR /usr/src/axum_wasm_sdb
COPY . .
RUN apt-get update
RUN cargo install --path .

FROM ubuntu:22.04
RUN apt-get update && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/axum_wasm_sdb /usr/local/bin/axum_wasm_sdb
CMD ["axum_wasm_sdb"]
