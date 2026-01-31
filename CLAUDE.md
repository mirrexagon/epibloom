# Epibloom Development Guide

## Interface Backend

In `epibloom/`, Rust + Rocket.

### Commands

- `cargo clippy` - Ensure the project builds, including extra lints.

## API Testing

To check backend API endpoints during development, use `http` (HTTPie), eg.

- `http :8000/api/chat` - Get list of all chats
- `http :8000/api/chat/<chat-id>` - Get specific chat with messages
- `http POST :8000/api/chat/<chat-id>/message role=user text='Message text'` - Add message to chat (HTTPie automatically serializes key=value pairs as JSON and sets Content-Type)

See files in `epibloom/src/api` for the API server code.
