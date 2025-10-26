# CappyCoding

A Tauri-based desktop application with voice-enabled AI assistant powered by LiveKit and Claude.

## Features

- 🎙️ **Voice AI Assistant**: Talk to Claude using natural speech
- 🔊 **LiveKit Integration**: Professional-grade WebRTC for real-time communication
- 🤖 **Claude Sonnet 4.5**: Advanced AI responses via Anthropic
- 📊 **Usage Metrics**: Track Claude API usage and costs
- 🎨 **Modern UI**: Built with Svelte 5 and Tauri 2.0
- 🔧 **Easy Configuration**: UI-based setup for API keys and agent management

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Tauri Desktop App                      │
│                     (Svelte 5 Frontend)                     │
├─────────────────────────────────────────────────────────────┤
│  🎤 Voice Input  →  LiveKit  →  Python Agent  →  🔊 Voice   │
│                                      ↓                       │
│                            LiveKit Inference                 │
│                       (Deepgram STT + Cartesia TTS)         │
│                                      ↓                       │
│                            Claude AI (Anthropic)             │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- **Node.js 18+** and **bun** (for frontend)
- **Rust** (for Tauri)
- **Python 3.13+** (for voice agent)

### 1. Get API Keys

You'll need:
- **LiveKit Cloud** account: https://cloud.livekit.io/ (free tier available)
- **Anthropic API** key: https://console.anthropic.com/

### 2. Launch the App

```bash
cd capycoding-app
bun install
bun run tauri dev
```

### 3. Configure Voice Agent (in the app)

1. Find the **"🤖 Voice Agent Configuration"** panel
2. Enter your LiveKit credentials (URL, API Key, API Secret)
3. Enter your Anthropic API key
4. Click **"💾 Save Configuration"**
5. Click **"▶️ Start Agent"**

### 4. Start Talking!

1. Go to **"🎙️ Connect to Voice Session"**
2. Enter a participant identity and room name
3. Click **"Connect to Voice Session"**
4. Start speaking naturally!

## Project Structure

```
CappyCoding/
├── agent.py              # Python voice agent (LiveKit + Claude)
├── env/                  # Python virtual environment
├── capycoding-app/       # Tauri desktop application
│   ├── src/              # Svelte frontend
│   └── src-tauri/        # Rust backend
├── capycoding-esp/       # ESP32 firmware (optional)
├── ble-types/            # Bluetooth type definitions
├── server/               # Go metrics server
└── docs/
    ├── FRONTEND-CONFIG.md  # UI configuration guide
    ├── README-AGENT.md     # Agent technical details
    └── QUICKSTART.md       # Quick start guide
```

## Components

### Voice Agent (`agent.py`)

Python agent using LiveKit's agent framework:
- **STT**: Deepgram (via LiveKit Inference)
- **LLM**: Claude Sonnet 4.5 (via Anthropic plugin)
- **TTS**: Cartesia (via LiveKit Inference)
- **VAD**: Silero voice activity detection

Configuration stored in:
- `~/.config/capycoding/agent_config.json` (UI-saved)
- `.env` file (manual configuration)
- Environment variables (highest priority)

### Tauri App (`capycoding-app/`)

Desktop application featuring:
- Voice agent configuration and management
- LiveKit voice session connection
- Claude usage metrics tracking
- Real-time audio streaming

Built with:
- **Frontend**: Svelte 5, Vite, LiveKit Client SDK
- **Backend**: Rust, Tauri 2.0, taurpc for IPC

## Documentation

- **[QUICKSTART.md](QUICKSTART.md)** - Get started in 1 minute
- **[FRONTEND-CONFIG.md](FRONTEND-CONFIG.md)** - UI configuration guide
- **[README-AGENT.md](README-AGENT.md)** - Agent technical details

## Costs

Using LiveKit Inference simplifies billing to just 2 providers:

**LiveKit Cloud** (includes STT + TTS):
- Deepgram STT: $0.0043/minute
- Cartesia TTS: $0.045/minute
- LiveKit transfer: Free tier includes 50 GB/month

**Anthropic**:
- Claude Sonnet 4.5: ~$3/$15 per million tokens

**Typical usage**: ~$0.26/hour of conversation

## Development

### Running the Agent Manually

```bash
cd /path/to/CappyCoding
source env/bin/activate
python agent.py dev
```

### Building the Tauri App

```bash
cd capycoding-app
bun run tauri build
```

### Installing Agent Dependencies

Already installed in `/env`, but to reinstall:

```bash
cd /path/to/CappyCoding
python -m venv env
source env/bin/activate
pip install livekit "livekit-agents[anthropic,silero,deepgram,cartesia]"
```

## Environment Variables

The agent loads configuration in this priority:

1. **Environment variables** (highest)
2. **UI-saved config** (`~/.config/capycoding/agent_config.json`)
3. **.env file** (lowest)

Required variables:
```env
LIVEKIT_URL=wss://your-project.livekit.cloud
LIVEKIT_API_KEY=APIxxxxxxxxxx
LIVEKIT_API_SECRET=xxxxxxxxxxxxxxxx
ANTHROPIC_API_KEY=sk-ant-xxxxxxxxxx
```

## Troubleshooting

### Agent won't start
- Verify API keys are correct (no extra spaces)
- Check that `/env` virtual environment exists
- Try running manually to see error messages

### Can't connect to voice session
- Ensure agent is running (check status in UI)
- Verify LiveKit credentials in agent configuration
- Check that participant identity and room name are filled in

### No audio response
- Check LiveKit Inference is enabled (default on new projects)
- Verify microphone permissions in your OS
- Check browser/Tauri audio settings

See [QUICKSTART.md](QUICKSTART.md) for more troubleshooting tips.

## License

[Your License Here]

## Contributing

[Your Contributing Guidelines Here]
