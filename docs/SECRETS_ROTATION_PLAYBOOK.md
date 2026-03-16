# Secrets Rotation Playbook (Local-First)

## Purpose

This playbook provides safe, local-first steps for rotating exposed API keys and preventing future secret leaks.

It does not require destructive disk operations and does not require pushing to any remote.

## Scope

- `GROQ_API_KEY`
- `SECUREWIPE_CERT_SIGNING_SEED`
- any future token in `.env` or shell history

## Immediate Containment

1. Revoke exposed keys in provider consoles immediately.
2. Generate replacement keys/seeds.
3. Store new values only in local `.env` (never commit `.env`).
4. Validate the app starts and health endpoints respond after rotation.

## Repository Hygiene

1. Keep `.env` ignored.
2. Keep examples and placeholders in `.env.example` only.
3. Run local scans before commit:

```powershell
git grep -n -I -E "(GROQ_API_KEY|SECUREWIPE_CERT_SIGNING_SEED|BEGIN PRIVATE KEY|api[_-]?key|secret|token)" -- .
```

4. If any real secret appears, replace with placeholders and rotate that credential again.

## Optional History Cleanup (Local)

If a secret was committed in history, use repository history rewrite only in a controlled branch and only after backup.

Recommended flow:

1. Create a backup clone.
2. Rewrite history to remove secrets.
3. Force-update remote only after team approval.

Note: this step is operationally sensitive and should be coordinated; it is not required for day-to-day local development.

## Preventive Controls

1. Add pre-commit secret scanning (e.g., `gitleaks` or equivalent).
2. Use short-lived credentials where possible.
3. Rotate high-impact credentials on a fixed schedule.
4. Keep credential ownership documented.

## Verification Checklist

- [ ] New keys generated and old keys revoked.
- [ ] `.env` updated locally and remains ignored.
- [ ] `.env.example` contains placeholders only.
- [ ] Local scan finds no hardcoded secrets.
- [ ] Application health checks pass.