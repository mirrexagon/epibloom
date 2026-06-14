# Epibloom Development Guide

## Project Vision

Epibloom is being developed as a text adventure game engine built on LLM conversations. The current architecture (traces, scripts, state objects) is designed to support both chat-style interactions and more complex game mechanics like branching narratives, character stats, inventory systems, and structured game events.

## Development Environment

The user should launch you in a Nix shell which should have packages like `just`, `cargo`/`rustc`, and `npm`.

If something you want to use is missing, let the user know (in case they forgot to launch you in the shell) and offer to add the packages to `flake.nix` under the dev shell if it is missing from there.

`just` is used as a command runner for the most frequently-used commands.

## Backend

In `epibloom/`, Rust + Rocket + mlua.

Lua script source in `epibloom/scripts/`.

### Architecture

Epibloom's backend has two core primitives, state objects and scripts.

#### State Objects (`epibloom/src/state.rs`, `epibloom/src/api/state.rs`)

A state object is a named, in-memory JSON value of any shape. Scripts read and update state objects, and the UI subscribes to them. When a state object changes, subscribers receive a JSON Patch diff.

Scripts interact with state objects via `require("epibloom.state")`:
- `state.get(name)` — get or create a state object by name.
- `obj:read()` — read the current value.
- `obj:update(fn)` — atomic read-modify-write (the function receives the old value and returns the new one).

The UI subscribes via `GET /api/state?objects=name1,name2,...`, receiving each object's current value first, then live patches as they change.

State objects with names prefixed with `persist:` are automatically persisted.

#### Scripts (`epibloom/src/script[.rs]`, `epibloom/src/api/script.rs`)

Scripts are Lua files run server-side via mlua. They are the glue between the UI, state objects, and LLM generation. A script receives a JSON value as input and returns a JSON value as output.

Scripts run inside named contexts which are Lua states that persist as long as the backend is running, Loaded modules, globals, etc. are retained across runs on the same context. Requests to the same context are serialised (FIFO queue); different contexts run concurrently.

Scripts are invoked via `POST /api/script/<context>/<script path relative to scripts directory>` with a JSON body, and return a JSON result. The idea is that the UI calls scripts to do things (eg. sending a chat message, triggering generation).

Built-in Lua modules available to scripts:
- `epibloom.llm`: LLM-related functions, primarily getting LLM responses.
- `epibloom.state`: Get and manipulate state objects.
- `epibloom.log`: Logging using Rust's `log` crate.
- `epibloom.uuid`: Generate UUIDs.
- `epibloom.time`: Functions related to date and time.

### Commands

- `just check-server`: Ensure the project builds by running `cargo clippy`.
- `just copy-scripts`: `rsync` the whole `epibloom/scripts/` tree to `~/.config/epibloom/scripts/` (with `--delete` so removed files are cleaned up).
- `just run-server` - Runs `just copy-scripts` and then starts the server. Run this in tmux so you can have it running while also testing API endpoints (see below).

### API Testing

To check backend API endpoints during development, use `http` (HTTPie), eg.

- `http :8000/api/state?objects=chat:123:messages,player:456:stats`: Get a stream of the full value and then updates to the specified state objects.
- `http POST :8000/api/script/<script context>/<script path> value=1`: Run a script on a specified context, providing a JSON body and getting back the result of the run as JSON.

See files in `epibloom/src/api` for the API server code.

When the server is running, the OpenAPI spec should be available at http://127.0.0.1:8000/api-docs/openapi.json

## Frontend UI

In `epibloom/ui/`, React + TypeScript + Vite + Tailwind CSS.

### Key Characteristics

- TypeScript types in `src/types/api.ts` should match the OpenAPI schema. Currently manually specified, in the future should be generated from the OpenAPI spec.
- Custom React hooks manage SSE connections and state.

### Commands

- `just check-ui` - Runs `npm run lint` to run ESLint.
- `just serve-ui` - Runs `npm run dev` to start development server (proxies `/api/*` to backend at `:8000`) - the user will already have this running in most cases.

## Gotchas

### Lua-JSON serialization

mlua has serde-based serialization (Rust to Lua)/deserialization (Lua to Rust) functionality, which is used for easy converting of Rust structs to and from Lua. This is used primarily for getting and setting the values of state objects with `epibloom.state` which converts Lua values to and from freeform `serde_json::Value` s. This has a few quirks.

`epibloom/src/script.rs` has explicit options set in `SERIALIZE_OPTIONS` and `DESERIALIZE_OPTIONS`, which set:

- Serializing `None` and `()` unit to "null" is disabled. When enabled, this uses a Lua light userdata value of `0x0` as a stand-in for a key in an object existing but being null, since in Lua a nil value for a key means that key doesn't even exist. This can cause issues when the UI is expecting a field to be present but can be null, so such code needs to take into account that the key-value pair may be missing entirely.
- `encode_empty_tables_as_array` is enabled, since an array becoming empty would otherwise make it turn into an empty object, which can be confusing to the frontend.
- `detect_mixed_tables` is enabled to ensure that object-like tables get deserialized into JSON properly. The mlua documentation states, "When this option is disabled, a table with a non-zero length (with one or more borders) will be always encoded as an array." For some reason this seemed to be happening even with a table with no integer keys.
