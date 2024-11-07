# Use the official Rust image.
# https://hub.docker.com/_/rust
FROM rust as builder

# Copy local code to the container image.
WORKDIR /usr/src/app
COPY . .

# Install production dependencies and build a release artifact.
RUN cargo build --release

FROM debian:latest
RUN apt-get update && apt-get install -y openssl && apt-get install -y curl #&& rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/server-main /usr/local/bin/server-main
RUN mkdir -p "/usr/local/bin/server_main/src/assets/wasm"

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
ENTRYPOINT ["server-main"]
