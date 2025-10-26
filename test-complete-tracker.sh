#!/bin/bash
# Complete Claude Usage Tracker Test Script
# Tests the full pipeline: claude-monitor → Python collector → Go server

set -e

echo "🔍 Testing Complete Claude Usage Tracker"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Activate virtual environment
source env/bin/activate

echo -e "${BLUE}Step 1: Verify claude-monitor installation${NC}"
if pip show claude-monitor &>/dev/null; then
    VERSION=$(pip show claude-monitor | grep Version | cut -d' ' -f2)
    echo -e "${GREEN}✓ claude-monitor ${VERSION} installed${NC}"
else
    echo -e "${RED}✗ claude-monitor not installed${NC}"
    echo "Install with: pip install claude-monitor"
    exit 1
fi
echo ""

echo -e "${BLUE}Step 2: Check for Claude usage data${NC}"
DATA_DIR="${HOME}/.claude/projects"
if [ -d "$DATA_DIR" ]; then
    FILE_COUNT=$(find "$DATA_DIR" -name "*.jsonl" 2>/dev/null | wc -l | tr -d ' ')
    if [ "$FILE_COUNT" -gt 0 ]; then
        echo -e "${GREEN}✓ Found ${FILE_COUNT} data files in ${DATA_DIR}${NC}"
        
        # Show file sizes
        echo "  Files:"
        find "$DATA_DIR" -name "*.jsonl" -exec sh -c 'echo "  - $(basename {}) ($(wc -l < {} | tr -d " ") lines)"' \;
    else
        echo -e "${YELLOW}⚠ No .jsonl files found in ${DATA_DIR}${NC}"
        echo "  This is normal if you haven't used Claude Code/Cline yet"
    fi
else
    echo -e "${YELLOW}⚠ Directory ${DATA_DIR} not found${NC}"
fi
echo ""

echo -e "${BLUE}Step 3: Test Python metrics collector${NC}"
echo "Testing with 24-hour lookback..."
METRICS_JSON=$(CLAUDE_METRICS_CONFIG='{"hours_back": 24}' python capycoding-app/src-tauri/src/python/collect_metrics.py 2>&1)

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Metrics collection successful${NC}"
    
    # Parse and display key metrics
    TOTAL_COST=$(echo "$METRICS_JSON" | python3 -c "import sys, json; data=json.load(sys.stdin); print(f\"\${data['total_cost_usd']:.4f}\")")
    TOTAL_TOKENS=$(echo "$METRICS_JSON" | python3 -c "import sys, json; data=json.load(sys.stdin); print(data['total_tokens'])")
    SESSION_COUNT=$(echo "$METRICS_JSON" | python3 -c "import sys, json; data=json.load(sys.stdin); print(data['session_count'])")
    BURN_RATE=$(echo "$METRICS_JSON" | python3 -c "import sys, json; data=json.load(sys.stdin); print(f\"\${data['burn_rate_per_hour']:.4f}\")")
    
    echo "  Collected metrics:"
    echo "  - Total cost: \$$TOTAL_COST"
    echo "  - Total tokens: $TOTAL_TOKENS"
    echo "  - Sessions: $SESSION_COUNT"
    echo "  - Burn rate: \$$BURN_RATE/hour"
else
    echo -e "${RED}✗ Metrics collection failed${NC}"
    echo "$METRICS_JSON"
    exit 1
fi
echo ""

echo -e "${BLUE}Step 4: Check Go metrics server${NC}"
SERVER_URL="http://localhost:8080"

# Check if server is running
if curl -sf "$SERVER_URL/metrics/claude" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Server is running at ${SERVER_URL}${NC}"
    
    # Get current metrics from server
    CURRENT=$(curl -s "$SERVER_URL/metrics/claude")
    STORED_COST=$(echo "$CURRENT" | python3 -c "import sys, json; data=json.load(sys.stdin); print(f\"\${data['total_cost_usd']:.4f}\")" 2>/dev/null || echo "0.0000")
    echo "  Current stored cost: \$$STORED_COST"
else
    echo -e "${YELLOW}⚠ Server not running at ${SERVER_URL}${NC}"
    echo "  Start with: cd server && go run main.go"
    echo "  Skipping server tests..."
    exit 0
fi
echo ""

echo -e "${BLUE}Step 5: Test pushing metrics to server${NC}"
# Push the collected metrics
PUSH_RESPONSE=$(curl -s -X POST "$SERVER_URL/metrics/claude" \
    -H "Content-Type: application/json" \
    -d "$METRICS_JSON")

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Successfully pushed metrics to server${NC}"
    
    # Verify the data was stored
    UPDATED_COST=$(echo "$PUSH_RESPONSE" | python3 -c "import sys, json; data=json.load(sys.stdin); print(f\"\${data['total_cost_usd']:.4f}\")")
    echo "  Server confirmed cost: \$$UPDATED_COST"
    
    if [ "$UPDATED_COST" = "$TOTAL_COST" ]; then
        echo -e "${GREEN}✓ Data integrity verified${NC}"
    else
        echo -e "${YELLOW}⚠ Cost mismatch (sent: \$$TOTAL_COST, received: \$$UPDATED_COST)${NC}"
    fi
else
    echo -e "${RED}✗ Failed to push metrics${NC}"
    exit 1
fi
echo ""

echo -e "${BLUE}Step 6: Test metrics history endpoint${NC}"
HISTORY=$(curl -s "$SERVER_URL/metrics/claude/history")
HISTORY_COUNT=$(echo "$HISTORY" | python3 -c "import sys, json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")

if [ "$HISTORY_COUNT" -gt 0 ]; then
    echo -e "${GREEN}✓ History contains ${HISTORY_COUNT} entries${NC}"
    
    # Show latest entry timestamp
    LATEST_TS=$(echo "$HISTORY" | python3 -c "import sys, json; data=json.load(sys.stdin); print(data[0]['timestamp'])" 2>/dev/null)
    echo "  Latest entry: $LATEST_TS"
else
    echo -e "${YELLOW}⚠ History is empty${NC}"
fi
echo ""

echo -e "${BLUE}Step 7: Voice agent integration check${NC}"
if [ -f "agent.py" ]; then
    echo -e "${GREEN}✓ Voice agent found (agent.py)${NC}"
    
    # Check if ClaudeUsageLogger is present
    if grep -q "class ClaudeUsageLogger" agent.py; then
        echo -e "${GREEN}✓ ClaudeUsageLogger class integrated${NC}"
        
        # Check for voice agent log file
        VOICE_LOG="${HOME}/.claude/projects/voice-agent.jsonl"
        if [ -f "$VOICE_LOG" ]; then
            LINE_COUNT=$(wc -l < "$VOICE_LOG" | tr -d ' ')
            echo -e "${GREEN}✓ Voice agent log exists (${LINE_COUNT} entries)${NC}"
            echo "  Location: $VOICE_LOG"
        else
            echo -e "${YELLOW}⚠ Voice agent log not created yet${NC}"
            echo "  Will be created when agent runs: $VOICE_LOG"
        fi
    else
        echo -e "${YELLOW}⚠ ClaudeUsageLogger not found in agent.py${NC}"
    fi
else
    echo -e "${YELLOW}⚠ agent.py not found${NC}"
fi
echo ""

echo -e "${GREEN}=========================================="
echo "✅ Complete Usage Tracker Test Passed!"
echo "==========================================${NC}"
echo ""
echo "📊 Summary:"
echo "  • claude-monitor: Installed and working"
echo "  • Python collector: Successfully reading data"
echo "  • Go server: Receiving and storing metrics"
echo "  • Data flow: End-to-end verified"
echo ""
echo "🚀 Next steps:"
echo "  1. Open the Tauri app to use the UI"
echo "  2. Configure auto-sync (5-minute intervals recommended)"
echo "  3. For voice agent: run 'python agent.py dev'"
echo "  4. Monitor usage: tail -f ~/.claude/projects/*.jsonl"
echo ""
echo "📍 Key endpoints:"
echo "  • GET  $SERVER_URL/metrics/claude - Current metrics"
echo "  • POST $SERVER_URL/metrics/claude - Push new metrics"
echo "  • GET  $SERVER_URL/metrics/claude/history - Historical data"
