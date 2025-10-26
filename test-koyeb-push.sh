#!/bin/bash
# Test pushing metrics to Koyeb server

set -e

echo "🧪 Testing Metrics Push to Koyeb"
echo "=================================="
echo ""

source env/bin/activate

echo "1️⃣  Collecting metrics from claude-monitor..."
METRICS=$(CLAUDE_METRICS_CONFIG='{"hours_back": 24}' python capycoding-app/src-tauri/src/python/collect_metrics.py)
echo "✅ Collected metrics"
echo ""

echo "2️⃣  Pushing to Koyeb server..."
RESPONSE=$(echo "$METRICS" | curl -s -X POST https://cappycoding.koyeb.app/metrics/claude \
  -H "Content-Type: application/json" \
  -d @-)

if [ $? -eq 0 ]; then
  echo "✅ Push successful!"
  echo ""
  echo "Response:"
  echo "$RESPONSE" | python3 -m json.tool
  echo ""
else
  echo "❌ Push failed"
  exit 1
fi

echo "3️⃣  Verifying data on server..."
curl -s https://cappycoding.koyeb.app/metrics/claude | python3 -m json.tool
echo ""

echo "4️⃣  Checking history..."
HISTORY=$(curl -s https://cappycoding.koyeb.app/metrics/claude/history)
COUNT=$(echo "$HISTORY" | python3 -c "import sys, json; print(len(json.load(sys.stdin)))")
echo "History contains: $COUNT entries"
echo ""

echo "✅ All tests passed!"
echo ""
echo "📝 Your Tauri app should use:"
echo "   Server URL: https://cappycoding.koyeb.app"
echo ""
echo "🔧 If the Tauri app still fails:"
echo "   1. Rebuild: cd capycoding-app && npm run tauri build"
echo "   2. Check macOS network permissions"
echo "   3. Try the app again"
