# RAM — Roblox Account Manager

<p align="center">
  <img src="logo-ram.png" alt="RAM Logo" width="128" />
</p>

Desktop application for managing multiple Roblox accounts. Built with Tauri 2, React, and Rust.

## Features

- **Multi-Account Management** — Add, remove, and organize Roblox accounts with group support
- **Multi-Instance** — Run multiple Roblox clients simultaneously (bypasses singleton mutex)
- **Game Joining** — Join games by Place ID, private server links, or share links with server browser
- **Account Utilities** — Change password, email, display name, description, and view privacy settings
- **Import/Export** — Import accounts via cookies or username/password/cookie combo; export for backup
- **Presence Tracking** — Real-time online status for all managed accounts
- **Process Watcher** — Track running Roblox instances per account
- **Web API** — Optional local HTTP API for external tool integration
- **Nexus WebSocket** — Optional WebSocket server for real-time communication with external clients
- **Login via WebView** — Browser-based login flow for adding accounts securely
- **Encrypted Storage** — Account credentials encrypted with AES-256 (key stored in app data directory)
- **System Tray** — Minimize to tray with quick access menu
- **Theming** — Custom theme support

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Runtime | [Tauri 2](https://v2.tauri.app/) |
| Backend | Rust |
| Frontend | React 19 + TypeScript |
| Build Tool | Vite 8 |
| Styling | CSS (no framework) |

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri CLI prerequisites](https://v2.tauri.app/start/prerequisites/)

## Setup

```bash
# Install dependencies
npm install

# Run in development mode
npx tauri dev

# Build for production
npx tauri build
```

## Project Structure

```
ram-manager/
├── src/                    # React frontend
│   ├── api/                # Tauri command bindings
│   ├── components/         # UI components
│   ├── hooks/              # React hooks
│   └── types/              # TypeScript types
├── src-tauri/              # Rust backend
│   └── src/
│       ├── commands/       # Tauri IPC command handlers
│       ├── models/         # Data models
│       ├── roblox/         # Roblox API client
│       ├── crypto.rs       # AES encryption
│       ├── nexus.rs        # WebSocket server
│       ├── storage.rs      # Encrypted account store
│       ├── watcher.rs      # Process tracker
│       └── webapi.rs       # HTTP API server
└── public/                 # Static assets
```

## License

Private — All rights reserved.
