#!/usr/bin/env bash
set -euo pipefail

# Generate Docker secrets files for DBSaaS platform
# Usage: ./scripts/init-secrets.sh [--force]

SECRETS_DIR="$(cd "$(dirname "$0")/.." && pwd)/secrets"
FORCE="${1:-}"

if [ -d "$SECRETS_DIR" ] && [ "$FORCE" != "--force" ]; then
  echo "Secrets directory already exists: $SECRETS_DIR"
  echo "Use --force to regenerate (WARNING: will break existing databases!)"
  exit 1
fi

mkdir -p "$SECRETS_DIR"
chmod 700 "$SECRETS_DIR"

gen_password() {
  openssl rand -base64 32 | tr -d '/+=' | head -c 32
}

gen_hex() {
  openssl rand -hex "$1"
}

echo "Generating secrets in $SECRETS_DIR ..."

# PostgreSQL
echo -n "dbsaas" > "$SECRETS_DIR/postgres_user"
gen_password > "$SECRETS_DIR/postgres_password"
echo -n "dbsaas_platform" > "$SECRETS_DIR/postgres_db"

# Redis
gen_password > "$SECRETS_DIR/redis_password"

# JWT
gen_password > "$SECRETS_DIR/jwt_secret"

# Encryption key (32 bytes = 64 hex chars)
gen_hex 32 > "$SECRETS_DIR/encryption_key"

# Stripe (empty by default, fill manually)
touch "$SECRETS_DIR/stripe_secret_key"
touch "$SECRETS_DIR/stripe_webhook_secret"

# Set restrictive permissions
chmod 600 "$SECRETS_DIR"/*

# Build DATABASE_URL and REDIS_URL from components
PG_USER=$(cat "$SECRETS_DIR/postgres_user")
PG_PASS=$(cat "$SECRETS_DIR/postgres_password")
PG_DB=$(cat "$SECRETS_DIR/postgres_db")
REDIS_PASS=$(cat "$SECRETS_DIR/redis_password")

echo -n "postgres://${PG_USER}:${PG_PASS}@postgres:5432/${PG_DB}" > "$SECRETS_DIR/database_url"
echo -n "redis://:${REDIS_PASS}@redis:6379" > "$SECRETS_DIR/redis_url"

# Also create a local variant for dev (localhost instead of Docker hostname)
echo -n "postgres://${PG_USER}:${PG_PASS}@localhost:5432/${PG_DB}" > "$SECRETS_DIR/database_url_local"
echo -n "redis://:${REDIS_PASS}@localhost:6379" > "$SECRETS_DIR/redis_url_local"

chmod 600 "$SECRETS_DIR"/*

echo ""
echo "Secrets generated in: $SECRETS_DIR"
echo ""
echo "Files created:"
ls -la "$SECRETS_DIR"
echo ""
echo "For local development, export these:"
echo "  export DATABASE_URL=\$(cat $SECRETS_DIR/database_url_local)"
echo "  export REDIS_URL=\$(cat $SECRETS_DIR/redis_url_local)"
echo "  export JWT_SECRET=\$(cat $SECRETS_DIR/jwt_secret)"
echo "  export ENCRYPTION_KEY=\$(cat $SECRETS_DIR/encryption_key)"
echo ""
echo "For Docker Compose, just run: docker compose up -d"
