FROM lukemathwalker/cargo-chef:latest as chef
WORKDIR /app

FROM chef AS planner
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./src ./src
RUN cargo chef prepare

FROM chef AS builder
COPY --from=planner /app/recipe.json .
RUN cargo chef cook --release
COPY ./assets ./assets
COPY ./src ./src
COPY ./templates ./templates
COPY ./tests ./tests
COPY ./Cargo.toml ./Cargo.lock ./
RUN cargo build --release
RUN mv ./target/release/darve_server ./app

FROM debian:stable-slim AS runtime
RUN apt-get update && apt-get install -y openssl && apt-get install -y curl #&& rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
COPY --from=builder /app/app /usr/local/bin/
COPY ./assets /usr/local/bin/assets
#RUN mkdir -p "/usr/local/bin//src/assets/wasm"
RUN mkdir -p "/usr/local/bin/uploads"

# Service must listen to $PORT environment variable.
# This default value facilitates local development.
ENV PORT=8080
ENV DEVELOPMENT=false
ENV START_PASSWORD=oooo
ENV STRIPE_SECRET_KEY=sec
ENV STRIPE_WEBHOOK_SECRET=sdf
ENV JWT_SECRET=jwtt
ENV STRIPE_PLATFORM_ACCOUNT=jwtt
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/app"]
