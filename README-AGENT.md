# LiveKit Voice Agent Setup

## Quick Start

The LiveKit voice agent is now consolidated at the project root.

### 1. Environment Setup

The agent uses the root `/env` virtual environment with all dependencies installed.

### 2. Configuration Methods

You can configure the agent in **3 ways** (in order of precedence):

#### Option A: Frontend UI (Recommended)
- Open the Tauri app
- Navigate to "LiveKit Voice Agent" section  
- Fill in your API keys
- Click "Save Configuration"
- Click "Start Agent"

Configuration saved to: `~/.config/capycoding/agent_config.json`

#### Option B: .env File
```bash
cd /Users/akhildatla/GitHub/CappyCoding
cp livekit-agent/.env.example .env
# Edit .env with your keys
```

#### Option C: Direct Environment Variables
```bash
export LIVEKIT_URL="wss://your-project.livekit.cloud"
export LIVEKIT_API_KEY="your_key"
export LIVEKIT_API_SECRET="your_secret"
export ANTHROPIC_API_KEY="your_claude_key"
```

### 3. Running the Agent

**From Frontend** (recommended):
- Click "Start Agent" in the Tauri app UI

**From Terminal**:
```bash
cd /Users/akhildatla/GitHub/CappyCoding
source env/bin/activate
python agent.py dev
```

### 4. API Keys Required

| Service | Purpose | Get it from |
|---------|---------|-------------|
| **LiveKit** | Room management + STT/TTS via Inference | https://cloud.livekit.io/ |
| **Anthropic** | Claude AI (LLM) | https://console.anthropic.com/ |

That's it! Just 2 API keys needed.

## How It Works

```
User → [Tauri App] ← LiveKit WebRTC → [Python Agent]
                                            ↓
                    LiveKit Inference (Deepgram STT)
                                            ↓
                        Anthropic Claude (LLM)
                                            ↓
                    LiveKit Inference (Cartesia TTS)
```

## Features

- ✅ **Automatic speech detection** - No button clicks needed
- ✅ **Natural conversation** - Claude Sonnet 4.5 for intelligence
- ✅ **Professional quality** - Deepgram STT + Cartesia TTS
- ✅ **Simple setup** - Configure from frontend UI
- ✅ **One venv** - All dependencies in `/env`

## Costs

- **LiveKit Inference STT**: $0.0043/min
- **LiveKit Inference TTS**: $0.045/min  
- **Claude Sonnet 4.5**: ~$3/$15 per million tokens (input/output)

**Typical usage**: ~$0.27/hour of conversation

## Troubleshooting

**Agent won't start**:
- Check API keys are set (via UI or .env)
- Verify venv is activated: `source env/bin/activate`
- Check dependencies: `pip list | grep livekit`

**No transcription**:
- Verify LiveKit Inference is enabled (should be automatic)
- Check LiveKit dashboard for errors

**Can't hear responses**:
- Check Tauri app has audio permissions
- Verify agent connected (see terminal output)
- Check browser/system audio settings

## Files

- `/agent.py` - Main agent code
- `/env/` - Consolidated Python virtual environment
- `/livekit-agent/` - Documentation and examples (can be removed if not needed)
- `~/.config/capycoding/agent_config.json` - Frontend-saved configuration

## Next Steps

1. Get your API keys from LiveKit and Anthropic
2. Configure them in the Tauri app UI
3. Click "Start Agent"
4. Connect to Live Kit from the app
5. Start talking!

For detailed documentation, see `/livekit-agent/QUICKSTART.md`
