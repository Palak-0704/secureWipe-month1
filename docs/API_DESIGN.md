# SecureWipe-AI REST API Design (2026-01-25)

## Overview
This document defines the REST API endpoints for connecting the SecureWipe Rust backend (Axum) to the React frontend. All endpoints return JSON.

---

## Endpoints

### 1. Device Management
- **GET /api/devices**
  - List all connected storage devices
  - Response: `[ { id, name, type, size, status, model }, ... ]`

### 2. Wipe Operations
- **POST /api/wipe/start**
  - Start a wipe on selected devices
  - Body: `{ device_ids: [id, ...], method: string }`
  - Response: `{ status: 'started', wipe_id }`

- **GET /api/wipe/progress/{wipe_id}**
  - Get progress of a running wipe
  - Response: `{ progress: 0-100, status: string }`

### 3. AI/ML Advisor
- **POST /api/advisor/recommend**
  - Get wipe method recommendation for selected devices
  - Body: `{ device_ids: [id, ...], compliance: string }`
  - Response: `{ recommendation: string, rationale: string }`

### 4. Chatbot (Groq API)
- **POST /api/chatbot**
  - Send a message to the SecureWipe AI assistant
  - Body: `{ message: string, concise?: bool }`
  - Response: `{ reply: string }`

### 5. Certificate & Logs
- **GET /api/certificate/{wipe_id}**
  - Download wipe certificate
  - Response: `{ certificate: string (base64 or text) }`

- **GET /api/logs/{wipe_id}**
  - Get logs for a wipe session
  - Response: `{ logs: [ ... ] }`

---

## Notes
- All errors return `{ error: string }` with appropriate HTTP status.
- All endpoints are CORS-enabled for frontend access.
- Auth (if needed) can be added via headers in the future.
