# Chat: Design Choices and Known Limitations

Notes from the initial implementation of chat (backend scripts in `epibloom/scripts/chat.lua` and `chat_action.lua`, UI in `epibloom/ui/src/components/chat/`), for later review and action.

## Design choices to review

### Backend

- [ ] **Two state objects per chat.** `chat:<id>:persist` holds the `Chat` (settings + messages); `chat:<id>` holds the ephemeral `ChatStatus` (generating, cancel requested, which message, trace name). The status object resets on restart, so a crash mid-generation can't leave the chat stuck as "generating".
- [ ] **Lock order.** Anything touching both objects locks the persistent object first, then the status object nested inside its update function (`chat._update()`). Code that only touches the status object (eg. `chat.cancel_generate()`) may lock it alone. This order must always be followed to avoid deadlocks.
- [ ] **Generation is split into three steps** because `StateObject:update()` holds its lock for the whole update function and only broadcasts at the end, so generating inside one update would show nothing until it finished:
    1. Lock both; validate, add a placeholder message, start the LLM request, set `generating`.
    2. Stream each text chunk in as its own small update.
    3. Lock both; record any error, clear `generating`.
- [ ] **All mutations are rejected while generating**, except the generation's own updates. This includes things that are arguably harmless, like changing swipes or editing the system prompt.
- [ ] **Indices are 1-based everywhere**, including `selected` in the stored JSON. The UI has to use `alternates[selected - 1]`. Consider switching the stored representation to 0-based if this gets annoying.
- [ ] **System prompt is a chat field** (`chat.system`), not a message with role `"system"`. The `"system"` role was removed from `Role`.
- [ ] **Model and max tokens are per-chat fields** (`chat.model`, `chat.max_tokens`), defaulting to `chat.DEFAULT_MODEL` / `chat.DEFAULT_MAX_TOKENS`.
- [ ] **Only text content blocks are stored.** Other streamed block types (eg. thinking) are ignored.
- [ ] **Messages with no non-empty text are skipped** when building the LLM request, since the API rejects empty content.
- [ ] **Failed or cancelled generations leave the message in place**, with an `error` field and whatever text arrived. They are not removed automatically.
- [ ] **New chats are named after their creation timestamp** unless a name is given.

### UI

- [ ] **Chat is the first and default tab.**
- [ ] **Messages are rendered as plain text** (line breaks kept), not Markdown. `react-markdown` and `remark-gfm` are already dependencies.
- [ ] **Script contexts.** Generation runs on `chat-generate:<chatId>` (one per chat). Everything else, including Stop, runs on the shared `chat` context, so edits and cancelling aren't queued behind a generation.
- [ ] **Script runner.** `ScriptRunnerProvider` (used via `useScriptRunner()`) subscribes over a single connection to `script-status:<context>` for every context used so far, and turns each run into a promise. Adding a new context reconnects that subscription.
- [ ] **The trace sidebar finds its message by trace name**, not by stored indices, so it survives messages being inserted or deleted.
- [ ] **Deleting asks for confirmation** with `window.confirm()`.
- [ ] **The selected chat is stored in `localStorage`** (`epibloom.chat.selected`). The open trace is not remembered across tab switches.
- [ ] **The "Send" button changes with context:** "Send" with text, otherwise "Generate" (last message from user) or "New swipe" (last message from assistant).

## Known limitations

### Backend

- [ ] **Cancellation is only checked when a stream event arrives.** If the API stalls completely, Stop won't take effect until the next event. Fixing this would need a way to cancel a handle from another context (eg. a shared registry of handles in Rust).
- [ ] **Cancellation must come from a different script context** than the generation, since a context runs one script at a time.
- [ ] **Orphaned generation edge case.** In `chat.generate()`, the LLM request is started inside the update function. If the update is rolled back after that (eg. the status object update fails), the request keeps running with nothing consuming it.
- [ ] **The `generating` flag could get stuck until restart** if the final update in `chat.generate()` itself fails (eg. the chat disappeared).
- [ ] **Each streamed chunk converts the whole chat** between JSON and Lua (and checks the chat exists by scanning the index). This is fine for now but scales with chat length.
- [ ] **Persisted chats are rewritten in full** (pretty-printed) on every debounced save, including during streaming.
- [ ] **No delete-chat action.**
- [ ] **No branching/tree structure.** Each message slot has alternates, but changing an earlier swipe doesn't preserve the later messages that followed the previous swipe.

### UI

- [ ] **The chat list subscribes to every chat in full** just to show names and generating status. With many long chats, a small backend summary object (eg. `chats:index` with names, modified time, and generating flag) would be cheaper.
- [ ] **Browser connection limit.** Each `useStateObjects` call is a separate SSE connection, and browsers allow about 6 per host over HTTP/1.1. The Chat tab uses up to four (index, chats, script status, trace sidebar). Other features adding subscriptions could hit the limit. Consider multiplexing all subscriptions over a single connection.
- [ ] **No edit/delete for the system prompt block** other than through Settings.

### Backend infrastructure (noticed while building chat)

- [ ] **`script-status:<context>` history grows without limit** and is sent in full on every (re)connect.
- [ ] **Each script context is an OS thread that is never cleaned up**, so per-chat generation contexts accumulate over the server's lifetime.
- [ ] **After a backend restart, script status indices start again from 0**, so a pending UI promise from before the restart could resolve against an unrelated run.

## Cleanup

- [ ] Delete the test chats ("UI test chat", "Test chat") from `~/.local/share/epibloom/state/` if they're no longer wanted.
