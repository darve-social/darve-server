set quiet := true
set dotenv-load := true

# Default task: List available tasks in an uncluttered way
[private]
default:
    @just -l -u


test: infra_stop infra_start
    RUST_BACKTRACE=1 cargo test

# Run Rust app in debug mode with backtrace enabled for better error reporting
dev: infra_stop infra_start
    @echo '\n\n🚀 Running backend'
    RUST_BACKTRACE=1 cargo run

# Build the project in release mode and execute the binary
release:
    @echo '\n\n🚀 Building in release mode'
    cargo build --release
    ./target/release/members-registry-server


# Start local infrastructure using Docker Compose
infra_start:
    @echo '\n\n🚀 Starting local infrastructure using a containerized environment'
    docker compose --env-file .env up -d fake-gcs surrealdb sendgrid
    sleep 2
    @echo "🪣 Creating bucket \"${GOOGLE_CLOUD_STORAGE_BUCKET}\" in fake-gcs"
    curl -X POST "${GOOGLE_CLOUD_STORAGE_ENDPOINT}/storage/v1/b" \
      -d "{\"name\":\"${GOOGLE_CLOUD_STORAGE_BUCKET}\"}" \
      -H "Content-Type: application/json"

# Stop local infrastructure and clean up
infra_stop:
    @echo '\n\n🔴 Stopping local infrastructure'
    docker compose --env-file .env down