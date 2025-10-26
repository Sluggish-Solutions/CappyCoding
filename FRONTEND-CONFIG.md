# Frontend Configuration Guide

## Overview

The CappyCoding app now includes a built-in UI for configuring and managing the LiveKit voice agent directly from the frontend. No need to manually edit configuration files!

## Features

✅ **Save Configuration** - Store API keys securely in `~/.config/capycoding/agent_config.json`  
✅ **Start/Stop Agent** - Launch and terminate the Python agent from the UI  
✅ **Status Monitoring** - Real-time agent status with PID display  
✅ **Auto-load Config** - Configuration loaded automatically on startup  

## How to Use

### 1. Launch the App

```bash
cd capycoding-app
bun run tauri dev
```

### 2. Configure the Agent

Look for the **"🤖 Voice Agent Configuration"** panel in the UI.

**Required Fields:**
- **LiveKit URL**: Your LiveKit server URL (e.g., `wss://your-project.livekit.cloud`)
- **LiveKit API Key**: Your LiveKit API key
- **LiveKit API Secret**: Your LiveKit API secret
- **Anthropic API Key**: Your Anthropic Claude API key

### 3. Get API Keys

#### LiveKit Cloud
1. Visit [cloud.livekit.io](https://cloud.livekit.io)
2. Sign up/login (free tier available)
3. Create a project
4. Copy the WebSocket URL, API Key, and API Secret

#### Anthropic
1. Visit [console.anthropic.com](https://console.anthropic.com)
2. Sign up/login
3. Add credits to your account
4. Create an API key

### 4. Save and Start

1. Fill in all four fields
2. Click **"💾 Save Configuration"**
3. Click **"▶️ Start Agent"**
4. Wait for status to show "Agent running (PID: xxxxx)"

### 5. Connect to LiveKit

Once the agent is running:

1. Scroll to the **"LiveKit Voice Agent"** section
2. Fill in your connection details:
   - Use the same LiveKit URL and API credentials
   - Choose a unique participant identity
   - Enter a room name
3. Click **"Connect to Voice Session"**
4. Start speaking! The agent will automatically:
   - Transcribe your speech (Deepgram STT)
   - Generate responses (Claude AI)
   - Speak responses back (Cartesia TTS)

## Configuration Storage

Your configuration is saved to:
- **macOS/Linux**: `~/.config/capycoding/agent_config.json`
- **Windows**: `%APPDATA%\capycoding\agent_config.json`

The configuration is automatically loaded when you open the app.

## Agent Management

### Start Agent
- Requires saved configuration
- Launches Python agent in background
- Uses the virtual environment at `/env`

### Stop Agent
- Safely terminates the running agent
- Can be restarted anytime

### Check Status
- Shows if agent is running
- Displays process ID (PID)
- Auto-updates every 5 seconds

## Troubleshooting

### Agent won't start
- Make sure you saved the configuration first
- Check that all API keys are valid
- Verify the virtual environment exists at `/env`
- Try running manually: `cd /path/to/CappyCoding && source env/bin/activate && python agent.py dev`

### Configuration not loading
- Check file permissions on `~/.config/capycoding/`
- Verify the JSON file is valid
- Try saving configuration again

### Agent status shows "not running" but it's running
- Click "Check Status" to refresh
- Agent may take a few seconds to start
- Check terminal output for errors

## Cost Estimates

With LiveKit Inference:
- **Deepgram STT**: $0.0043/minute
- **Cartesia TTS**: $0.045/minute  
- **Claude Sonnet 4-5**: ~$3/$15 per million tokens (input/output)

A typical 1-minute conversation:
- STT: ~$0.004
- TTS: ~$0.045
- Claude: ~$0.05-0.20 (depending on context)
- **Total**: ~$0.10 per minute

## Advanced: Manual Configuration

If you prefer, you can still configure the agent manually:

### Option 1: Environment Variables
```bash
export LIVEKIT_URL="wss://your-project.livekit.cloud"
export LIVEKIT_API_KEY="APIxxxxxxxxxx"
export LIVEKIT_API_SECRET="secret"
export ANTHROPIC_API_KEY="sk-ant-xxxxx"
```

### Option 2: .env File
Create `/Users/akhildatla/GitHub/CappyCoding/.env`:
```env
LIVEKIT_URL=wss://your-project.livekit.cloud
LIVEKIT_API_KEY=APIxxxxxxxxxx
LIVEKIT_API_SECRET=secret
ANTHROPIC_API_KEY=sk-ant-xxxxx
```

The agent loads configuration in this priority:
1. Environment variables (highest)
2. Frontend-saved config (`~/.config/capycoding/agent_config.json`)
3. `.env` file (lowest)

## Next Steps

- Test the voice interaction with different prompts
- Adjust the agent's system prompt in `agent.py` if needed
- Monitor costs via the Claude Usage Metrics panel
- Customize voice settings (voice ID, model, etc.) in `agent.py`

For more details, see [README-AGENT.md](README-AGENT.md) and [QUICKSTART.md](QUICKSTART.md).
