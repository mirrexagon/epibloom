# Epibloom Development Guide

## Project Vision

Epibloom is being developed as a text adventure game engine built on LLM conversations. The current architecture (traces, scripts, state objects) is designed to support both chat-style interactions and more complex game mechanics like branching narratives, character stats, inventory systems, and structured game events.

## Development Environment

The user should launch you in a Nix shell which should have packages like `just`, `cargo`/`rustc`, and `npm`.

If something you want to use is missing, let the user know (in case they forgot to launch you in the shell) and offer to add the packages to `flake.nix` under the dev shell if it is missing from there.

`just` is used as a command runner for the most frequently-used commands.

## Backend

In `epibloom/`, Rust + Rocket + mlua.

### Commands

- `just check` - Ensure the project builds by running `cargo clippy`.
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
