# SecureWipe Product TODO (Code-Based)

Last updated: 2026-03-16
Scope: End-to-end product (not just hackathon demo)
Source of truth: current code in crates/core, crates/cli, frontend/frontend-app

## 1. Current Baseline (Already Implemented)
- [x] Device scan and listing API (`/api/devices`) with platform-aware detection.
- [x] Advisor API (`/api/advisor/recommend`) with compliance-aware rule logic.
- [x] Chatbot API (`/api/chatbot`) with Groq integration behind feature flag.
- [x] Wipe session/log persistence in JSON history.
- [x] Preflight and handoff scaffolding endpoints added:
  - `/api/preflight/mvp`
  - `/api/usb/devices`
  - `/api/wipe/session/create`
  - `/api/usb/prepare`
- [x] Certificate endpoint now returns structured JSON + SHA256 digest.

## 2. P0 (Launch Blockers) - Must Complete First

### Security and Safety
- [~] Revoke and rotate any exposed API key, remove secrets from git history, add `.env.example` (playbook added: `docs/SECRETS_ROTATION_PLAYBOOK.md`; key rotation/history rewrite still operational tasks).
- [x] Enforce multi-step wipe confirmation on backend (init, risk, final ERASE token).
- [x] Enforce safety policy for unsupported scenarios (multi-disk, unsupported host profile).
- [x] Enforce feature gates for destructive operations (`real_erase`) at API layer.
- [x] Enforce protected system-disk blocking in session creation and offline destructive execution paths.
- [x] Enforce strict target policy: destructive flows accept only removable or explicitly allowlisted devices when `SECUREWIPE_STRICT_TARGETING` is enabled (default on).
- [x] Enforce production startup fail-safe: API boot is blocked when strict targeting is disabled unless explicit emergency override is set.
- [x] Enforce fail-closed real USB provisioning policy: requires explicit break-glass (`SECUREWIPE_USB_REAL_BREAKGLASS=1`) and explicit removable USB allowlist (`SECUREWIPE_USB_REAL_ALLOWLIST`).

### Core Wipe Correctness
- [~] Replace wipe simulation with real offline-compatible wipe executor abstraction (command-based offline executor is implemented behind `real_erase` with runtime guards; platform-native erase strategies remain optional integrations).
- [x] Add deterministic wipe state machine: `created -> usb_prepared -> reboot_pending -> offline_started -> wiping -> verified -> certified -> completed/failed`.
- [x] Implement resilient progress tracking for each wipe session (phase-based progress with session status and stream support).
- [x] Add device identity lock (model + serial + size) and mismatch rejection before destructive step.

### API Reliability
- [~] Add typed error model + proper HTTP statuses (400/403/404/409/422/500).
- [~] Add request validation for all POST payloads and path params.
- [x] Add rate limiting on high-risk endpoints (`/api/wipe/*`, `/api/usb/*`).

## 3. P1 (Critical Product Completion)

### Offline Runtime and USB Handoff
- [~] Build actual bootable USB image flow (provisioning adapter module extracted and `/api/usb/prepare` now routes through it; production imaging command/runtime hardening remains).
- [x] Define and version `wipe_manifest` schema for offline runtime.
- [x] Add offline runtime result ingestion endpoint and session reconciliation.
- [x] Add recovery/resume markers on USB for interruption handling.

### Verification and Certificate Trust
- [~] Implement real post-wipe verification policy (typed completion/verification contract validation added; real media sampling and proof criteria still pending).
- [~] Add signed JSON certificate (public-key verifiable signature added; key management still needs production hardening).
- [x] Add PDF certificate generation from JSON for user/download workflows.
- [x] Add certificate verification endpoint.
- [x] Add backend certificate review endpoint for downstream distribution checks.

### Backend Hardening
- [x] Move config to environment-driven model (CORS, bind host, limits, keys, modes).
- [x] Add response/request size limits and timeout guards.
- [x] Add structured logs with operation/session IDs.

### Test Coverage
- [~] Add integration tests for all main API routes.
- [x] Add end-to-end backend tests for wipe state machine transitions.
- [x] Add failure-path tests (USB missing, disk mismatch, interrupted process).

## 4. P2 (Production Readiness and Scale)

### Frontend-Backend Linking Status
- [~] Frontend-backend linking is partially enabled for offline session create/prepare/execute/ingest flows; broader polish and coverage remain.
- [ ] Enable full frontend linking only after backend E2E checklist in Sections 2 and 3 is complete.

### Frontend (after backend stabilization)
- [~] Connect frontend to new session-state APIs and confirmations.
- [~] Add USB handoff wizard with clear boot instructions and recovery steps.
- [x] Add certificate viewer/download and verification status UI.
- [x] Fix all frontend lint errors and add error boundary flow.

### DevOps and Release
- [x] Add CI pipeline: build + tests + lint + security audit checks.
- [x] Add release artifact packaging with checksum generation in CI for the current supported CLI target.
- [x] Add centralized observability plan (metrics, error tracking, alerts).
- [x] Run local release rehearsal (tests/lint/build + artifact bundle + checksum verification).

### Documentation and Compliance
- [x] Update API spec (OpenAPI).
- [x] Add architecture docs for in-app vs offline runtime split.
- [x] Add compatibility matrix and safety claims wording.
- [x] Add privacy/security policy and vulnerability disclosure process.

## 5. Suggested Execution Order (One-by-One)
1. P0 safety and secrets
2. P0 wipe state machine and real API error model
3. P0 progress/session identity lock
4. P1 offline runtime contract + result ingestion
5. P1 verification and signed certificate
6. P1 tests and hardening
7. P2 frontend linking and full integration (only after backend E2E completion)
8. P2 release and compliance docs

## 6. Definition of Done (Product)
- [x] Destructive wipe can only happen from offline runtime with final ERASE confirmation.
- [x] Wrong-disk protections are enforced by backend and verified in tests.
- [x] Certificate is cryptographically verifiable and tied to session/device identity, with JSON review and PDF export routes.
- [x] Full session survives restart/interruption and can recover safely.
- [x] CI passes for backend + frontend, with documented supported environments.
- [x] Release packaging includes checksum generation for published CI artifacts.
