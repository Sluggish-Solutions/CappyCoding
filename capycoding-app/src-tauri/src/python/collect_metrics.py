import json
import os
import sys
from datetime import datetime, timezone
from typing import Any, Dict


def _load_config() -> Dict[str, Any]:
    raw = os.environ.get("CLAUDE_METRICS_CONFIG", "{}")
    try:
        config = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"invalid CLAUDE_METRICS_CONFIG: {exc}")
    return config


def _load_entries(data_dir: str | None, hours_back: int | None):
    try:
        from claude_monitor.core.models import CostMode
        from claude_monitor.data.reader import load_usage_entries
    except Exception as exc:  # pragma: no cover - executed in external python runtime
        raise SystemExit(
            "claude-monitor package is required. Install with 'pip install claude-monitor'."
        ) from exc

    mode = CostMode.AUTO
    entries, _ = load_usage_entries(data_path=data_dir, hours_back=hours_back, mode=mode)
    return entries


def _serialize_datetime(value: datetime) -> str:
    if value.tzinfo is None:
        value = value.replace(tzinfo=timezone.utc)
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def main() -> None:
    config = _load_config()
    data_dir = config.get("data_dir")
    hours_back = config.get("hours_back")

    try:
        hours_back_int = int(hours_back) if hours_back is not None else None
    except (TypeError, ValueError) as exc:
        raise SystemExit(f"hours_back must be an integer: {exc}") from exc

    entries = _load_entries(data_dir, hours_back_int)
    now = datetime.now(timezone.utc)

    if not entries:
        window_hours = float(hours_back_int or 1)
        result: Dict[str, Any] = {
            "timestamp": _serialize_datetime(now),
            "window_hours": window_hours,
            "burn_rate_per_hour": 0.0,
            "total_cost_usd": 0.0,
            "input_tokens": 0,
            "output_tokens": 0,
            "cache_creation_tokens": 0,
            "cache_read_tokens": 0,
            "total_tokens": 0,
            "session_count": 0,
            "active_session_id": None,
            "last_activity": _serialize_datetime(now),
            "source": "claude-monitor",
        }
        print(json.dumps(result))
        return

    total_cost = 0.0
    input_tokens = 0
    output_tokens = 0
    cache_creation_tokens = 0
    cache_read_tokens = 0
    sessions: set[str] = set()

    for entry in entries:
        total_cost += getattr(entry, "cost_usd", 0.0) or 0.0
        input_tokens += int(getattr(entry, "input_tokens", 0) or 0)
        output_tokens += int(getattr(entry, "output_tokens", 0) or 0)
        cache_creation_tokens += int(getattr(entry, "cache_creation_tokens", 0) or 0)
        cache_read_tokens += int(getattr(entry, "cache_read_tokens", 0) or 0)
        request_id = getattr(entry, "request_id", "") or ""
        if request_id:
            sessions.add(request_id)

    total_tokens = input_tokens + output_tokens + cache_creation_tokens + cache_read_tokens
    first_activity = min(getattr(entry, "timestamp") for entry in entries)
    last_activity = max(getattr(entry, "timestamp") for entry in entries)

    window_hours = float(hours_back_int or 0)
    if window_hours <= 0:
        window_hours = max((last_activity - first_activity).total_seconds() / 3600.0, 0.1)

    burn_rate = total_cost / window_hours if window_hours > 0 else 0.0

    result = {
        "timestamp": _serialize_datetime(now),
        "window_hours": window_hours,
        "burn_rate_per_hour": burn_rate,
        "total_cost_usd": total_cost,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cache_creation_tokens": cache_creation_tokens,
        "cache_read_tokens": cache_read_tokens,
        "total_tokens": total_tokens,
        "session_count": len(sessions) or len(entries),
        "active_session_id": getattr(entries[-1], "request_id", None),
        "last_activity": _serialize_datetime(last_activity),
        "source": "claude-monitor",
    }

    print(json.dumps(result))


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:  # pragma: no cover - runtime error propagation
        print(exc, file=sys.stderr)
        sys.exit(1)
