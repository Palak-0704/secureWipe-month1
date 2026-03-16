# SecureWipe Architecture

## Overview

SecureWipe is split into two operational planes:

1. The host-side control plane, which discovers devices, applies safety policy, prepares offline sessions, records evidence, and serves the operator UI.
2. The offline execution plane, which performs destructive work only after an explicit handoff and returns signed result evidence for reconciliation.

This split is intentional. The host application should remain safe for routine use on development and operator machines. High-risk actions are isolated behind explicit, offline-only flows.

## Major Components

### Rust core

`crates/core` provides:

- device discovery and confidence scoring
- wipe advisor logic
- wipe engine orchestration
- offline session lifecycle and storage
- API handlers for frontend and integration tests
- certificate generation and verification helpers

### CLI wrapper

`crates/cli` exposes command-line entry points over the same core logic for local and scripted workflows.

### Web frontend

`frontend/frontend-app` is a React + Vite application that calls the local API server. It supports device review, wipe advice, offline session creation, USB preparation, result ingestion, and certificate access.

## Runtime Boundaries

### Host-side API server

The API server binds locally by default and acts as the control surface. It is responsible for:

- enumerating target devices
- blocking unsafe device selection
- requiring explicit session creation before offline wipe execution
- recording manifests, confirmations, logs, and final evidence under the configured data directory

### Offline session handoff

An offline session contains the minimum information needed to perform a controlled wipe outside the normal host runtime. Session manifests and bootable USB preparation are staged via storage helpers rooted under `SECUREWIPE_DATA_DIR`.

### Offline execution

Destructive operations occur only in the offline path. The host-side code is simulation-first by default. Real host-side USB provisioning is separately guarded and fail-closed.

## Storage Model

All mutable runtime artifacts now resolve through `SECUREWIPE_DATA_DIR`.

This includes:

- wipe history
- confirmations
- offline session manifests
- offline result payloads
- USB handoff artifacts

This design avoids accidental repository pollution during tests and makes operator deployments easier to isolate.

## Safety Controls

SecureWipe is designed to fail closed.

Key controls include:

- system disk blocking
- strict target selection and allowlist support
- detection-confidence gating for uncertain classification
- simulation-first default behavior
- explicit break-glass and allowlist gating for any real USB provisioning path

These controls are enforced in backend handlers before state transitions occur.

## Frontend Integration

The frontend is intentionally coupled to backend response shapes for the offline wizard flow.

Current operator path:

1. inspect detected devices
2. request advisor guidance
3. open offline flow for a selected target
4. create session
5. prepare USB handoff package
6. execute offline process
7. ingest result
8. review/download certificate evidence

## Testing Strategy

The backend uses unit and integration coverage with fixture-driven device detection overrides to avoid touching host disks. Integration tests isolate mutable outputs into temporary data roots.

The frontend uses linting, build validation, and component tests for the offline wizard and app-shell flows.

## Design Constraints

- Do not perform destructive host actions by default.
- Keep cross-platform device logic behind platform-specific modules.
- Preserve additive API evolution where possible because frontend and backend are coupled by payload shape.
- Prefer explicit policy gates over hidden environment-based behavior.
