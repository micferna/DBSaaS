#!/bin/sh
set -e

SRC=/etc/unbound/unbound.conf
CONF=/tmp/unbound.conf

# Sanitize environment variables to prevent sed injection
DOMAIN=$(echo "${PLATFORM_DOMAIN:-localhost}" | sed 's/[^a-zA-Z0-9._-]//g')
IP=$(echo "${PLATFORM_IP:-127.0.0.1}" | sed 's/[^0-9.]//g')

# Validate IP format
if ! echo "$IP" | grep -qE '^[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}$'; then
  echo "Invalid PLATFORM_IP: $IP, using 127.0.0.1"
  IP="127.0.0.1"
fi

# Validate domain format
if ! echo "$DOMAIN" | grep -qE '^[a-zA-Z0-9][a-zA-Z0-9._-]*$'; then
  echo "Invalid PLATFORM_DOMAIN: $DOMAIN, using localhost"
  DOMAIN="localhost"
fi

# Copy config to writable location and replace placeholders
cp "$SRC" "$CONF"
sed -i "s/PLATFORM_DOMAIN/${DOMAIN}/g" "$CONF"
sed -i "s/PLATFORM_IP/${IP}/g" "$CONF"

# Validate configuration
unbound-checkconf "$CONF"

# Run unbound in foreground
exec unbound -d -c "$CONF"
