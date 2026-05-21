# Plan: Replace `epibloom.llm` with `epibloom.anthropic`

## Goal

Replace the generic `epibloom.llm` Lua module (which dispatches to a backend by
name) with a provider-specific `epibloom.anthropic` module that exposes the full
Anthropic message structure, enabling Claude-specific features such as
per-block `cache_control` points. Remove the now-unused Featherless backend.

## Motivation

The current `epibloom.llm` API forces all message content to be plain strings,
which means features like prompt-caching breakpoints (`cache_control`) cannot be
expressed. A provider-specific module lets the Lua API mirror the Anthropic API
shape directly without any awkward generic escape hatches.

---

## Steps

### Step 1 — Delete Featherless

- Delete `epibloom/src/backend/featherless.rs`
- Delete `epibloom/src/backend/featherless/` (directory with `chat_completion.rs`)
- In `epibloom/src/backend.rs`:
  - Remove `pub mod featherless;`
  - Remove the `TypeMapKey` impl for `featherless::FeatherlessApi`

### Step 2 — Remove the `Backend` trait and `Backends` typemap

With only Anthropic remaining, the generic abstraction is unnecessary.

- In `epibloom/src/backend.rs`:
  - Delete the `Backend` trait, `GenerateChunk` enum, and `Backends` struct
    (everything except the `pub mod anthropic` declaration and any shared types
    still needed).
  - The file may become very small or disappear entirely; if it only re-exports
    `pub mod anthropic`, consider flattening it, but a thin `backend.rs` is also
    fine.
- In `epibloom/src/trace/store.rs`:
  - Replace `Mutex<Backends>` with `Mutex<Option<Arc<AnthropicApi>>>`.
  - Remove the generic `generate<B: Backend + TypeMapKey …>()` method, replacing
    it with a non-generic `generate()` (signature detailed in Step 4).

### Step 3 — Expose the Anthropic request message type and change `generate()` signature

The goal is that the caller builds the full Anthropic request message list and
passes it in, rather than having the backend derive it from a `trace::Trace`.

- In `epibloom/src/backend/anthropic/messages.rs`:
  - Make `request::Message` and `request::Request` (or at least the parts needed
    externally) `pub` so `script.rs` can construct them.
  - Change `pub fn generate(client, config, trace: trace::Trace)` to
    `pub fn generate(client, config, system: Option<String>, messages: Vec<request::Message>)`.
    The function already extracts the system prompt from the trace; now the
    caller does that extraction instead.
  - `extract_system_prompt_from_trace()` can be kept as a helper or inlined into
    the new caller — delete it if no longer needed.
- In `epibloom/src/backend/anthropic.rs`:
  - Change `AnthropicApi::generate()` to match — it now takes
    `(config, system, messages)` instead of `(config, trace)` and forwards
    directly to `messages::generate`.
  - Remove the `Backend` trait impl (the trait is deleted).
  - Keep `new_with_key_from_env()` (renamed or kept as a plain associated fn)
    and `list_models()` unchanged.

### Step 4 — Update `TraceStore::generate()`

`TraceStore::generate()` currently creates the trace from `Vec<trace::Message>`,
then calls the generic backend. Now it receives both the Anthropic-native
messages (for the API call) and derives trace messages from them (for storage).

New signature (non-generic):

```rust
pub async fn generate(
    &self,
    trace_messages: Vec<trace::Message>,   // flattened, for storage/display
    system: Option<String>,                // extracted system prompt
    anthropic_messages: Vec<request::Message>, // full Anthropic structure
    config: backend::anthropic::Config,
) -> Result<(Uuid, broadcast::Receiver<TraceEvent>), Error>
```

- Remove `use typemap_rev::TypeMapKey;` and all generic bounds that came from it.
- Lazily initialise `AnthropicApi` from `ANTHROPIC_API_KEY` env var on first call
  (stored in `Mutex<Option<Arc<AnthropicApi>>>`).

### Step 5 — Add `epibloom.anthropic` Lua module in `script.rs`

Replace the `epibloom.llm` block with an `epibloom.anthropic` block.

The module exposes a single `generate(messages, config?)` function.

**Lua-visible message shape (deserialized from Lua tables in Rust):**

```
Message {
    role: "system" | "user" | "assistant",
    content: string | { TextBlock },   -- plain string or list of content blocks
    thinking: string | nil,            -- for assistant turns (extended thinking)
}

TextBlock {
    type: "text",
    text: string,
    cache_control: { type: "ephemeral" } | nil,
}
```

**In Rust (in `script.rs`):**

- Define a `LuaAnthropicMessage` struct (or deserialize manually) that handles
  `content` being either a string or a table of content blocks.
- Add a helper `lua_messages_to_anthropic_and_trace()` that:
  1. Iterates `LuaAnthropicMessage` list.
  2. Pulls out system messages → concatenates → `Option<String>` system prompt.
  3. Converts each non-system message into a `request::Message` (Anthropic format,
     preserving `cache_control`).
  4. Also produces `Vec<trace::Message>` by flattening each message's text blocks
     to a single string (concatenating text fields, discarding `cache_control`).
- Call `trace_store.generate(trace_messages, system, anthropic_messages, config).await`.

**The returned handle** is the same `LuaLlmGenerateHandle` with the same
`trace_id`, `__call`, and `wait()` interface — no changes needed there.

**Remove** the `epibloom.llm` block (the old `llm_module` table setup).

Also clean up `script.rs` imports: remove anything that only existed to support
the old generic `generate<B>` call (Featherless config, `backend::featherless`,
the match on backend string, etc.).

### Step 6 — Update Teal type stubs

- **Delete** `epibloom/scripts/epibloom/llm.d.tl` and `epibloom/scripts/epibloom/llm.d.lua`.
- **Create** `epibloom/scripts/epibloom/anthropic.d.tl` with the following types:

```teal
local record anthropic
    enum Role
        "system"
        "user"
        "assistant"
    end

    record CacheControl
        type: "ephemeral"
    end

    record TextBlock
        type: "text"
        text: string
        cache_control: CacheControl | nil
    end

    -- content is a plain string (shorthand) or a list of content blocks.
    record Message
        role: Role
        content: string | {TextBlock}
        thinking: string | nil
    end

    record Config
        max_tokens: integer
        model: string
        stop_sequences: {string}
        thinking_budget: integer | nil
    end

    enum ChunkType
        "thinking"
        "message"
        "error"
    end

    record GenerateChunk
        type: ChunkType
        delta: string
        message: string
    end

    record GenerateHandle
        trace_id: string
        metamethod __call: function(GenerateHandle): GenerateChunk | nil
        wait: function(GenerateHandle): (boolean, string | nil)
    end

    generate: function(messages: {Message}, config?: Config): GenerateHandle
end

return anthropic
```

- Run `tl gen` on it to produce `epibloom/scripts/epibloom/anthropic.d.lua`.

### Step 7 — Update `chat.tl` (and other scripts)

- Change `require("epibloom.llm")` → `require("epibloom.anthropic")`.
- Remove the `"anthropic"` second argument from `llm.generate(messages, "anthropic")` → `anthropic.generate(messages)`.
- Update any type annotations that reference `llm.Role`, `llm.Message`,
  `llm.GenerateChunk`, `llm.GenerateHandle` → `anthropic.*`.
- Regenerate `chat.lua` via `tl gen`.

Do the same for any other `.tl` files that `require("epibloom.llm")` (currently
only `chat.tl`, but check `chat_action.tl` too).

---

## Verification

After each step, or at the end:

1. `just check-server` — `cargo clippy` must pass with no errors.
2. `just check-scripts` — `tl check` must pass.
3. `just copy-scripts` — compiles and deploys scripts.
4. `just run` — server starts, no panics.
5. Manual smoke test: send a chat message, confirm streaming works end-to-end.

---

## Non-goals (out of scope for this plan)

- Image, document, or tool-use content blocks exposed to Lua.
- A separate `system` parameter in `anthropic.generate()` (system messages in
  the messages array continue to be extracted server-side).
- Richer trace message storage (content blocks in trace — traces continue to
  store plain strings).
