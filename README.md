# AIPET

AIPET is a local-first AI desktop companion built with Tauri, Vue 3, TypeScript, and Rust.

The project explores a desktop pet that can be shaped by the user, stay present on the desktop, and join lightweight everyday activities without depending on invasive screen reading.

## What It Does

- Provides a desktop pet surface for presence, movement, and lightweight interaction.
- Supports chat through OpenAI-compatible LLM providers.
- Uses user-owned persona files as the basis for personality shaping.
- Includes task, reminder, and focus-session workflows inside the desktop shell.
- Keeps private runtime data local by default.

## Privacy Direction

AIPET is designed around local-first behavior:

- User data is not uploaded by default.
- Secrets are stored locally and encrypted through the Windows security stack.
- The app does not need screen capture, window-title reading, microphone access, or clipboard reading for its core companion behavior.
- LLM usage is routed through user-configured providers.

## Tech Stack

- Desktop: Tauri 2.x
- Frontend: Vue 3, TypeScript, Pinia, Vite
- Runtime: Rust
- Rendering: Three.js, `@pixiv/three-vrm`
- Storage: SQLite
- Package manager: pnpm

## Development

Prerequisites:

- Node.js 20+
- pnpm 9+
- Rust toolchain compatible with Tauri 2.x
- Windows with WebView2

Install dependencies:

```powershell
pnpm install
```

Run the desktop app in development mode:

```powershell
pnpm tauri:dev
```

Useful checks:

```powershell
pnpm typecheck
pnpm lint
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

Build:

```powershell
pnpm tauri:build
```

## Status

AIPET is an active personal MVP project. The public repository is kept focused on source code and buildable artifacts; planning notes, private development logs, and agent workflow files are intentionally kept out of the public tree.
