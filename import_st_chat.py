#!/usr/bin/env python3
"""Convert a SillyTavern JSONL chat file to Epibloom chat JSON format.

Usage:
    python convert_st_chat.py <input.jsonl>

The converted chat is written directly to:
    ~/.local/share/epibloom/data/chats/<chat-id>.json

---

SillyTavern structure
---------------------
The JSONL has one JSON object per line:
  - Line 1: chat_metadata  (skipped)
  - Line 2+: message objects, in linear order

Each message object has:
  - is_user (bool)          true → user, false → assistant
  - mes (str)               the selected swipe's content
  - swipes (str[])          all swipe variants at this position
  - swipe_id (int)          index of the currently selected swipe
  - swipe_info (obj[])      per-swipe metadata (timestamps, extra)
  - extra.reasoning (str)   reasoning/thinking text for the selected swipe
  - send_date (str)         ISO 8601 timestamp

Epibloom structure
------------------
A chat is a JSON file containing a tree of message nodes:

  {
    "id": "<uuid7>",
    "messages": {
      "<uuid7>": {
        "id":        "<uuid7>",
        "parent_id": "<uuid7> | null",    -- null for root nodes
        "role":      "user" | "assistant",
        "content":   ["<text>"],          -- array of chunks; one element for imports
        "thinking":  ["<text>"],          -- optional; reasoning text
      }
    }
  }

Swipe → tree mapping
--------------------
SillyTavern's swipes represent alternative branches at a given turn. In the
epibloom tree they are siblings: each swipe at position N shares the same
parent_id (the *selected* swipe from position N-1). The selected swipe at
position N becomes the parent for all messages at position N+1.

                parent=null
               ┌──────────────┐
               │  msg0[sw=0]  │  ← selected; parent for msg1
               └──────────────┘
               ┌──────────────┐
               │  msg0[sw=1]  │  ← sibling; dead end
               └──────────────┘

  parent=msg0[sw=0]
               ┌──────────────┐
               │  msg1[sw=0]  │  ← selected; parent for msg2
               └──────────────┘

  parent=msg1[sw=0]
               ┌──────────────┐
               │  msg2[sw=0]  │  ← selected
               └──────────────┘
               ┌──────────────┐
               │  msg2[sw=1]  │  ← sibling; dead end
               └──────────────┘
"""

import json
import os
import sys
import time
import uuid
from datetime import datetime, timezone


# ---------------------------------------------------------------------------
# UUID v7 helpers
# ---------------------------------------------------------------------------

def _uuid7(timestamp_ms: int) -> str:
    """Return a UUID v7 string for the given Unix timestamp (milliseconds).

    Layout (RFC 9562 §5.7):
      bits  0–47   unix_ts_ms  (48 bits, big-endian)
      bits 48–51   version = 0x7
      bits 52–63   rand_a      (12 random bits)
      bits 64–65   variant = 0b10
      bits 66–127  rand_b      (62 random bits)
    """
    rand = int.from_bytes(os.urandom(10), "big")
    rand_a = (rand >> 62) & 0xFFF          # 12 bits
    rand_b = rand & 0x3FFF_FFFF_FFFF_FFFF  # 62 bits

    n = (
        (timestamp_ms & 0xFFFF_FFFF_FFFF) << 80  # 48-bit timestamp in top
        | 0x7 << 76                               # version nibble
        | rand_a << 64                            # rand_a
        | 0b10 << 62                              # variant bits
        | rand_b                                  # rand_b
    )
    return str(uuid.UUID(int=n))


def _parse_ts_ms(date_str: str) -> int:
    """Parse an ISO 8601 / RFC 3339 date string → Unix milliseconds."""
    dt = datetime.fromisoformat(date_str.replace("Z", "+00:00"))
    return int(dt.timestamp() * 1000)


# ---------------------------------------------------------------------------
# Conversion
# ---------------------------------------------------------------------------

def convert(input_path: str) -> dict:
    with open(input_path, "r", encoding="utf-8") as fh:
        lines = [ln.strip() for ln in fh if ln.strip()]

    # Parse all lines; skip the chat_metadata line.
    st_messages = []
    for line in lines:
        obj = json.loads(line)
        if "chat_metadata" in obj:
            continue
        st_messages.append(obj)

    nodes: dict[str, dict] = {}

    # UUID of the selected swipe from the *previous* ST message.
    # This becomes the parent_id for all swipes at the current position.
    prev_selected_uuid: str | None = None

    for st_msg in st_messages:
        is_user: bool = st_msg.get("is_user", False)
        role = "user" if is_user else "assistant"

        # Swipe variants at this position.
        swipes: list[str] = st_msg.get("swipes") or [st_msg.get("mes", "")]
        selected_idx: int = st_msg.get("swipe_id", 0)

        # Per-swipe metadata (timestamps, reasoning).
        swipe_info: list[dict] = st_msg.get("swipe_info") or []

        # Base timestamp for this message.
        try:
            base_ts = _parse_ts_ms(st_msg.get("send_date", ""))
        except Exception:
            base_ts = int(time.time() * 1000)

        # Reasoning on the top-level message applies to the selected swipe.
        top_reasoning: str = (st_msg.get("extra") or {}).get("reasoning") or ""

        selected_uuid: str | None = None

        for swipe_idx, swipe_content in enumerate(swipes):
            # Derive a timestamp for this specific swipe.
            # Use swipe_info if available; otherwise add 1 ms per swipe index so
            # that UUID7 ordering matches swipe order when timestamps are equal.
            ts = base_ts + swipe_idx
            if swipe_idx < len(swipe_info):
                si_date = (swipe_info[swipe_idx] or {}).get("send_date", "")
                if si_date:
                    try:
                        ts = _parse_ts_ms(si_date)
                    except Exception:
                        pass

            node_id = _uuid7(ts)

            # Reasoning: selected swipe gets top-level extra.reasoning;
            # other swipes get reasoning from their own swipe_info entry.
            if swipe_idx == selected_idx:
                reasoning = top_reasoning
            else:
                si_extra = (swipe_info[swipe_idx] if swipe_idx < len(swipe_info) else {}) or {}
                reasoning = (si_extra.get("extra") or {}).get("reasoning") or ""

            node: dict = {
                "id": node_id,
                "parent_id": prev_selected_uuid,
                "role": role,
                "content": [swipe_content] if swipe_content else [],
            }

            if reasoning:
                node["thinking"] = [reasoning]

            nodes[node_id] = node

            if swipe_idx == selected_idx:
                selected_uuid = node_id

        # Selected swipe at this position is the parent for the next position.
        prev_selected_uuid = selected_uuid

    chat_id = _uuid7(int(time.time() * 1000))
    return {"id": chat_id, "messages": nodes}


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

EPIBLOOM_CHATS_DIR = os.path.expanduser("~/.local/share/epibloom/data/chats")


def main() -> None:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        sys.exit(1)

    input_path = sys.argv[1]
    chat = convert(input_path)

    chat_id = chat["id"]
    msg_count = len(chat["messages"])

    os.makedirs(EPIBLOOM_CHATS_DIR, exist_ok=True)
    output_path = os.path.join(EPIBLOOM_CHATS_DIR, f"{chat_id}.json")

    with open(output_path, "w", encoding="utf-8") as fh:
        fh.write(json.dumps(chat, indent=2, ensure_ascii=False))

    print(f"Wrote {output_path}")
    print(f"Chat ID  : {chat_id}")
    print(f"Messages : {msg_count}")


if __name__ == "__main__":
    main()
