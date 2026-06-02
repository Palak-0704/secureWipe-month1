# SecureWipe (Current Project Overview)

## What This Repo Is
SecureWipe is a safety-first secure data sanitization platform with:
- Rust backend API + wipe/advisor engine
- Rust CLI tools (including offline runtime)
- React + Vite operator UI
- Offline-session workflow (prepare → boot offline → execute → ingest → certificate)

Default behavior is simulation-first. Real/destructive behaviors are feature-gated and guarded.

## Workspace Layout (Important Paths)
- `Month1-Submission/` (primary application)
  - `crates/core/` (API server, device detection, wipe engine, advisor, certificates)
  - `crates/cli/` (CLI tools)
  - `frontend/frontend-app/` (React + Vite)
  - `scripts/` (ISO + USB provisioning helpers)
  - `data/`, `templates/`, `locales/` (runtime assets; many paths assume this working directory)

## Run Locally (Dev)
From `Month1-Submission/`:

### Backend API
- Build/test:
  - `cargo build`
  - `cargo test`
- Run API server (chatbot feature-gated):
  - `cargo run --bin main_api --features groq_api`
- Default API URL: `http://127.0.0.1:8080`
- CORS: intended for `localhost/127.0.0.1:5173`

### Frontend
From `Month1-Submission/frontend/frontend-app/`:
- `npm install`
- `npm run dev`
- Dev URL: `http://localhost:5173`
- Backend base URL is controlled by `VITE_API_BASE_URL` (defaults to `http://127.0.0.1:8080`).

## Safety Model (Short)
- Simulation-first by default.
- Destructive paths require explicit enabling (feature flags / env guards).
- Strong guard rails: protected/system target blocking, strict targeting policies, device identity re-validation, and confirmation state machines.

## Key Feature Flags / Modes
- Rust feature flags:
  - `groq_api` (chatbot integration)
  - `real_scan`, `real_erase` (non-default; controlled-lab only)
- Common env assumptions:
  - Working directory often expected to be `Month1-Submission/` (assets resolve via relative paths).

## API Surface (High Level)
Core endpoints include:
- `GET /api/devices`
- `POST /api/advisor/recommend`
- `POST /api/wipe/session/create`
- `GET /api/usb/devices`
- `POST /api/usb/prepare`
- `POST /api/offline/wipe/execute`
- `POST /api/offline/result/ingest`
- Certificate endpoints: `GET /api/certificate/:id/review`, `GET /api/certificate/:id/pdf`
