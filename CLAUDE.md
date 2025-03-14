# Epibloom Development Guide

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
- **File Naming**: PascalCase for components (Button.res, App.res)
- **Interface Files**: Use .resi files for type signatures
- **Components**: Use `@react.component` annotation with `make` function
- **Types**: Explicit type annotations in .resi files
- **Strings**: Use `React.string("text")` or `` `${value->toString}` ``
- **Functions**: camelCase naming, pipe operator (`->`) for chaining
- **Pattern Matching**: Exhaustive with `switch` for optional values
- **Styling**: Use Tailwind CSS classes directly in JSX

### Project Structure
- React components in src/
- Component structure: implementation (.res) + interface (.resi)
- Auto-opened modules: RescriptCore (-open compiler flag)
- ESModule output format with .res.mjs extension

Remember to run `npm run res:build` after making changes to ReScript files.

## Interface Backend

To be added.
