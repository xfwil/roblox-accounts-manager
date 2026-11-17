# RAM — Roblox Account Manager

<p align="center">
  <img src="src-tauri/app-icon.png" alt="RAM" width="128" />
</p>

<p align="center">
  Desktop app for managing multiple Roblox accounts.<br/>
  Built with <strong>Tauri 2</strong>, <strong>React</strong>, and <strong>Rust</strong>.
</p>

---

## Features

| Category | Details |
|----------|---------|
| **Account Management** | Add, remove, organize accounts with groups and aliases |
| **Multi-Instance** | Run multiple Roblox clients simultaneously (bypasses singleton mutex) |
| **Game Joining** | Join by Place ID, private server link, or share link with server browser |
| **Safety Dashboard** | Account standing status (All good / Caution / At risk / Critical) and violation count |
| **Account Utilities** | Change password, email, display name, description, privacy settings |
| **Import / Export** | Import via cookies or username/password/cookie combo; export for backup |
| **Presence Tracking** | Real-time online / in-game / in-studio status |
| **Process Watcher** | Track running Roblox instances per account |
| **Web API** | Optional local HTTP API for external tool integration |
| **Nexus WebSocket** | Real-time communication channel for external clients |
| **Login via WebView** | Browser-based login flow for secure cookie capture |
| **Encrypted Storage** | AES-256 encrypted credentials |
| **System Tray** | Minimize to tray with quick-access menu |
| **Theming** | Custom theme support |

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Runtime | [Tauri 2](https://v2.tauri.app/) |
| Backend | Rust |
| Frontend | React 19 + TypeScript |
| Build Tool | Vite |
| Styling | CSS |

## Setup

```bash
npm install
npx tauri dev      # development
npx tauri build    # production
```

Requires [Node.js](https://nodejs.org/) 18+, [Rust](https://www.rust-lang.org/tools/install) stable, and [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## License

Private — All rights reserved.
