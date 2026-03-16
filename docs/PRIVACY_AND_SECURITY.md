# Privacy And Security

## Data Handling

SecureWipe stores operational metadata needed for wipe coordination and evidence.

Potential stored data includes:

- device model identifiers and capacity information
- session manifests
- wipe confirmations and progress state
- result evidence and certificate review payloads
- operator-supplied verification notes

By default, these artifacts are local to the deployment and are not intended for third-party transmission except where operators explicitly integrate external services.

## Local Storage

Runtime data is stored under the configured data root. Operators should place `SECUREWIPE_DATA_DIR` on an access-controlled path and apply normal endpoint protections, backup policy, and retention policy appropriate to evidence records.

## Chatbot Integration

If `groq_api` features are enabled, chatbot prompts and responses may involve external API calls depending on runtime configuration. Operators should not send sensitive device or customer data to external AI endpoints unless that is explicitly approved.

## Security Posture

SecureWipe favors local-only control surfaces and explicit safety gates.

Current protective measures include:

- localhost API binding by default
- restricted default CORS policy for local frontend origins
- explicit confirmation and state checks for session progression
- fail-closed policy for real USB provisioning
- signed certificate verification path for result evidence workflows

## Operator Responsibilities

Operators are responsible for:

- protecting access to the machine running the API server
- limiting filesystem access to stored evidence and session data
- securing any environment variables used to enable non-default features
- reviewing logs and artifacts before sharing them externally

## Data Minimization Guidance

Use the minimum operational data necessary to complete a wipe record. Avoid embedding customer secrets, credentials, or unrelated host information into verification notes, manifests, or certificate annotations.

## Retention Guidance

Keep wipe records only for the retention window required by your compliance or audit process. If certificates or result evidence are exported, ensure the exported copies are governed by the same retention and access controls.
