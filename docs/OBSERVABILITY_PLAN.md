# Centralized Observability Plan

## Scope

This plan defines a minimum viable observability model for SecureWipe that improves operator confidence without changing destructive behavior.

It focuses on:

- metrics (service and workflow health)
- structured logs (traceable event records)
- error tracking and alert routing

## Objectives

1. Detect API degradation and workflow failures early.
2. Provide enough context to investigate incidents quickly.
3. Preserve auditability for offline wipe lifecycle events.
4. Keep local-first deployment support while allowing centralized collection.

## Signal Model

### Metrics

Implement and export these baseline counters/gauges/histograms:

- `securewipe_http_requests_total{route,method,status}`
- `securewipe_http_request_duration_ms{route,method}`
- `securewipe_active_sessions{phase}`
- `securewipe_session_transitions_total{from_phase,to_phase}`
- `securewipe_offline_ingest_failures_total{code}`
- `securewipe_certificate_review_attention_total{reason}`
- `securewipe_usb_prepare_total{mode,status}`
- `securewipe_rate_limit_rejections_total{route}`

### Logs

All high-risk operations should emit structured JSON logs with these common fields:

- `timestamp`
- `level`
- `event`
- `operation_id`
- `session_id` (when available)
- `wipe_id` (when available)
- `phase`
- `code` (for typed errors)
- `detail`

Do not log secrets, full certificate private material, or external API keys.

### Error Tracking

Capture server exceptions and classified conflict/error responses into a centralized sink with deduplication by `(code, route, release)`.

Minimum tracked categories:

- startup safety gate failures
- invalid state transitions
- offline result ingest validation failures
- certificate verification/review failures
- USB provisioning policy rejections in real mode

## Collection Architecture

### Local-First (default)

- API logs written locally (stdout + optional file sink).
- Metrics endpoint exposed locally (for example, Prometheus scrape endpoint).
- Frontend error reports kept local in development.

### Centralized (production-like)

- Forward structured logs to a centralized backend (for example, OpenSearch, Loki, or Azure Monitor).
- Scrape/push metrics to a centralized TSDB (for example, Prometheus-compatible store).
- Route alerts through a single channel (email/Teams/Slack/PagerDuty).

## Alert Policy (Initial)

Create alerts with severity and owner mapping:

1. `critical`: API unavailable or startup blocked unexpectedly.
2. `high`: repeated offline ingest failures or phase-transition conflicts above threshold.
3. `high`: certificate review attention-required spike over baseline.
4. `medium`: sustained rate-limit rejections on high-risk endpoints.
5. `medium`: USB prepare failures above threshold in simulation mode.

Each alert should include:

- service/environment
- time window
- top error codes
- sample correlated `operation_id` or `session_id`

## Dashboards

Minimum dashboard set:

1. **API Health**: request rate, latency p95/p99, non-2xx ratio.
2. **Offline Workflow**: sessions by phase, transition counts, resume-required trends.
3. **Safety Rejections**: protected-system blocks, strict-targeting rejects, confidence-gate rejects.
4. **Certificate Pipeline**: eligible vs attention-required rates, signature verification failures.

## Implementation Phases

### Phase 1 (Immediate)

- Standardize log schema fields across handlers.
- Add missing counters around session transitions and ingest failures.
- Define alert thresholds with conservative defaults.

### Phase 2

- Integrate centralized log/metric sinks.
- Add dashboard templates to repo docs.
- Add runbook links per alert.

### Phase 3

- Add SLOs and error budgets.
- Add anomaly alerts for unusual safety rejection patterns.

## Runbook Expectations

For each high/critical alert, maintain a runbook with:

- triage steps
- immediate mitigation
- rollback/fail-safe action
- post-incident data to preserve

## Safety Constraint

Observability changes must remain non-destructive and must not bypass existing safety gates. Telemetry is informational and cannot authorize wipe execution.