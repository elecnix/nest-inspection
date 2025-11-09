#!/bin/bash
# Nest MQTT Bridge - Quick Deploy Script

set -e

NEST_IP="192.168.2.113"
NEST_PASS="gtvh4ckr"

echo "=== Nest MQTT Bridge Deployment ==="
echo ""

echo "Step 1: Building ARM binaries..."
cd "$(dirname "$0")"
cargo build --release --target=armv7-unknown-linux-musleabihf

echo ""
echo "Step 2: Transferring binaries to Nest device..."
cd target/armv7-unknown-linux-musleabihf/release
tar -czf - nest-mqtt nest-test | \
  sshpass -p "$NEST_PASS" ssh -o StrictHostKeyChecking=no root@$NEST_IP \
  'cd /root && tar -xzf - && ls -lh nest-mqtt nest-test'

echo ""
echo "Step 3: Running serial connection test..."
sshpass -p "$NEST_PASS" ssh -o StrictHostKeyChecking=no root@$NEST_IP \
  'cd /root && ./nest-test --test serial 2>&1 | tail -10'

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "To run tests on device:"
echo "  ssh root@$NEST_IP 'cd /root && ./nest-test --test all'"
echo ""
echo "To run MQTT bridge (requires Mosquitto):"
echo "  ssh root@$NEST_IP 'cd /root && killall nlclient && ./nest-mqtt --broker localhost:1883'"
echo ""
echo "To monitor MQTT topics:"
echo "  mosquitto_sub -h $NEST_IP -t 'nest/thermostat/#' -v"
echo ""
