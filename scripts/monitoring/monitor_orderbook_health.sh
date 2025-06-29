#!/bin/bash

# Monitor orderbook service health

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
GRPC_PORT=50052
LOG_FILE="/home/ubuntu/hp_node_stream/orderbook_server.log"
ALERT_THRESHOLD_SECONDS=300  # 5 minutes

echo "=== Orderbook Service Health Monitor ==="
echo "Time: $(date)"
echo ""

# 1. Check if service is running
SERVICE_PID=$(pgrep -f "orderbook-service-realtime")
if [ -z "$SERVICE_PID" ]; then
    echo -e "${RED}❌ Service is NOT running${NC}"
    exit 1
else
    echo -e "${GREEN}✓ Service is running (PID: $SERVICE_PID)${NC}"
fi

# 2. Check current hour file being tailed
CURRENT_HOUR=$(date +%-H)
CURRENT_DATE=$(date +%Y%m%d)
EXPECTED_FILE="/home/hluser/hl/data/node_order_statuses/hourly/${CURRENT_DATE}/${CURRENT_HOUR}"

# Note: Files rotate exactly at :00:00 (previous hour stops at :59:59)

TAIL_PROCESS=$(ps aux | grep "docker exec.*tail.*node_order_statuses" | grep -v grep | tail -1)
if [ -z "$TAIL_PROCESS" ]; then
    echo -e "${RED}❌ No tail process found${NC}"
else
    TAILED_FILE=$(echo "$TAIL_PROCESS" | grep -oP 'hourly/\d+/\d+' | tail -1)
    if [[ "$TAIL_PROCESS" == *"$EXPECTED_FILE"* ]]; then
        echo -e "${GREEN}✓ Tailing correct file: hourly/${TAILED_FILE}${NC}"
    else
        echo -e "${RED}❌ Tailing WRONG file: hourly/${TAILED_FILE} (expected: ${EXPECTED_FILE})${NC}"
    fi
fi

# 3. Check last log entry time
if [ -f "$LOG_FILE" ]; then
    LAST_LOG_TIME=$(tail -1000 "$LOG_FILE" | grep -E "order:|Processing" | tail -1 | grep -oP '\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}' | head -1)
    if [ -n "$LAST_LOG_TIME" ]; then
        # Convert to seconds since epoch
        LAST_LOG_EPOCH=$(date -d "$LAST_LOG_TIME" +%s 2>/dev/null || echo "0")
        CURRENT_EPOCH=$(date +%s)
        SECONDS_SINCE_LAST_LOG=$((CURRENT_EPOCH - LAST_LOG_EPOCH))
        
        if [ $SECONDS_SINCE_LAST_LOG -lt $ALERT_THRESHOLD_SECONDS ]; then
            echo -e "${GREEN}✓ Last order processed: ${SECONDS_SINCE_LAST_LOG} seconds ago${NC}"
        else
            echo -e "${RED}❌ No orders processed for ${SECONDS_SINCE_LAST_LOG} seconds${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ Cannot determine last order time${NC}"
    fi
fi

# 4. Check gRPC port
if netstat -tln | grep -q ":${GRPC_PORT}"; then
    echo -e "${GREEN}✓ gRPC port ${GRPC_PORT} is listening${NC}"
else
    echo -e "${RED}❌ gRPC port ${GRPC_PORT} is NOT listening${NC}"
fi

# 5. Check order rate (last minute)
if [ -f "$LOG_FILE" ]; then
    ORDER_COUNT=$(tail -10000 "$LOG_FILE" | grep -E "order:.*status" | grep -oP '\d{4}-\d{2}-\d{2}T\d{2}:\d{2}' | uniq -c | tail -1 | awk '{print $1}')
    if [ -n "$ORDER_COUNT" ]; then
        echo -e "${GREEN}✓ Order rate: ~${ORDER_COUNT} orders/minute${NC}"
    fi
fi

# 6. Check container data freshness
CONTAINER_FILE_SIZE=$(docker exec hyperliquid-node-1 stat -c%s "$EXPECTED_FILE" 2>/dev/null || echo "0")
if [ "$CONTAINER_FILE_SIZE" -gt 0 ]; then
    echo -e "${GREEN}✓ Current hour file size: $(numfmt --to=iec-i --suffix=B $CONTAINER_FILE_SIZE)${NC}"
else
    echo -e "${RED}❌ Current hour file not found or empty${NC}"
fi

echo ""
echo "=== Recommendations ==="

# Provide recommendations based on health status
if [[ "$TAIL_PROCESS" != *"$EXPECTED_FILE"* ]]; then
    echo "- Service is reading old hour file. Restart required:"
    echo "  sudo systemctl restart orderbook-service"
    echo "  OR"
    echo "  kill $SERVICE_PID && cd /home/ubuntu/hp_node_stream && nohup ./target/debug/orderbook-service-realtime --grpc-port 50052 > orderbook_server.log 2>&1 &"
fi

if [ $SECONDS_SINCE_LAST_LOG -gt $ALERT_THRESHOLD_SECONDS ] 2>/dev/null; then
    echo "- No recent orders detected. Check if HL node is healthy:"
    echo "  docker logs hyperliquid-node-1 --tail 50"
fi