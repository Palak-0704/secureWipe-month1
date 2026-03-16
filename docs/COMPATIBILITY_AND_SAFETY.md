# Compatibility And Safety

## Scope

This document defines the current compatibility posture and the safety claims that SecureWipe is designed to uphold.

## Compatibility

### Supported runtime shape

- Local Rust API server on Windows, Linux, and macOS development environments
- React frontend served through Vite during development
- Offline-session workflow for destructive execution handoff

### Development assumptions

- The API server is started from the `Month1-Submission` directory so relative asset lookups for locales, templates, and bundled data remain valid.
- The frontend expects the local API at `127.0.0.1:8080` unless reconfigured.
- Local development CORS is restricted to localhost Vite origins by default.

### Storage compatibility

Runtime artifacts are written beneath `SECUREWIPE_DATA_DIR` when set. If not set, the application falls back to the project data directory layout.

### Feature-gated behavior

- `groq_api` enables chatbot-backed features.
- `real_scan` and `real_erase` remain non-default and should only be enabled in controlled environments.

## Safety Claims

### Safe-by-default claim

SecureWipe is intended to be safe to run on an operator or developer workstation because destructive behavior is not the default path.

### Host protection claim

The application should not erase the host system disk during normal operation. Device selection and offline-session creation are guarded by system-disk and confidence checks.

### Fail-closed claim

If the system cannot establish that a target is acceptable, SecureWipe should reject the request rather than infer operator intent.

### Real USB provisioning claim

Real USB provisioning is blocked by default. Enabling it requires explicit runtime policy flags, a break-glass confirmation, and an allowlist that names the intended removable device.

## Non-Claims

SecureWipe does not currently claim:

- formal certification against any destruction standard
- guaranteed success on every storage controller or firmware combination
- safety when operators intentionally disable guards or supply incorrect allowlists
- safe operation when run outside controlled working-directory and environment assumptions without validation

## Operational Requirements

Operators should:

- verify the selected target device before creating a session
- keep `SECUREWIPE_STRICT_TARGETING` enabled
- use isolated removable media for offline handoff
- set `SECUREWIPE_DATA_DIR` to a dedicated writable location in production-like environments
- treat any real provisioning or erase feature as lab-only until separately validated

## Recommended Validation Before Broader Use

- run backend test suite from the project root
- run frontend lint, tests, and production build
- verify offline flow with fixture devices before any lab hardware trial
- confirm storage output goes to a dedicated non-repository data root
