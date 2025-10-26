"""
LiveKit Voice Agent using LiveKit Inference and Anthropic Claude
With codebase awareness tools
"""

import os
import json
import sys
import subprocess
from pathlib import Path
from dotenv import load_dotenv

from livekit.agents import AutoSubscribe, JobContext, WorkerOptions, cli, function_tool
from livekit.agents.voice import Agent as VoiceAgent, AgentSession
from livekit.plugins import anthropic, silero

# Load environment variables from .env file
load_dotenv()

# Determine config path based on OS
if sys.platform == "darwin":  # macOS
    config_path = Path.home() / "Library" / "Application Support" / "capycoding" / "agent_config.json"
else:  # Linux/Windows
    config_path = Path.home() / ".config" / "capycoding" / "agent_config.json"

# Get the workspace path (where the agent is running from)
WORKSPACE_PATH = Path.cwd()

# Load from Tauri config file if exists
if config_path.exists():
    try:
        with open(config_path) as f:
            config = json.load(f)
            # Set environment variables from config file if not already set
            for key, env_var in [
                ("livekit_url", "LIVEKIT_URL"),
                ("livekit_api_key", "LIVEKIT_API_KEY"),
                ("livekit_api_secret", "LIVEKIT_API_SECRET"),
                ("anthropic_api_key", "ANTHROPIC_API_KEY"),
            ]:
                if key in config and not os.getenv(env_var):
                    os.environ[env_var] = config[key]
            
            # Override workspace path if specified in config
            if "codebase_path" in config and config["codebase_path"]:
                WORKSPACE_PATH = Path(config["codebase_path"])
                print(f"Using codebase path from config: {WORKSPACE_PATH}")
    except Exception as e:
        print(f"Warning: Could not load config file: {e}")

# Create custom agent class with tools
class CodebaseAgent(VoiceAgent):
    """Voice agent with codebase awareness tools"""
    
    @function_tool()
    async def read_file(self, file_path: str) -> str:
        """
        Read the contents of a file in the workspace.
        
        Args:
            file_path: Relative path to the file from workspace root
        """
        try:
            full_path = WORKSPACE_PATH / file_path
            if not full_path.exists():
                return f"Error: File '{file_path}' not found"
            
            if not full_path.is_file():
                return f"Error: '{file_path}' is not a file"
            
            # Read the file
            with open(full_path, 'r', encoding='utf-8') as f:
                content = f.read()
            
            # Limit to first 500 lines to avoid overwhelming the context
            lines = content.split('\n')
            if len(lines) > 500:
                content = '\n'.join(lines[:500]) + f"\n... (truncated, {len(lines) - 500} more lines)"
            
            return f"Contents of {file_path}:\n\n{content}"
        except Exception as e:
            return f"Error reading file: {str(e)}"
    
    @function_tool()
    async def search_code(self, query: str) -> str:
        """
        Search for text in all code files in the workspace using grep.
        
        Args:
            query: Text to search for in the codebase
        """
        try:
            # Use grep to search recursively
            result = subprocess.run(
                ['grep', '-r', '-n', '-i', '--include=*.py', '--include=*.rs', '--include=*.svelte', 
                 '--include=*.ts', '--include=*.js', '--include=*.json', query, str(WORKSPACE_PATH)],
                capture_output=True,
                text=True,
                timeout=5
            )
            
            if result.returncode == 0 and result.stdout:
                # Limit results to first 50 lines
                lines = result.stdout.strip().split('\n')
                if len(lines) > 50:
                    output = '\n'.join(lines[:50]) + f"\n... ({len(lines) - 50} more matches)"
                else:
                    output = '\n'.join(lines)
                return f"Search results for '{query}':\n\n{output}"
            else:
                return f"No matches found for '{query}'"
        except subprocess.TimeoutExpired:
            return "Search timed out"
        except Exception as e:
            return f"Error searching: {str(e)}"
    
    @function_tool()
    async def list_files(self, directory: str = ".") -> str:
        """
        List files and directories in a given path.
        
        Args:
            directory: Directory path relative to workspace root, or '.' for root
        """
        try:
            full_path = WORKSPACE_PATH / directory
            if not full_path.exists():
                return f"Error: Directory '{directory}' not found"
            
            if not full_path.is_dir():
                return f"Error: '{directory}' is not a directory"
            
            # List contents, excluding common ignore patterns
            ignore_patterns = {'.git', '__pycache__', 'node_modules', 'target', 'dist', 'build', '.venv', 'env'}
            items = []
            
            for item in sorted(full_path.iterdir()):
                if item.name not in ignore_patterns and not item.name.startswith('.'):
                    if item.is_dir():
                        items.append(f"📁 {item.name}/")
                    else:
                        items.append(f"📄 {item.name}")
            
            if not items:
                return f"Directory '{directory}' is empty"
            
            return f"Contents of {directory}:\n\n" + "\n".join(items)
        except Exception as e:
            return f"Error listing directory: {str(e)}"
    
    @function_tool()
    async def get_project_info(self) -> str:
        """Get overview information about the project structure and available commands."""
        # Try to identify project type
        project_type = "Unknown"
        frameworks = []
        
        if (WORKSPACE_PATH / "package.json").exists():
            frameworks.append("Node.js/npm")
        if (WORKSPACE_PATH / "Cargo.toml").exists():
            frameworks.append("Rust")
        if (WORKSPACE_PATH / "pyproject.toml").exists() or (WORKSPACE_PATH / "setup.py").exists():
            frameworks.append("Python")
        if (WORKSPACE_PATH / "go.mod").exists():
            frameworks.append("Go")
        if (WORKSPACE_PATH / ".git").exists():
            frameworks.append("Git repository")
        
        info = f"""Project Workspace
Location: {WORKSPACE_PATH}
Detected: {', '.join(frameworks) if frameworks else 'Generic project'}

I can help you understand and work with this codebase:
- read_file(path): Read any file in the project
- search_code(query): Search for text across all code files
- list_files(directory): List files in any directory (use "." for root)

Ask me to read specific files, search for functions, or explore the project structure!
"""
        return info

async def entrypoint(ctx: JobContext):
    await ctx.connect(auto_subscribe=AutoSubscribe.AUDIO_ONLY)
    
    print("✅ Connected to room")
    
    # Create agent with codebase tools
    assistant = CodebaseAgent(
        instructions="""You are an expert AI coding assistant with access to the user's codebase.

Available tools:
- read_file(path): Read any file's contents
- search_code(query): Search for code patterns across files
- list_files(directory): List files in a directory
- get_project_info(): Get workspace info and detected frameworks

Workflow:
1. Listen carefully to what the user wants to know
2. Use tools to examine relevant files and code
3. Provide clear, concise explanations with specific file paths
4. Suggest improvements and explain how code works

Keep responses brief and conversational. When discussing code, always mention the specific file path.
If you need to see a file's contents to answer, use read_file(). If searching for something, use search_code().""",
        llm=anthropic.LLM(model="claude-sonnet-4-5", temperature=0.7),
        vad=silero.VAD.load(),
        stt="deepgram/nova-2:en",
        tts="cartesia/sonic-2:9626c31c-bec5-4cca-baa8-f8ba9e84c8bc",
    )
    
    session = AgentSession(
        llm=anthropic.LLM(model="claude-sonnet-4-5", temperature=0.7),
        vad=silero.VAD.load(),
        stt="deepgram/nova-2:en",
        tts="cartesia/sonic-2:9626c31c-bec5-4cca-baa8-f8ba9e84c8bc",
    )
    
    await session.start(assistant, room=ctx.room)
    
    print("✅ Agent started with codebase tools enabled!")
    print(f"   Workspace: {WORKSPACE_PATH}")
    print(f"   Tools: read_file, search_code, list_files, get_project_info")
    print(f"   STT: Deepgram Nova-2 | TTS: Cartesia Sonic | LLM: Claude Sonnet 4-5")


if __name__ == "__main__":
    cli.run_app(WorkerOptions(entrypoint_fnc=entrypoint))
