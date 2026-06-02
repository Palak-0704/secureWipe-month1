# Project Guidelines

## Scope
- Primary application code is under Month1-Submission.
- Workspace root also contains reference and exported files; avoid changing root snapshots unless explicitly requested.

## Architecture
- Rust workspace:
  - crates/core: device detection, wipe engine, advisor logic, API server.
  - crates/cli: command-line UX that wraps core capabilities.
- Frontend:
  - frontend/frontend-app: React + Vite app calling the Rust API.
- Canonical project references (kept compact):
  - references/PROJECT_OVERVIEW.md
  - references/VENTOY_USB_AND_ISO_RUNBOOK.md

## Build and Test
- Rust (from Month1-Submission):
  - cargo build
  - cargo test
  - cargo run --bin main_api --features groq_api
- Frontend (from Month1-Submission/frontend/frontend-app):
  - npm install
  - npm run dev
  - npm run build
  - npm run lint

## Conventions
- Keep wipe operations simulation-first unless a task explicitly requires real erase behavior.
- Preserve feature-gated behavior in Rust:
  - groq_api for chatbot integration
  - real_scan and real_erase for non-default paths
- USB boot/provisioning is Ventoy-first; do not reintroduce WinPE/bootsect/manual boot-file stitching.
- API and frontend are coupled by endpoint shapes; when changing API contracts, update frontend calls in src/App.jsx and related components.
- Device model fields are used across advisor, API, and tests. Prefer additive changes over breaking renames.
- Keep cross-platform logic behind cfg gates in device/platform code.

## Environment and Pitfalls
- Chatbot paths require GROQ_API_KEY (and optionally GROQ_API_ENDPOINT).
- API server binds to 127.0.0.1:8080 and currently allows CORS only for localhost/127.0.0.1 on port 5173.
- Several runtime paths assume Month1-Submission as working directory (data/, locales/, templates/). If startup context changes, path handling may need updates.

## Key Files
- crates/core/src/devices.rs
- crates/core/src/api.rs
- crates/core/src/main_api.rs
- tests/integration.rs
- frontend/frontend-app/src/App.jsx
- frontend/frontend-app/src/lib/api.js
- scripts/create_securewipe_iso.ps1
- scripts/usb_provision_enhanced.ps1
