#!/usr/bin/env bash
#
# setup-docker-tls.sh — Configure Docker daemon with mTLS on a remote server
#
# Usage:
#   ./setup-docker-tls.sh <SERVER_IP> [API_IP]
#
# Arguments:
#   SERVER_IP  — IP address of this Docker server (used in SAN)
#   API_IP     — IP of the API server allowed to connect (for firewall rule)
#                If omitted, firewall rule is skipped.
#
# This script:
#   1. Generates a CA (if not already present)
#   2. Generates server certificate + key (with SAN = SERVER_IP)
#   3. Generates client certificate + key (for the API)
#   4. Configures Docker daemon for TLS on port 2376
#   5. Restarts Docker
#   6. Adds firewall rule (iptables) to restrict port 2376 to API_IP
#   7. Prints client PEM files to paste into the admin panel
#
set -euo pipefail

SERVER_IP="${1:?Usage: $0 <SERVER_IP> [API_IP]}"
API_IP="${2:-}"

TLS_DIR="/etc/docker/tls"
CERT_DAYS=3650

echo "=== Docker mTLS Setup ==="
echo "Server IP: ${SERVER_IP}"
[ -n "$API_IP" ] && echo "API IP (firewall): ${API_IP}"
echo ""

mkdir -p "$TLS_DIR"
cd "$TLS_DIR"

# --------------------------------------------------
# 1. Certificate Authority
# --------------------------------------------------
if [ ! -f ca-key.pem ]; then
    echo ">>> Generating CA..."
    openssl genrsa -out ca-key.pem 4096
    openssl req -new -x509 -days "$CERT_DAYS" -key ca-key.pem -sha256 \
        -out ca.pem -subj "/CN=DBSaaS Docker CA"
    chmod 400 ca-key.pem
    echo "    CA created."
else
    echo ">>> CA already exists, reusing."
fi

# --------------------------------------------------
# 2. Server certificate
# --------------------------------------------------
echo ">>> Generating server certificate..."

cat > server-extfile.cnf <<EOF
subjectAltName = IP:${SERVER_IP},IP:127.0.0.1
extendedKeyUsage = serverAuth
EOF

openssl genrsa -out server-key.pem 4096
openssl req -new -key server-key.pem -sha256 \
    -out server.csr -subj "/CN=${SERVER_IP}"
openssl x509 -req -days "$CERT_DAYS" -sha256 \
    -in server.csr -CA ca.pem -CAkey ca-key.pem -CAcreateserial \
    -out server-cert.pem -extfile server-extfile.cnf

chmod 400 server-key.pem
rm -f server.csr server-extfile.cnf
echo "    Server cert created."

# --------------------------------------------------
# 3. Client certificate
# --------------------------------------------------
echo ">>> Generating client certificate..."

cat > client-extfile.cnf <<EOF
extendedKeyUsage = clientAuth
EOF

openssl genrsa -out client-key.pem 4096
openssl req -new -key client-key.pem -sha256 \
    -out client.csr -subj "/CN=dbsaas-api-client"
openssl x509 -req -days "$CERT_DAYS" -sha256 \
    -in client.csr -CA ca.pem -CAkey ca-key.pem -CAcreateserial \
    -out client-cert.pem -extfile client-extfile.cnf

chmod 444 client-key.pem client-cert.pem
rm -f client.csr client-extfile.cnf
echo "    Client cert created."

# --------------------------------------------------
# 4. Configure Docker daemon
# --------------------------------------------------
echo ">>> Configuring Docker daemon for TLS..."

mkdir -p /etc/systemd/system/docker.service.d

cat > /etc/systemd/system/docker.service.d/override.conf <<EOF
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd \\
    --tlsverify \\
    --tlscacert=${TLS_DIR}/ca.pem \\
    --tlscert=${TLS_DIR}/server-cert.pem \\
    --tlskey=${TLS_DIR}/server-key.pem \\
    -H tcp://0.0.0.0:2376 \\
    -H unix:///var/run/docker.sock
EOF

echo "    Daemon override written."

# --------------------------------------------------
# 5. Restart Docker
# --------------------------------------------------
echo ">>> Restarting Docker..."
systemctl daemon-reload
systemctl restart docker
echo "    Docker restarted."

# --------------------------------------------------
# 6. Firewall (optional)
# --------------------------------------------------
if [ -n "$API_IP" ]; then
    echo ">>> Adding firewall rule..."

    if command -v nft &>/dev/null; then
        # nftables
        echo "    Using nftables..."

        # Create table and chain if they don't exist
        nft add table inet docker-tls 2>/dev/null || true
        nft add chain inet docker-tls input '{ type filter hook input priority 0; }' 2>/dev/null || true

        # Flush existing rules in our chain
        nft flush chain inet docker-tls input

        # Allow only from API IP on port 2376, drop the rest
        nft add rule inet docker-tls input tcp dport 2376 ip saddr "$API_IP" accept
        nft add rule inet docker-tls input tcp dport 2376 drop

        echo "    Port 2376 restricted to ${API_IP} (nftables)."

        # Persist
        if [ -d /etc/nftables.d ]; then
            nft list table inet docker-tls > /etc/nftables.d/docker-tls.nft
            echo "    Rules saved to /etc/nftables.d/docker-tls.nft"
        else
            nft list table inet docker-tls > /etc/nftables-docker-tls.conf
            echo "    Rules saved to /etc/nftables-docker-tls.conf"
            echo "    Add 'include \"/etc/nftables-docker-tls.conf\"' to /etc/nftables.conf to persist."
        fi
    elif command -v iptables &>/dev/null; then
        # iptables fallback
        echo "    Using iptables..."
        iptables -D INPUT -p tcp --dport 2376 -j DROP 2>/dev/null || true
        iptables -D INPUT -p tcp --dport 2376 -s "$API_IP" -j ACCEPT 2>/dev/null || true
        iptables -I INPUT -p tcp --dport 2376 -s "$API_IP" -j ACCEPT
        iptables -A INPUT -p tcp --dport 2376 -j DROP
        echo "    Port 2376 restricted to ${API_IP} (iptables)."

        if command -v netfilter-persistent &>/dev/null; then
            netfilter-persistent save
            echo "    Rules persisted."
        else
            echo "    WARNING: Install iptables-persistent to persist rules across reboots."
        fi
    else
        echo "    WARNING: Neither nft nor iptables found. Add firewall rules manually."
    fi
fi

# --------------------------------------------------
# 7. Print client PEM files
# --------------------------------------------------
echo ""
echo "=============================================="
echo "  Client certificates for the admin panel"
echo "=============================================="
echo ""
echo "--- CA Certificate (tls_ca) ---"
cat "$TLS_DIR/ca.pem"
echo ""
echo "--- Client Certificate (tls_cert) ---"
cat "$TLS_DIR/client-cert.pem"
echo ""
echo "--- Client Key (tls_key) ---"
cat "$TLS_DIR/client-key.pem"
echo ""
echo "=============================================="
echo "Copy the above PEM contents into the admin panel"
echo "when adding this server (URL: tcp://${SERVER_IP}:2376)"
echo "=============================================="
