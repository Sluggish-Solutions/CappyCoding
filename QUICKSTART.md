# Quick Start Guide - LiveKit Voice Agent

## What We're Building

A voice-enabled AI assistant that:
- Listens to you speak through LiveKit
- **Transcribes with LiveKit Inference** (Deepgram via LiveKit Cloud)
- Thinks with **Claude Sonnet 4.5** (Anthropic plugin)
- **Responds with LiveKit Inference** (Cartesia via LiveKit Cloud)

**Key Advantage**: LiveKit manages STT/TTS infrastructure - you only need 2 API providers!

## Setup (1 minute with the app!)

### Option 1: Using the Tauri App UI (Recommended)

1. **Launch the app:**
   ```bash
   cd capycoding-app
   bun run tauri dev
   ```

2. **Get API Keys:**
   - **LiveKit**: https://cloud.livekit.io/ (includes STT/TTS via Inference)
   - **Anthropic**: https://console.anthropic.com/

3. **Configure in the app:**
   - Find the "🤖 Voice Agent Configuration" panel
   - Enter your LiveKit URL, API Key, API Secret
   - Enter your Anthropic API Key
   - Click "💾 Save Configuration"
   - Click "▶️ Start Agent"

4. **Connect and talk:**
   - Fill in participant identity and room name
   - Click "Connect to Voice Session"
   - Start speaking naturally!

### Option 2: Manual Setup

If you prefer command-line configuration:

### Option 2: Manual Setup

If you prefer command-line configuration:

#### 1. Configure Environment

Create a `.env` file in the project root:

```bash
cd /Users/akhildatla/GitHub/CappyCoding
cp .env.example .env
nano .env  # or use your favorite editor
```

Fill in:
```env
LIVEKIT_URL=wss://your-project.livekit.cloud
LIVEKIT_API_KEY=APIxxxxxxxxxxxxxxx
LIVEKIT_API_SECRET=xxxxxxxxxxxxxxxxxxxxxxxxxx
ANTHROPIC_API_KEY=sk-ant-xxxxxxxxxxxxx
```

#### 2. Run the Agent Manually

```bash
source env/bin/activate
python agent.py dev
```

You should see:
```
INFO: Agent starting...
INFO: Waiting for job requests...
```

## How To Get API Keys

## How To Get API Keys

### LiveKit Cloud (Includes STT/TTS via Inference)
1. Go to https://cloud.livekit.io/
2. Sign up / Login (free tier available)
3. Create a new project
4. Copy: **URL**, **API Key**, **API Secret**
5. **That's it!** STT/TTS are included via LiveKit Inference

### Claude / Anthropic (Only external API needed)
1. Go to https://console.anthropic.com/
2. Sign up and add credits to your account
3. Get your API key from dashboard

**Note**: No Deepgram or Cartesia keys needed! LiveKit Inference handles both.

## Using the Voice Agent

## Using the Voice Agent

Once the agent is running (started via UI or manually):

1. **In the Tauri app**, find the "🎙️ Connect to Voice Session" section
2. Fill in:
   - **Participant identity**: `my-laptop` (or any unique ID)
   - **Room name**: `my-voice-room` (or any name)
   - **Display name** (optional): Your name
3. Click "Connect to Voice Session"
4. **Start talking naturally!**
   - The agent will greet you
   - Voice activity detection is automatic
   - Just speak - no need to click buttons
   - The agent will respond with voice

## How It Works

```
You speak
   ↓
[LiveKit transmits audio]
   ↓
[LiveKit Inference: Deepgram transcribes]
   ↓
[Agent: Claude processes via Anthropic plugin]
   ↓
[LiveKit Inference: Cartesia TTS generates voice]
   ↓
[LiveKit transmits response]
   ↓
You hear the response
```

## Costs

**LiveKit Inference Pricing** (billed through LiveKit Cloud):
- **Deepgram STT**: $0.0043/minute of audio
- **Cartesia TTS**: $0.045/minute of audio generated
- **LiveKit**: Free tier includes 50 GB transfer/month

**Claude API** (billed separately by Anthropic):
- **Claude Sonnet 4.5**: ~$3/$15 per million tokens (input/output)

### Typical Conversation (1 hour):
- **STT** (6 min speaking): ~$0.026
- **TTS** (3 min responses): ~$0.135  
- **Claude**: ~$0.10 (typical usage)
- **Total: ~$0.26/hour**

**Much simpler billing**: Just 2 providers (LiveKit + Anthropic) instead of 4!

## Troubleshooting

**Agent doesn't start from UI:**
- Make sure you saved the configuration first
- Check that API keys are valid (no extra spaces)
- Try starting manually: `cd /path/to/CappyCoding && source env/bin/activate && python agent.py dev`
- Check the terminal for error messages

**Can't connect to voice session:**
- Verify the agent is running (check status in UI)
- Ensure LiveKit URL starts with `wss://`
- Check that participant identity and room name are filled in
- Try a different room name

**No audio response:**
- Check LiveKit Inference is enabled (it should be by default on new projects)
- Verify you're on a paid LiveKit plan or within free tier limits
- Check browser/Tauri audio permissions

**Configuration not saving:**
- Check file permissions on `~/.config/capycoding/`
- Try creating the directory manually: `mkdir -p ~/.config/capycoding`
- Verify the JSON file is valid after saving

**Agent status shows "not running" but it is:**
- Click "🔄 Check Status" to refresh
- Agent may take a few seconds to fully start
- Check if another agent instance is already running

## Development Tips

- Use the UI to start/stop the agent during development
- Agent configuration is saved and reloaded automatically
- Check agent status every 5 seconds automatically
- Monitor your API usage in LiveKit and Anthropic dashboards
- Test with short phrases first before long conversations

## Next Steps

- Customize the system prompt in `agent.py` (line ~50)
- Adjust VAD sensitivity for your environment
- Try different Cartesia TTS voices (see voice IDs in agent.py)
- Add custom functions/tools for Claude to use
- Monitor costs in your dashboards

## File Locations

- **Agent**: `/Users/akhildatla/GitHub/CappyCoding/agent.py`
- **Virtual Environment**: `/Users/akhildatla/GitHub/CappyCoding/env/`
- **Config** (UI-saved): `~/.config/capycoding/agent_config.json`
- **Config** (manual): `/Users/akhildatla/GitHub/CappyCoding/.env`
- **Documentation**: See `FRONTEND-CONFIG.md` and `README-AGENT.md`
