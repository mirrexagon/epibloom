# Performance Analysis

Two distinct slowness problems have been identified:

1. **Streaming is slow** — updating individual message content during generation
   causes visible lag or jank.
2. **Navigation is slow** — switching the viewed branch/node by clicking in the
   flowchart or using the sibling navigator is slow, especially in long chats.

---

## Root causes

### A. Full deep-clone on every SSE patch (`useStateObjects.ts`)

`fast-json-patch.applyPatch(..., mutate: false)` **deep-clones the entire chat
object** before applying each patch.  A streaming response generates dozens of
patches per second (`/messages/<uuid>/content/-` appends).  For a chat with 100
messages each with a long content array, every single token triggers an O(N)
clone of the whole document.  This gets visibly slower as a chat grows.

The direct consequence: every patch produces a brand-new `messages` object in
which **every individual message object also has a new reference**, even for
messages whose content didn't change.  React then considers every message as
"changed", which brings us to the next problem.

### B. React re-renders the entire chat on every patch

`chatData` is derived from a single state object.  Any patch — even a single
streamed token — replaces the reference at every level of the object tree (see A),
so every `useMemo` that depends on `chatData` or `messages` recomputes, and every
`MessageBubble` re-renders with a new `message` prop reference.  Since
`MessageBubble` is not memoised, all bubbles redraw on every token.

### C. The flowchart re-renders all nodes on selection change

When `selectedLeafId` changes, `pathIds` is recomputed and then the entire `nodes`
array is rebuilt in a `useMemo` (because `pathIds` changed).  Every node gets a
new `data` object with a fresh `isOnPath` boolean — even nodes that weren't on
the path before and aren't now.  React Flow sees N changed nodes and re-renders
all N custom node components.

---

## Options

### Option 1 — Structural sharing in patch application (frontend only, high impact)

Replace `applyPatch(…, mutate: false)` with a function that does **structural
sharing**: it creates new object references only along the path touched by each
patch, leaving all unrelated branches of the object tree pointing to their old
references.

For the common streaming patch shape (`{ op: "add", path:
"/messages/<uuid>/content/-", value: "…" }`), this means:

```
new root = { ...old root }
new root.messages = { ...old root.messages }
new root.messages[uuid] = { ...old root.messages[uuid] }
new root.messages[uuid].content = [...old content, newChunk]
```

All other messages keep the **same** object reference.  Combined with
`React.memo` on `MessageBubble`, only the streaming message re-renders per
token.  This also fixes problem C above: if a navigation change doesn't alter
message content at all, the `message` reference inside each node's `data` is
stable, so `React.memo` on the node component prevents unnecessary redraws.

**Changes needed:** `useStateObjects.ts` (custom patch applier or switch to
`immer`), `ChatView.tsx` (wrap `MessageBubble` and `SiblingNavigator` in
`React.memo`), `ChatFlowchart.tsx` (wrap `ChatNodeComponent` in `React.memo`).
No backend changes.

**Downside:** Non-trivial to implement a correct structural-sharing patch
applier that handles all JSON Patch op types.  `immer` would handle it
automatically but is another dependency.

---

### Option 2 — Debounce / batch frontend patch application (frontend only, medium impact)

Instead of applying every SSE patch immediately, accumulate incoming patches over
a short window (e.g. 50 ms) and apply them all in one `setState` call.  This
caps the React re-render rate at ~20/s regardless of how fast the LLM streams
tokens, which is sufficient for smooth visual updates.

The deep-clone cost (problem A) is also amortised: instead of cloning the whole
document 30 times a second you clone it at most 20 times a second, and a single
clone applies N accumulated patches.

**Changes needed:** `useStateObjects.ts` only.  No backend changes.  Composable
with Option 1.

**Downside:** Each token appears up to 50 ms later than it arrives.  Barely
perceptible at typical streaming speeds (~10–30 tokens/s) but worth noting.

---

### Option 3 — Separate streaming content from tree structure at the backend (backend + frontend, high impact)

Split the single `chat:<id>` state object into two:

| State object | Contents | Update frequency |
|---|---|---|
| `chat:<id>` | Tree structure: message IDs, parent IDs, roles, trace IDs, errors. No `content` arrays. | Only when messages are added or generation completes. |
| `chat:<id>:stream` | `{ message_id, content: string[], thinking: string[] }` for the single currently-generating message. Cleared between generations. | Every streamed token. |

The UI subscribes to both.  The flowchart subscribes only to `chat:<id>` (tree
structure), so it never re-renders during streaming at all.  The main chat view
renders the streaming message from `chat:<id>:stream`, and reads all completed
message content from the tree structure.

This eliminates problem A entirely for streaming (the stream object is tiny so
cloning it is cheap) and mostly eliminates problem B and C for the same reason.

**Changes needed:** Significant Lua changes to `chat.lua` (split content out of
the main chat state, write to `chat:<id>:stream` during generation, merge back at
end). Frontend changes to subscribe to both objects and merge them in the UI.

**Downside:** More complex data flow.  The merge of streaming content with the
completed message tree needs careful handling (e.g. race conditions, navigating
away while generating).

---

### Option 4 — Per-message state objects (backend + frontend, highest granularity)

Each message is its own state object: `chat:<id>:message:<message-id>`.  A
"directory" object (`chat:<id>`) lists the existing message IDs and their
parent/role metadata.  The UI subscribes to the directory to render the tree, and
subscribes to individual message objects only for messages that are currently
visible in the chat scroll view or actively streaming.

This is the most granular approach and scales the best, but is the most complex.

**Changes needed:** Major refactor of both `chat.lua` and the UI subscription
logic.

---

### Option 5 — Stream via trace events instead of state patches (backend + frontend, targeted)

Traces already have their own SSE stream (`GET /api/trace/<id>`).  Instead of
pushing content chunks through the chat state object, the UI could watch the
trace SSE directly for the currently-generating assistant message and display
content from there.  The chat state object is only updated once, when generation
completes, with the final content.

This eliminates streaming-related state patches entirely.  Navigation performance
is unaffected (needs Option 1 or 3 for that).

**Changes needed:** Small Lua change (don't push content chunks to chat state
during generation; store final content at end).  Frontend: subscribe to the
trace for the generating message ID, merge streaming content with the static
chat state.  The `trace_id` field on each message already makes it easy to find
the right trace.

**Downside:** There's a window between when generation completes and when the
content appears in the chat state — the UI needs to handle this transition
cleanly.

---

## Recommended path

1. **Option 2** (batching) is a one-file change that gives immediate relief with
   minimal risk.  Ship this first.
2. **Option 1** (structural sharing) combined with `React.memo` on message
   bubbles and flowchart nodes eliminates the root cause of both problems at the
   frontend level.  This plus Option 2 should make the app feel fast for typical
   chat sizes.
3. **Option 3 or 5** if very long chats (1000+ messages) or very fast streaming
   still cause problems after the frontend fixes, since those options eliminate
   backend-to-frontend data volume for streaming entirely.
