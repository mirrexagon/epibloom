# Epibloom UI

React frontend for Epibloom trace viewer.

## Development

1. Install dependencies:
   ```bash
   npm install
   ```

2. Make sure the backend is running on `http://127.0.0.1:8000`

3. Start the dev server:
   ```bash
   npm run dev
   ```

4. Open http://localhost:5173 in your browser

## Building for Production

```bash
npm run build
```

This creates static files in `dist/` that can be served by the Rocket backend.

## Features

- **Real-time trace list**: Connects to `/api/trace/` SSE endpoint and shows all traces
- **Live trace view**: Shows messages as they stream in from `/api/trace/{id}`
- **Message rendering**: Displays user, assistant, and system messages with role indicators
- **Thinking content**: Collapsible thinking sections for assistant messages
- **Connection status**: Visual indicators for SSE connection status
