# Trace Module Refactoring Plan

## Overview

The trace module (`epibloom/src/trace.rs` and `epibloom/src/trace/session.rs`) has grown messy. This plan restructures it into focused submodules organized by concept, simplifies the event types, cleans up the generation state machine, and separates persistence from data types.

## Current Problems

1. **`session.rs` is a grab-bag** — Contains `TraceStoreEvent`, `TraceEvent`, `TraceStore`, `Session`, `State`, and `process_generation_stream`, all with different responsibilities.
2. **`TraceEvent` is needlessly verbose** — 10 variants encoding `{System, User, Assistant} × {Start, Delta, End}` plus thinking and error. Causes repetitive match arms everywhere.
3. **Persistence is baked into `Trace`** — `Trace` knows about `TRACES_DIR` (a global) and has load/save methods, coupling a data struct to filesystem layout.
4. **`Session::start_generating` has an awkward self-as-Arc signature** — Takes `session_arc: Arc<RwLock<Session>>` because it hands itself to a spawned task.
5. **Generation state is spread across loose `Session` fields** — `state`, `current_assistant_thinking`, `current_assistant_message`, `generate_complete_notify` are logically one state machine but are scattered.
6. **Naming is confusing** — `Trace` is really the serializable conversation data; `Session` is the live runtime wrapper.

## Target File Layout

```
trace/
  mod.rs              Re-exports + Error enum
  conversation.rs     Role, Message, TraceMetadata, Trace (pure data, no I/O)
  persistence.rs      load/save free functions (filesystem I/O for conversations)
  event.rs            TraceEvent, TraceStoreEvent (simplified variants)
  session.rs          Session, GenerationState, generation driving logic
  store.rs            TraceStore (manages sessions, owns Backends, spawn point)
```

Conceptual groupings:

- **`conversation.rs`** — "What is a conversation?" Pure serializable types, no behaviour beyond construction.
- **`persistence.rs`** — "How do conversations get saved/loaded?" All filesystem I/O, depends on `conversation` types.
- **`event.rs`** — "What can happen to a trace?" The event enums that flow over SSE and broadcast channels.
- **`session.rs`** — "What does a live, in-progress trace look like?" Runtime state machine: `Session`, `GenerationState`, stream-driving logic. Everything about a *single* active trace.
- **`store.rs`** — "How are all traces managed together?" `TraceStore` that holds sessions, owns `Backends`, handles creation/lookup, and is the entry point for starting generation.
- **`mod.rs`** — Thin glue: `Error` enum (referenced across submodules) and re-exports of the public surface.

Dependency flow: `store → session → event + conversation`, `persistence → conversation`, `mod` ties them together.

## Steps

Each step should compile and work on its own so changes can be reviewed incrementally.

### Step 1: Simplify `TraceEvent`

Replace the 10-variant enum with 5 role-agnostic variants:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TraceEvent {
    MessageStart { position: usize, role: Role },
    ThinkingDelta { delta: String },
    ContentDelta { delta: String },
    MessageEnd { position: usize },
    Error { message: String },
}
```

This eliminates all "match on three roles to do the same thing" blocks in `set_message()`, `get_trace_stream()`, and the Lua `LuaTraceGenerateHandle`.

**Affected files:** `trace/session.rs`, `api/trace.rs`, `script.rs`, and the frontend UI TypeScript types.

### Step 2: Extract `GenerationState`

Pull in-flight generation fields out of `Session` into a dedicated type:

```rust
enum GenerationState {
    Idle,
    Generating {
        thinking_so_far: String,
        content_so_far: String,
        complete_notify: Arc<Notify>,
    },
}
```

`Session` holds `generation: GenerationState` instead of four separate fields. State transitions become explicit — replacing a variant rather than clearing strings and flipping a flag. The "catch up a late-joining subscriber" logic becomes a single `if let Generating { .. }` block.

### Step 3: Move generation spawn into `TraceStore`

Instead of `Session::start_generating(session_arc, backend, config)`, make `TraceStore::start_generating` own the spawn:

```rust
impl TraceStore {
    pub async fn start_generating<B: Backend>(
        &self, trace_id: Uuid, config: B::Config,
    ) -> Result<broadcast::Receiver<TraceEvent>, Error> {
        let session_arc = self.get_session(trace_id).await?;
        let backend = self.backends.lock().await.get::<B>()?;

        let (trace_snapshot, rx) = {
            let mut session = session_arc.write().await;
            session.begin_generation()?
        };

        let stream = backend.generate(&config, &trace_snapshot);
        tokio::spawn(drive_generation(session_arc, stream, trace_snapshot.messages.len()));

        Ok(rx)
    }
}
```

`Session` exposes `begin_generation() -> Result<(Trace, Receiver)>` and `finish_generation(thinking, content)` as clean state transitions. The free function `process_generation_stream` becomes `drive_generation`.

### Step 4: Pull persistence out of `Trace`

Move load/save methods from `Trace` into `persistence.rs` as free functions:

```rust
// persistence.rs
pub async fn save_trace(trace: &Trace, traces_dir: &Path) -> Result<(), Error> { ... }
pub async fn load_trace(trace_id: Uuid, traces_dir: &Path) -> Result<Trace, Error> { ... }
pub async fn load_all_traces(traces_dir: &Path) -> Result<Vec<Trace>, Error> { ... }
```

`Trace` becomes a pure serializable data struct with no dependency on globals or I/O.

### Step 5: Split into submodule files

Now that each concern is cleanly separated, perform the mechanical file split into the target layout described above. This is mostly moving code between files and updating `use` paths.

### Step 6: Rename (optional)

Two options:

- **Option A (follow existing TODO):** `Trace` → `Conversation`, `Session` → `Trace`
- **Option B (minimal):** `Trace` stays, `Session` → `LiveTrace` or `ActiveTrace`

Option B is less disruptive. Decide at the time based on how things feel after the structural changes.
