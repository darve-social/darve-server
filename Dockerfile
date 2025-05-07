# Use the official Rust image.
# https://hub.docker.com/_/rust
FROM rust as builder

# Copy local code to the container image.
WORKDIR /usr/src/app
COPY ./sb_community ./sb_community
COPY ./sb_middleware ./sb_middleware
COPY ./sb_task ./sb_task
COPY ./sb_user_auth ./sb_user_auth
COPY ./sb_wallet ./sb_wallet
COPY ./server_main ./server_main
COPY ./templates ./templates
#COPY ./static ./static
COPY ./Cargo.lock .
COPY ./Cargo.toml .

# Install production dependencies and build a release artifact.
RUN cargo build
#RUN cargo build --release

FROM debian:latest
RUN apt-get update && apt-get install -y openssl && apt-get install -y curl #&& rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/debug/server-main /usr/local/bin/server-main
#COPY --from=builder /usr/src/app/target/release/server-main /usr/local/bin/server-main
RUN mkdir -p "/usr/local/bin/server_main/src/assets/wasm"
COPY --from=builder /usr/src/app/server_main/src/assets /usr/local/bin/server_main/src/assets
#COPY --from=builder /usr/src/app/static /usr/local/bin/static

# Service must listen to $PORT environment variable.
# This default value facilitates local development.
ENV PORT 8080
ENV  DEVELOPMENT false
ENV START_PASSWORD oooo
ENV STRIPE_SECRET_KEY sec
ENV STRIPE_WEBHOOK_SECRET sdf
ENV UPLOADS_DIRECTORY upload
ENV JWT_SECRET jwtt
EXPOSE 8080

# Run the web service on container startup.
#ENTRYPOINT ["bash"]
RUN cd /usr/local/bin
WORKDIR /usr/local/bin
ENTRYPOINT ["server-main"]
