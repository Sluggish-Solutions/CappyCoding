# 🚀 Claude Usage Tracker - Quick Start

## ✅ Status: FULLY OPERATIONAL

Everything is installed, configured, and tested.

## �� What You Have

**Current Usage (Last 24 Hours):**
- Total cost: $0.51 USD
- Total tokens: 94,434
- Sessions: 19
- Burn rate: $0.021/hour

## 🎯 3-Step Quick Start

### 1. Start the Go Server

```bash
cd server
go run main.go
```

Server starts on `http://localhost:8080`

### 2. Start the Tauri App

```bash
cd capycoding-app
npm run tauri dev
```

The UI opens automatically.

### 3. Use the UI

1. Navigate to **"Claude Usage Metrics"** section
2. Click **"Collect metrics"** (uses defaults)
3. Click **"Start auto-sync"** for automatic 5-minute updates

Done! 🎉

## 🧪 Test Everything

```bash
./demo-usage-tracker.sh
```

Expected output: All checks pass ✅

## 🎤 Voice Agent (Optional)

```bash
# Start voice agent
python agent.py dev

# Monitor usage in another terminal
tail -f ~/.claude/projects/voice-agent.jsonl
```

## 📍 API Endpoints

- `GET http://localhost:8080/metrics/claude` - Current metrics
- `POST http://localhost:8080/metrics/claude` - Push metrics
- `GET http://localhost:8080/metrics/claude/history` - History

## 🔍 Monitor Real-Time

```bash
# Watch all Claude usage
tail -f ~/.claude/projects/*.jsonl

# Pretty print
tail -f ~/.claude/projects/*.jsonl | jq
```

## 📚 More Info

- **Full docs:** `USAGE-TRACKER-COMPLETE.md`
- **Implementation:** `IMPLEMENTATION-COMPLETE.md`
- **Tests:** `test-complete-tracker.sh`
- **Demo:** `demo-usage-tracker.sh`

---

**Last Updated:** October 26, 2025  
**Status:** Production Ready ✅
