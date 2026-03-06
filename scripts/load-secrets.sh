#!/usr/bin/env bash
# Source this file to load secrets into env vars for local development
# Usage: source scripts/load-secrets.sh

SECRETS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/secrets"

if [ ! -d "$SECRETS_DIR" ]; then
  echo "No secrets directory found. Run: ./scripts/init-secrets.sh"
  return 1 2>/dev/null || exit 1
fi

export DATABASE_URL=$(cat "$SECRETS_DIR/database_url_local" 2>/dev/null)
export REDIS_URL=$(cat "$SECRETS_DIR/redis_url_local" 2>/dev/null)
export JWT_SECRET=$(cat "$SECRETS_DIR/jwt_secret" 2>/dev/null)
export ENCRYPTION_KEY=$(cat "$SECRETS_DIR/encryption_key" 2>/dev/null)

# Optional Stripe keys (only if non-empty)
STRIPE_SK=$(cat "$SECRETS_DIR/stripe_secret_key" 2>/dev/null)
STRIPE_WH=$(cat "$SECRETS_DIR/stripe_webhook_secret" 2>/dev/null)
[ -n "$STRIPE_SK" ] && export STRIPE_SECRET_KEY="$STRIPE_SK"
[ -n "$STRIPE_WH" ] && export STRIPE_WEBHOOK_SECRET="$STRIPE_WH"

echo "Secrets loaded from $SECRETS_DIR"
