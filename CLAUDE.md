# Epibloom Development Guide

## Project Vision

Epibloom is being developed as a text adventure game engine built on LLM conversations. The current architecture (traces, scripts, state objects) is designed to support both chat-style interactions and more complex game mechanics like branching narratives, character stats, inventory systems, and structured game events.

## Architecture

Epibloom has three core primitives that work together: **traces**, **state objects**, and **scripts**.

### Traces (`epibloom/src/trace.rs`, `epibloom/src/api/trace.rs`)

A trace is a record of a single LLM generation: the input messages (with `role` `system`/`user`/`assistant` and `content`), the assistant response (which may also carry `thinking` text for reasoning models), and metadata (UUID, created/modified timestamps). Traces are identified by UUID and persisted as JSON files.

Traces are **not** directly manipulated from Lua. They are created automatically each time a script calls `llm.generate()` (see Scripts below). Scripts receive back a handle for streaming the response; the trace itself lives entirely in Rust.

The UI subscribes to traces over SSE:
- `GET /api/trace` — streams metadata for all existing traces, then any newly created ones.
- `GET /api/trace/<id>` — streams the full message history of a trace, then live deltas (thinking chunks, content chunks, errors) as generation proceeds.

### State Objects (`epibloom/src/state.rs`, `epibloom/src/api/state.rs`)

A state object is a named, in-memory JSON value of any shape. Scripts read and update state objects; the UI subscribes to them. When a state object changes, subscribers receive a JSON Patch diff rather than the full value, keeping SSE payloads small even for large objects.

Scripts interact with state objects via `require("epibloom.state")`:
- `state.get(name)` — get or create a state object by name.
- `obj:read()` — read the current value.
- `obj:update(fn)` — atomic read-modify-write (the function receives the old value and returns the new one).

The UI subscribes via `GET /api/state?objects=name1,name2,...`, receiving each object's current value first, then live patches as they change.

State objects are not persisted. Scripts that need to persist data use `require("epibloom.filesystem")` to read/write JSON files in a sandboxed script data directory.

### Scripts (`epibloom/src/script.rs`, `epibloom/src/api/script.rs`)

Scripts are Lua files run server-side via mlua. They are the glue between the UI, state objects, and LLM generation. A script receives a JSON value as input and returns a JSON value as output.

Scripts run inside named **contexts**, each of which is a persistent Lua VM — state (loaded modules, globals) is retained across runs on the same context. Requests to the same context are serialised (FIFO queue); different contexts run concurrently.

Scripts are invoked via `POST /api/script/<context>/<path>` with a JSON body, and return a JSON result. The UI calls scripts for user actions (e.g. sending a chat message, triggering generation).

Built-in Lua modules available to scripts:
- `epibloom.llm` — start LLM generation and stream the response.
- `epibloom.state` — read/write named state objects (broadcasts patches to UI subscribers).
- `epibloom.filesystem` — sandboxed JSON file I/O (for persistence).
- `epibloom.log` — structured logging (levels: error/warn/info/debug/trace).
- `epibloom.uuid` — generate UUIDs.

The `epibloom.llm` module exposes a single function:
```lua
local handle = llm.generate(messages, backend, config?)
```
`messages` is a list of `{role, content}` tables covering the full conversation. `backend` is `"anthropic"` or `"featherless"`. `config` is an optional backend-specific config table (defaults are used if omitted). The call creates a trace internally, saves it to disk, starts generation, and returns a `GenerateHandle`. The handle has:
- `handle.trace_id` — UUID string of the newly created trace (useful for storing alongside a chat message so the UI can link to it).
- `handle()` — callable; returns the next `GenerateChunk` (`{type, delta}` or `{type, message}` for errors), or `nil` when done.
- `handle:wait()` — blocks until generation finishes, discarding remaining chunks; returns `(success, error_message)`.

#### Teal source files

Scripts are written in [Teal](https://github.com/teal-language/tl), a typed dialect of Lua. The file naming convention in `epibloom/scripts/` is:

- **`.tl`** — Teal source (edit these).
- **`.d.tl`** — Teal type-definition stubs for built-in modules (in `epibloom/scripts/epibloom/`). These tell the Teal type checker the types of the Rust-backed built-in functions. The `.d.tl` file for each built-in module is at `epibloom/scripts/epibloom/<module>.d.tl`.
- **`.lua`** — Generated Lua; produced by `tl gen` and committed alongside source. **Do not edit these directly.**

The Teal compiler lives at `epibloom/deps/tl/tl` and is invoked by the `just` commands.

Script source files live in `epibloom/scripts/` and are compiled from Teal to Lua in-place and then synced to `~/.config/epibloom/scripts/` by `just copy-scripts`.

### How they fit together (example: chat)

The chat system (`epibloom/scripts/chat.tl`) illustrates the pattern: the UI calls a script action → the script calls `llm.generate(messages, backend)` which creates a trace internally and starts streaming → the script updates a state object (chat messages) so the UI receives the new empty assistant node → the script iterates generation chunks and writes each one back into the state object → more patches reach the UI in real time → the final chat state is saved to disk via the filesystem module.

## Development Environment

The user should launch you in a Nix shell which should have packages like `just`, `cargo`/`rustc`, and `npm`.

If something you want to use is missing, let the user know (in case they forgot to launch you in the shell) and offer to add the packages to `flake.nix` under the dev shell if it is missing from there.

`just` is used as a command runner for the most frequently-used commands.

## Backend

In `epibloom/`, Rust + Rocket + mlua.

Lua script source in `epibloom/scripts/`.

### Commands

- `just check-server` - Ensure the project builds by running `cargo clippy`.
- `just check-scripts` - Type-check all `.tl` and `.lua` files in `epibloom/scripts/` using the bundled Teal compiler (`tl check`). Run this **before** `just copy-scripts` to catch type errors without overwriting deployed files.
- `just copy-scripts` - Compile every `.tl` file to a `.lua` file alongside it, then rsync the whole `epibloom/scripts/` tree to `~/.config/epibloom/scripts/` (with `--delete` so removed files are cleaned up).
- `just run` - Runs `just copy-scripts` and then starts the server. Run this in tmux so you can have it running while also testing API endpoints (see below).

### API Testing

To check backend API endpoints during development, use `http` (HTTPie), eg.

- `http :8000/api/trace` - Get a stream of what traces exist.
- `http :8000/api/trace/<trace-id>` - Get a stream of a specific trace with messages and message deltas.
- `http POST :8000/api/script/<script context>/<script path> value=1` - Run a script on a specified context, providing a JSON body and getting back the result of the run as JSON.

See files in `epibloom/src/api` for the API server code.

When the server is running, the OpenAPI spec should be available at http://127.0.0.1:8000/api-docs/openapi.json

## Frontend UI

In `epibloom/ui/`, React + TypeScript + Vite + Tailwind CSS.

### Key Characteristics

- TypeScript types in `src/types/api.ts` should match the OpenAPI schema. Currently manually specified, in the future should be generated from the OpenAPI spec.
- Custom React hooks manage SSE connections and state.

### Commands

- `just check-ui` - Runs `npm run lint` to run ESLint.
- `just serve` - Runs `npm run dev` to start development server (proxies `/api/*` to backend at `:8000`) - the user will already have this running in most cases.

## Gotchas

### Lua-JSON serialization

mlua has a serde-based serialization (Rust to Lua)/deserialization (Lua to Rust) functionality, which is used for easy converting of Rust structs to and from Lua, but primarily getting and setting the values of state objects with `epibloom.state`, and for saving to disk with `epibloom.filesystem`, both of which convert Lua values to and from freeform `serde_json::Json` values. This has a few quirks.

`epibloom/src/script.rs` has explicit options set in `SERIALIZE_OPTIONS` and `DESERIALIZE_OPTIONS`, which set:

- Serializing `None` and `()` unit to "null" is disabled. When enabled, this uses a Lua light userdata value of `0x0` as a stand-in for a key in an object existing but being null, since in Lua a nil value for a key means that key doesn't even exist. This can cause issues when the UI is expecting a field to be present but can be null, so such code needs to take into account that the key-value pair may be missing entirely.
- `encode_empty_tables_as_array` is enabled, since with an earlier representation of chat messages (`epibloom/scripts/chat.lua`), an empty messages array was encoded as an object instead of an array and caused issues in the frontend.
- `detect_mixed_tables` is enabled to ensure that object-like tables get deserialized into JSON properly. The mlua documentation states, "When this option is disabled, a table with a non-zero length (with one or more borders) will be always encoded as an array." For some reason this seemed to be happening even with a table with no integer keys.
