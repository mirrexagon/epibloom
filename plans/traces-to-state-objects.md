# Plan: Migrate LLM Traces to State Objects

## Goal

Replace the separate `TraceStore` / `Session` / `TraceEvent` system with ordinary
state objects. Each LLM generation will produce two well-known state objects:

- **`llm-trace:<uuid>`** — the full trace value (input messages + streaming
  assistant response)
- **`llm-trace-index`** — an append-only list of trace metadata entries, for
  discovery/listing

Persistence is intentionally out of scope for now (can be re-added later via the
existing filesystem module).

---

## New State Object Schemas

### `llm-trace:<uuid>`

Initial value (written atomically before generation starts):

```json
{
  "id": "<uuid>",
  "name": "2026-05-27 10:00:00",
  "created": "2026-05-27T10:00:00+00:00",
  "messages": [
    { "role": "system",    "content": ["System prompt text"] },
    { "role": "user",      "content": ["Hello!"] },
    { "role": "assistant", "content": [], "thinking": [], "generating": true }
  ]
}
```

- All input messages (including `system`) are stored verbatim in the array for
  debugging visibility.
- The trailing assistant entry is added by the backend before generation starts.
  Its `content` and `thinking` are empty arrays that grow via JSON Patch
  array-append as chunks stream in (same pattern as `ChatMessage.content`).
- On completion `generating` is set to `false`.
- On error `error` is set to the error string and `generating` to `false`.

### `llm-trace-index`

Starts as `[]` (initialised by `LlmStore::new`). Each new trace appends one
entry via a JSON Patch array-append:

```json
[
  { "id": "<uuid1>", "name": "2026-05-27 10:00:00", "created": "..." },
  { "id": "<uuid2>", "name": "...",                  "created": "..." }
]
```

---

## Changes

### Phase 1 — Rust backend

#### 1a. Create `epibloom/src/llm.rs`

New module containing:

```
pub enum LlmChunk {
    Thinking { delta: String },
    Message  { delta: String },
    Error    { message: String },
}

pub struct LlmStore {
    anthropic:   Mutex<Option<Arc<AnthropicApi>>>,
    state_store: Arc<StateStore>,
}
```

`LlmStore::new(state_store)` — also writes the initial `[]` to
`llm-trace-index`.

`LlmStore::generate(trace_messages, system, anthropic_messages, config)`
→ `Result<(String, mpsc::Receiver<LlmChunk>), Error>`

Steps inside `generate`:
1. Generate a `Uuid::now_v7()` and a human-readable datetime name.
2. Build the initial state value (all input messages + empty assistant entry with
   `generating: true`).
3. `get_or_create("llm-trace:<uuid>")` and write initial value atomically.
4. `get_or_create("llm-trace-index")` and append the new metadata entry.
5. Lazily init `AnthropicApi` (same pattern as the old `TraceStore`).
6. Create an `mpsc::channel(1024)` for `LlmChunk`s.
7. Spawn a tokio task that:
   - Iterates the Anthropic stream.
   - For each `ThinkingDelta`: appends to `messages[last].thinking` in the state
     object **and** sends `LlmChunk::Thinking` on the channel.
   - For each `MessageDelta`: same for `messages[last].content` / `LlmChunk::Message`.
   - On stream error: sets `error` + `generating: false` in state, sends
     `LlmChunk::Error`, closes channel.
   - After the stream ends normally: sets `generating: false` in state, closes
     channel (the Lua handle detects closure as "done").
8. Return `(uuid_str, rx)`.

#### 1b. Modify `epibloom/src/script.rs`

- Remove all `trace` imports and the `LuaTraceStore` struct.
- Replace the `epibloom.anthropic` Lua module with `epibloom.llm`:
  - The registry key changes from `"epibloom.anthropic.trace_store"` to
    `"epibloom.llm.store"`.
  - The `generate` async function now accepts a `backend: String` second
    argument (error if not `"anthropic"` for now).
  - Calls `llm_store.generate(...)` instead of `trace_store.generate(...)`.
- `LuaLlmGenerateHandle` replaces `broadcast::Receiver<TraceEvent>` with
  `mpsc::Receiver<LlmChunk>`:
  - `__call` loops over `rx.recv().await`:
    - `Some(LlmChunk::Thinking { delta })` → return `GenerateChunk::Thinking`
    - `Some(LlmChunk::Message  { delta })` → return `GenerateChunk::Message`
    - `Some(LlmChunk::Error { message })` → return `GenerateChunk::Error`
    - `None` (channel closed) → return `nil`
  - `wait()` drains until `None` or `Error`.
- `ContextStore::new` takes `Arc<LlmStore>` instead of `Arc<TraceStore>`.
- `Context::new` stores it in the Lua registry under `"epibloom.llm.store"`.
- Rename the function `lua_messages_to_anthropic_and_trace` →
  `lua_messages_to_llm_input` (same logic, but the first return value
  (`Vec<Message>`) now uses a local `TraceMessage { role, content, thinking }`
  type defined in `llm.rs` instead of `trace::Message`).

#### 1c. Modify `epibloom/src/main.rs`

- Remove `mod trace`, `TRACES_DIR`, and the startup `load_traces_from_directory`
  fairing.
- Remove the `Arc<TraceStore>` managed state and trace routes.
- Remove trace-related entries in the `#[openapi]` macro.
- Create `Arc<LlmStore>` (calling `LlmStore::new`) and pass it to
  `script::ContextStore::new`.
- Remove `trace` from the `#[openapi(nest(...))]`.

#### 1d. Delete files

- `epibloom/src/trace/` (entire directory)
- `epibloom/src/api/trace.rs`

---

### Phase 2 — Lua scripts

#### 2a. Create `epibloom/scripts/epibloom/llm.d.tl`

Rename the module from `anthropic` to `llm`. The type definitions are mostly
the same; notable changes:
- Module name: `llm` (was `anthropic`)
- `generate` signature: `generate: function(messages: {Message}, backend: string, config?: Config): GenerateHandle`
- The `Role` enum and all other types stay identical.

#### 2b. Delete `epibloom/scripts/epibloom/anthropic.d.tl` and `anthropic.d.lua`

#### 2c. Update `epibloom/scripts/chat.tl`

- `require("epibloom.anthropic")` → `require("epibloom.llm")`
- All `anthropic.X` type references → `llm.X`
- `anthropic.generate(messages)` → `llm.generate(messages, "anthropic")`
- `handle.trace_id` is unchanged (still a UUID string, now the suffix of
  `llm-trace:<uuid>`).

#### 2d. Compile scripts

Run `just check-scripts` then `just copy-scripts`.

---

### Phase 3 — Frontend

#### 3a. Update `src/types/api.ts`

Remove:
- `TraceMetadata`, `TraceStoreEvent`, `TraceEvent`, `Message` (the
  delta-event-based type)

Add:
```ts
export interface LlmTraceMessage {
  role: Role;
  content: string[];
  thinking?: string[];
  generating?: boolean;
  error?: string;
}

export interface LlmTrace {
  id: string;
  name: string;
  created: string;
  messages: LlmTraceMessage[];
}

export interface LlmTraceIndexEntry {
  id: string;
  name: string;
  created: string;
}
```

#### 3b. Rewrite `src/hooks/useTrace.ts`

Uses `useStateObjects(["llm-trace:<id>"])` internally. Returns
`{ trace: LlmTrace | null, error, connected }`.

The hook key-maps the raw state object value to `LlmTrace`.

#### 3c. Rewrite `src/hooks/useTraceList.ts`

Uses `useStateObjects(["llm-trace-index"])`. Parses the array value as
`LlmTraceIndexEntry[]`. Returns `{ traces, error, connected, latestTrace }` with
the same surface API as before.

#### 3d. Update `src/components/TraceView.tsx`

Renders `LlmTrace` directly instead of rebuilding from delta events:
- Join each message's `content` array and `thinking` array.
- Show a spinner/indicator when `generating: true`.
- Show error text when `error` is set.

#### 3e. Update `src/components/TraceList.tsx`

Accepts `LlmTraceIndexEntry[]` instead of `TraceMetadata[]` (same rendered
shape, just updated prop type).

#### 3f. Update `src/App.tsx` and `src/components/DebugDrawer.tsx`

Simplify `selectedTrace` from `TraceMetadata | null` to
`{ id: string; name: string } | null`. Remove the `TraceMetadata` import.
`DebugDrawer` drops the `useTraceList` import (now inside `useTraceList` hook
itself via state objects) and passes the simplified type down.

---

## Execution Order

1. Phase 1 (Rust) — compile-checked with `just check-server` after each step.
2. Phase 2 (scripts) — `just check-scripts`, then `just copy-scripts`.
3. Phase 3 (frontend) — `just check-ui`.
