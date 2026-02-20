# Epibloom Development Guide

## Project Vision

Epibloom is being developed as a text adventure game engine built on LLM conversations. The current architecture (traces, scripts, state objects) is designed to support both chat-style interactions and more complex game mechanics like branching narratives, character stats, inventory systems, and structured game events.

## Architecture

Epibloom has three core primitives that work together: **traces**, **state objects**, and **scripts**.

### Traces (`epibloom/src/trace.rs`, `epibloom/src/api/trace.rs`)

A trace is an LLM conversation: an ordered list of messages, each with a `role` (`system`/`user`/`assistant`), message `content`, and possible other items like `thinking` for reasoning text. Traces are identified by UUID and persisted as JSON files.

Generation is initiated from Lua via `trace:generate(backend)`, which returns an iterable handle that yields chunks with things like message content or any error that occurs as they stream in from the LLM backend.

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
- `epibloom.trace` — create traces and start LLM generation.
- `epibloom.state` — read/write named state objects (broadcasts patches to UI subscribers).
- `epibloom.filesystem` — sandboxed JSON file I/O (for persistence).
- `epibloom.log` — structured logging (levels: error/warn/info/debug/trace).
- `epibloom.uuid` — generate UUIDs.

Each built-in Lua module has a Lua Language Server meta/definition file which defines the types and function type information on the Lua side. These are in `epibloom/scripts/meta`.

Script source files live in `epibloom/scripts/` and are deployed to `~/.config/epibloom/scripts/` by `just run`.

### How they fit together (example: chat)

The chat system (`epibloom/scripts/chat.lua`) illustrates the pattern: the UI calls a script action → the script updates a state object (chat messages) → state patches stream to the UI in real time → the script also creates a trace and drives LLM generation → generation chunks are written back into the state object → more patches reach the UI → the final chat is saved to disk via the filesystem module.

## Development Environment

The user should launch you in a Nix shell which should have packages like `just`, `cargo`/`rustc`, and `npm`.

If something you want to use is missing, let the user know (in case they forgot to launch you in the shell) and offer to add the packages to `flake.nix` under the dev shell if it is missing from there.

`just` is used as a command runner for the most frequently-used commands.

## Backend

In `epibloom/`, Rust + Rocket + mlua.

Lua script source in `epibloom/scripts/`.

### Commands

- `just check-server` - Ensure the project builds by running `cargo clippy`.
- `just check-scripts` - Check the Lua scripts using Lua Language Server.
- `just run` - Copy scripts from `epibloom/scripts/` to `~/.config/epibloom/scripts` and then run the server. Run this in tmux so you can have it running while also testing API endpoints (see below).

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
