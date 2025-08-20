# Epibloom Development Guide

## Interface Backend

In `epibloom/`, Rust + Rocket.

### Commands

- `cargo build` - Build Rust project
- `cargo run` - Run Rust project

## Interface Frontend

In `epibloom/ui/`, ReScript + React + TailwindCSS.

### Commands

- `npm run res:build` - Build ReScript files
- `npm run res:clean` - Clean ReScript build artifacts
- `npm run build` - Build for production
- `npm run res:dev` - Watch ReScript files and rebuild on changes
- `npm run dev` - Start development server with hot reloading
- `npm run preview` - Preview production build locally

Note: Don't use commands like `npm run res:dev` and `npm run dev` as the user will have this running already separately. Just `npm run res:build` to check for errors after making changes to ReScript files.

### Code Style Guidelines

- File Naming: PascalCase for source files, including components (`Button.res`, `App.res`)
- No Interface Files: Don't create `.resi` interface files on this project
- Components: Use `@react.component` annotation with `make` function
- Types: Explicit type annotations in .resi files
- Strings in React components: Explicit `React.string("text")` is required
- Functions: camelCase naming, pipe operator (`->`) for chaining
- Pattern Matching: Exhaustive with `switch` for optional values
- Styling: Use Tailwind CSS classes directly in JSX

### Project Structure

- Interface backend API interaction in `src/Api.res`
- React components in `src/`
- Component structure: Implementation (`.res`) + interface (`.resi`)
- Modules: No need to explicitly `open` modules in other files, just refer to them by name
- ESModule output format with `.res.mjs` extension

Remember to run `npm run res:build` after making changes to ReScript files, to check for errors.

## API Testing

To check backend API endpoints during development, use `http` (HTTPie), eg.

- `http :8000/api/chat` - Get list of all chats
- `http :8000/api/chat/<chat-id>` - Get specific chat with messages
- `http POST :8000/api/chat/<chat-id>/message role=user text='Message text'` - Add message to chat (HTTPie automatically serializes key=value pairs as JSON and sets Content-Type)

See files in `epibloom/src/api` for the API server code.

The UI uses a Vite proxy to forward `/api/*` requests to the backend at `http://localhost:8000`, avoiding CORS issues.

