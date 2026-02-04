# Epibloom Development Guide

## Development Environment

The user should launch you in a Nix shell which should have packages like `cargo/rustc` and `npm`.

If something you want to use is missing, let the user know (in case they forgot to launch you in the shell) and offer to add the packages to `flake.nix` under the dev shell.

## Interface Backend

In `epibloom/`, Rust + Rocket.

### Commands

- `cargo clippy` - Ensure the project builds, including extra lints.

### API Testing

To check backend API endpoints during development, use `http` (HTTPie), eg.

- `http :8000/api/trace` - Get a stream of what traces exist.
- `http :8000/api/trace/<trace-id>` - Get a stream of a specific trace with messages and message deltas.
- `http POST :8000/api/script/<script context>/<script path> value=1` - Run a script on a specified context, providing a JSON body and getting back the result of the run as JSON.

See files in `epibloom/src/api` for the API server code.
