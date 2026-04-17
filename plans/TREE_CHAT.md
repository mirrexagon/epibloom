# Tree Chat: Design & Implementation Plan

## Overview

Convert the chat system from a linear message array to a tree of messages.  Every
node in the tree is a message; its children are continuations or alternatives.
Two kinds of branching are supported by this one mechanism:

- **New response** — generate a second (or third…) assistant reply to the same
  user message.  The user message gains multiple assistant-message children.
- **New thread** — write a new user message as a follow-up to any assistant
  message, including ones deep in history.  The assistant message gains multiple
  user-message children.

---

## Data Model

### `ChatMessage` (updated)

```
id        : uuid            -- unique node identifier
parent_id : uuid | null     -- null only for root nodes
role      : "user" | "assistant" | "system"
content   : string[]        -- chunks joined for full text
thinking  : string[]?
trace_id  : uuid?
error     : string?
```

### `Chat` (updated)

```
id       : uuid
messages : Record<uuid, ChatMessage>   -- map keyed by message id
```

`messages` is a flat map of all nodes.  The tree structure is implied entirely
by `parent_id` links.  There is no explicit root pointer; root nodes are those
whose `parent_id` is `null` (in practice there should be exactly one).

### JSON Patch streaming

Content chunks continue to arrive as JSON Patch array-append operations, just
now with a path like `/messages/<uuid>/content/0` instead of
`/messages/<n>/content/0`.  No change required to the SSE machinery.

---

## Backend API (script actions)

| Action | New params | Returns | Notes |
|--------|-----------|---------|-------|
| `list` | — | `uuid[]` | unchanged |
| `new` | — | `uuid` | unchanged |
| `poke` | `chat_id` | — | unchanged |
| `get` | `chat_id` | `Chat` | returns new format |
| `add_user_message` | `chat_id`, `parent_id\|null`, `content` | new message `uuid` | replaces `append_message` |
| `generate` | `chat_id`, `parent_message_id` | — | generates assistant node as child of given node |

`generate` no longer implicitly appends to an array.  The caller passes the
exact user-message node to respond to.  The action:
1. Creates a new empty assistant node (child of `parent_message_id`) and writes
   it into the state object immediately.  The SSE push means clients see the
   new node before streaming begins.
2. Builds the LLM trace from the path root → `parent_message_id`.
3. Streams chunks into the new node as before.

The `set_message` helper is removed; `append_message` is replaced by
`add_user_message`.

**No migration of old chats** — the format change is a clean break and saved
chats have been cleared.

---

## UX Design

### Linear chat view — current path

The frontend tracks `selectedLeafId: string | null`, the ID of the node at the
bottom of the currently viewed path.  The **current path** is the ordered
sequence of nodes from the root to `selectedLeafId`, recovered by walking
`parent_id` links upward and reversing.  Only nodes on this path are shown as
message bubbles.

When a chat is first opened, `selectedLeafId` is set to the deepest node
reachable by always following the **first child** from the root (the same
behaviour as the current linear chat).

### Sibling navigation

Where a node on the current path has more than one sibling (i.e. its parent has
multiple children of the same role), a small navigator is shown directly above
the bubble:

```
  ← 2 / 3 →
```

Clicking `←` or `→` swaps which sibling is on the path.  All nodes below that
point are reset to the first-child chain of the newly selected sibling.

### Input bar behaviour

The input bar at the bottom always sends a **new user message as child of
`selectedLeafId`** (for an assistant-message leaf) or **triggers a
re-generation from `selectedLeafId`** (for a user-message leaf).

| State of `selectedLeafId` | Input bar shows |
|--------------------------|-----------------|
| `null` / empty chat | Normal text input, Send button |
| assistant message (leaf) | Normal text input, Send button |
| user message | "Regenerate ↺" button; text input disabled |

After Send:
1. `add_user_message(chat_id, selectedLeafId, content)` → `newUserMsgId`
2. `selectedLeafId = newUserMsgId` (path now ends at the user message)
3. `generate(chat_id, newUserMsgId)` is called (fire-and-forget; streaming
   content arrives via state SSE)
4. A `useEffect` watches `chatData` for a new child of `newUserMsgId`; when
   one appears, it sets `selectedLeafId` to it.  The user sees the response
   streaming in as usual.

### Branching from history

Any message bubble in the current path can be clicked to make it the new
`selectedLeafId`.  This does **not** delete any data; it simply moves the
"cursor".

Visual treatment when `selectedLeafId` is not the deepest node on the
previously displayed path:
- Bubbles below `selectedLeafId` are shown **dimmed** (50 % opacity) to
  indicate they are now "below the branch point" rather than the active
  thread.
- A small label above the input reads **"Branching from here"**.
- Send / Regenerate behaves as described above but creates a new branch
  rather than continuing the existing one.

Clicking the deepest-already-selected leaf again, or pressing Escape, cancels
branch mode and returns `selectedLeafId` to the deepest existing node.

### Regenerate button

When `selectedLeafId` points to an assistant-message leaf, a small
**Regenerate ↺** button is shown alongside the input.  It calls
`generate(chat_id, parentOfCurrentLeaf)`, creating a new sibling to the
current assistant node.  After the new node appears, `selectedLeafId` is moved
to it.

### Tree flowchart panel

A collapsible right panel shows the full tree as an SVG diagram:

- **Layout**: top-down, root at top.  Node x-positions are computed with a
  simple recursive subtree-width algorithm (Reingold–Tilford style) to avoid
  overlap.  Depth determines y-position.
- **Nodes**: rounded rectangles (~160 × 54 px), coloured by role (blue = user,
  dark-grey = assistant, purple = system).  Show first ~40 chars of content.
- **Edges**: straight lines from parent centre-bottom to child centre-top.
- **Current path**: nodes on the path have a bright border / ring.
- **Interaction**: clicking any node sets `selectedLeafId` to that node
  (navigating the linear view to it; nodes below are reset to their
  first-child chains).
- **Pan & zoom**: mouse-drag to pan, scroll-wheel to zoom, implemented as a
  CSS `transform` on the SVG group.  No external library.
- **Toggle**: a button in the top-right of the main panel shows/hides the
  flowchart.  Default: visible.

---

## Files Changed / Created

| File | Action |
|------|--------|
| `epibloom/scripts/chat.lua` | Rewrite for tree model |
| `epibloom/scripts/chat_action.lua` | New/renamed actions |
| `epibloom/ui/src/types/api.ts` | Updated `Chat`, `ChatMessage` types |
| `epibloom/ui/src/components/ChatView.tsx` | Path tracking, branch UI |
| `epibloom/ui/src/components/ChatFlowchart.tsx` | New SVG tree component |

---

## Implementation Steps

Each step ends with the system fully working.

---

### Step 1 — Backend + minimal frontend (data model refactor)

**Goal**: new tree data model end-to-end; UX identical to today (single linear
thread, no branching controls yet).

#### `epibloom/scripts/chat.lua`

- Replace `messages: ChatMessage[]` with `messages: table<uuid, ChatMessage>`.
- Add `id` and `parent_id` fields to `ChatMessage`.
- Remove `chat.set_message`; add `chat.add_message(chat_id, parent_id, role,
  content)` → returns new node ID.
- Rewrite `chat.generate(chat_id, parent_message_id)`:
  - Creates empty assistant node (child of `parent_message_id`) immediately and
    writes to state, so the SSE push reaches clients before streaming starts.
  - Builds the trace by calling a new helper `chat.get_path(messages, node_id)`
    that walks `parent_id` links from the given node back to the root, then
    reverses the result.
  - Streams chunks into the new node's `content` / `thinking` / `error` as now.
- Add `chat.get_path(messages, node_id)` helper.
- Update `chat.save` / `chat.load` (no migration needed; just works with the
  new map format since it's round-tripped as JSON).

#### `epibloom/scripts/chat_action.lua`

- Add `add_user_message` action (calls `chat.add_message`; role forced to
  `"user"`; returns new node ID).
- Update `generate` action to accept `parent_message_id` and forward it.
- Remove `append_message`.
- Keep `list`, `new`, `poke`, `get` unchanged.

#### `epibloom/ui/src/types/api.ts`

- Update `ChatMessage`: add `id: string` and `parent_id: string | null`.
- Update `Chat`: change `messages` from `ChatMessage[]` to
  `Record<string, ChatMessage>`.

#### `epibloom/ui/src/components/ChatView.tsx`

Add pure helper functions (no new state):
- `getChildren(messages, parentId)` — returns child nodes sorted by insertion
  order (a stable sort on some criteria; see note below).
- `getPath(messages, leafId)` — walks `parent_id` chain to root, returns
  root-first array.
- `getFirstLeaf(messages, nodeId)` — follows first child repeatedly until a
  leaf.

Add state:
- `selectedLeafId: string | null` — initially null.

Wiring:
- When `chatData` arrives and `selectedLeafId` is null, compute
  `getFirstLeaf(chatData.messages, rootNodeId)` and set it.
- Compute `currentPath = selectedLeafId ? getPath(messages, selectedLeafId) :
  []` and render those messages.
- **Send**: call `add_user_message(chat_id, selectedLeafId, content)` → get
  `newUserMsgId` → set `selectedLeafId = newUserMsgId` → call
  `generate(chat_id, newUserMsgId)`.
- Auto-navigate to new assistant node: a `useEffect` on `chatData` — if
  `selectedLeafId` is a user-message node, check whether it now has a child;
  if so, set `selectedLeafId` to that child.
- Remove old `append_message` / `generate` calls.

**Insertion order note**: `messages` is a JSON object (map).  To reconstruct
a stable ordering for siblings, each node stores nothing extra; instead, when
inserting a new node, its UUID is time-based (v7). Just use the Lua
`epibloom.uuid` module since it returns UUIDv7s.

**End state**: chat works exactly as before.  One linear thread.  No
branching UI.  Old chats do not load (format changed — that's fine).

---

### Step 2 — Branch navigation in the chat view

**Goal**: full branching UX inside the existing linear chat panel; tree panel
not yet present.

#### `epibloom/ui/src/components/ChatView.tsx`

**Sibling navigator**: above each bubble, if the node has siblings, show
`← X / N →`.  Clicking updates `selectedLeafId` by calling
`getFirstLeaf(messages, sibling.id)` so the path continues from the newly
chosen sibling's first-child chain.

**Click-to-branch on any bubble**:
- Every `MessageBubble` gets an `onClick` prop.
- Clicking sets `selectedLeafId` to that node's ID.
- Bubbles **below** the new `selectedLeafId` (i.e. later on the previously
  displayed path) are rendered at 50 % opacity.
- A "Branching from here" label appears above the input bar.
- Clicking the same bubble again, or pressing Escape, resets `selectedLeafId`
  to the current deepest node (cancels branch mode).

**Input bar state machine**:

```
selectedLeafId is null OR points to an assistant message
  → text input enabled + "Send" button
  → (if selectedLeafId has a sibling with role assistant)
      also show "Regenerate ↺" button that generates another
      child of selectedLeafId's parent

selectedLeafId points to a user message
  → text input disabled
  → "Regenerate ↺" button that calls generate(chat_id, selectedLeafId)
```

**Regenerate flow**:
- Call `generate(chat_id, targetUserMsgId)`.
- The generate action creates a new empty assistant node immediately (SSE push).
- The auto-navigate `useEffect` from Step 1 fires and moves `selectedLeafId`
  to the new node.

**End state**: full branching UX.  No tree visualisation yet, but all
branching operations are possible.

---

### Step 3 — Tree flowchart panel

**Goal**: visual tree alongside the chat, click to navigate.

#### `epibloom/ui/src/components/ChatFlowchart.tsx` (new file)

Props:
```typescript
interface ChatFlowchartProps {
    messages: Record<string, ChatMessage>;
    selectedLeafId: string | null;
    onSelectNode: (nodeId: string) => void;
}
```

Internals:

1. **Layout computation** (pure function, recomputed on `messages` change):
   - Build a `children: Record<string, string[]>` map from `messages`.
   - Find root nodes (those with `parent_id === null`).
   - Recursive subtree-width pass (bottom-up): a leaf has width 1; an internal
     node has width = sum of children widths, minimum 1.
   - Recursive position pass (top-down): each node is centred over its
     children.  `x = (leftEdge + rightEdge) / 2`, `y = depth * ROW_HEIGHT`.
   - Output: `Record<string, {x: number, y: number}>`.

2. **SVG rendering**:
   - Outer `<svg>` with a `<g>` child that receives a CSS `transform` for
     pan/zoom.
   - Draw edges first (lines from parent bottom-centre to child top-centre).
   - Draw nodes on top (foreignObject or rect + text).
   - Nodes on `currentPath` (computed from `selectedLeafId` using `getPath`)
     get a bright ring.

3. **Interaction**:
   - `onMouseDown` / `onMouseMove` / `onMouseUp` on the `<svg>` for panning.
   - `onWheel` for zooming (scale around mouse position).
   - `onClick` on each node rect calls `onSelectNode(nodeId)`.

#### `epibloom/ui/src/components/ChatView.tsx`

- Import `ChatFlowchart`.
- Add `showFlowchart: boolean` state (default `true`).
- Restructure layout to a horizontal split: chat column (flex-1) + optional
  flowchart column (fixed ~320 px wide, with an inner scrollable SVG area).
- Toggle button (e.g. `⎇` icon) in the chat header shows/hides the flowchart.
- Pass `messages`, `selectedLeafId`, and `onSelectNode` to `ChatFlowchart`.
  `onSelectNode` just sets `selectedLeafId` (same as clicking a bubble, so the
  chat view auto-follows).

**End state**: full feature set — tree chat with branching, regenerate, branch
from history, and a clickable flowchart.
