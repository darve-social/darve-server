#!/bin/bash

set -e

echo "Staging environment detected: inserting test users..."
SCHEMA="http" HOST="localhost" PORT="8080" API_PATH="/api/register" ./init_test_data.sh &

exec /usr/local/bin/app