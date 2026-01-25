

# SecureWipe-AI: AI-Powered Secure Data Erasure

SecureWipe-AI is a cross-platform, safety-first tool for verifiable and auditable secure data erasure. It helps individuals and organizations permanently sanitize storage devices (HDD, SSD, NVMe, USB, SD, phones), verify the result with forensic checks, and produce tamper-proof certificates. The project is simulation-first: by default, no destructive actions run until lab validation, mentor sign-off, and explicit admin enablement.

---

## 🧑‍🔬 Research & Standards

SecureWipe-AI is grounded in direct research and best practices from leading standards and security guidelines:
- [NIST SP 800-88 Guidelines for Media Sanitization](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-88r1.pdf)
- [GDPR Article 17 (Right to Erasure)](https://gdpr-info.eu/art-17-gdpr/)
- [HIPAA Security Rule](https://www.hhs.gov/hipaa/for-professionals/security/index.html)
- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
- [Rust Security Guidelines](https://github.com/iqlusioninc/crates/blob/main/SECURITY.md)
- Academic literature on data remanence and secure deletion (e.g., Gutmann, 1996)

All safety, device detection, consent, and advisor logic are documented in `docs/`:
	- [SAFETY.md](docs/SAFETY.md): Simulation-first, consent, and lab safety protocols
	- [device-detection-wipe-methods/summary.md](docs/device-detection-wipe-methods/summary.md): Device detection and wipe method research
	- [security-consent-bestpractices/summary.md](docs/security-consent-bestpractices/summary.md): Consent, secure coding, and auditability
	- [wipe-advisor-logic/summary.md](docs/wipe-advisor-logic/summary.md): Advisor logic and explainability

**Proof of research and standards compliance is included in each doc.**

---

## 🚀 Features

- **AI-Powered Wipe Advisor:** Detects device type, encryption, HPA/DCO, and recommends the best sanitization method, time, and risk level.
- **One-Click Secure Wipe:** Easy UI/CLI flow for non-technical users with multi-step confirmations.
- **Hybrid Erase Methods:** Overwrite, ATA/NVMe firmware erase, crypto-erase (key destruction), and vendor tools.
- **Forensic Verification:** Post-wipe checks, trust score, and analytics for reuse/refurbishment potential.
- **Verifiable Certificates:** Digitally-signed JSON + PDF, with optional blockchain anchoring.
- **Voice & Chatbot Assistant:** Step-by-step voice prompts and chatbot in multiple languages.
- **Cross-Platform & Offline:** Runs on Windows, Linux, Android, or as a bootable offline ISO.
- **Modular Backend:** Rust workspace with core logic, CLI, and GUI support.
- **Internationalization:** Multi-language UI, voice, and certificate support.

---

## 📦 Project Structure

```
Month1-Submission/
├── Cargo.toml, LICENSE, PROGRESS_REPORT.md, README.md
├── certs/                # Generated certificates
├── crates/               # Rust backend: core logic, CLI
├── data/                 # Feedback and logs for analytics/audit
├── docs/                 # Safety, consent, architecture, wireframes, etc.
├── frontend/             # React frontend (Vite, modular components)
├── locales/              # i18n resource files (en, hi, ...)
├── target/               # Build artifacts
├── templates/            # Certificate HTML templates
├── tests/                # Integration tests
```

---

## 🖥️ Frontend Overview

- Built with **React** (Vite) for a modern, modular UI
- Edge-to-edge layout, Material Icons, Google Fonts
- Multi-language support (English, Hindi)
- Modular components: Welcome, Dashboard, Device Selection, Advisor, Progress
- All text in `locales/` for easy translation
- See `frontend/README.md` for research, design, and implementation details

---

## 🦀 Backend Overview

- Rust workspace: `crates/core` (engine, AI, certifier), `crates/cli` (command-line)
- Device detection, AI wipe advisor, risk scoring, logging
- Chatbot integration (Groq API, simulation-ready)
- All major logic exposed as public API for frontend/GUI
- See `crates/core/README.md` for API and usage

---

## 📊 Data & Audit

- All user/device feedback and logs are stored in `data/` (CSV/JSON)
- Use these for analytics, model training, or audit proof
- Example: See `data/feedback_history.csv` and `data/feedback_history.json` for real feedback and recommendations dataset.

---

## 📄 Documentation

- `docs/` contains safety, consent, architecture, and wireframes
- `PROGRESS_REPORT.md` tracks completed and pending tasks
- All research and design decisions are documented in `frontend/README.md`

---

## 🦺 Safety & Consent Model

- **Simulation-First:** All destructive actions are simulated by default. No real data is erased unless explicitly enabled in a lab setting.
- **Enabling Real Erase:**
	1. Compile with `real_erase` feature
	2. Set runtime admin flag (e.g., `ENABLE_REAL_ERASE=1`)
	3. Complete consent and mentor sign-off (see `docs/SAFETY.md`)
- **Audit Logging:** All actions (simulated or real) are logged in detail for compliance and troubleshooting.
- **Emergency Stop:** Immediate halt of all operations is supported (see docs for details).

---

## 🧩 Research-Driven Frontend & Modularity

- The frontend is modular, research-documented, and supports multi-language UI, voice, and certificate templates.
- See `frontend/README.md` for a student-perspective research log, design journey, and best practices.
- All UI/UX, error handling, and feedback flows are based on research into usability and safety for destructive tools.

---

## 🧠 Enabling Groq API Chatbot

To use the AI-powered chatbot, you must:
1. Set your Groq API key and endpoint as environment variables:
	- Windows (PowerShell):
	  ```powershell
	  $env:GROQ_API_KEY = "your_groq_api_key"
	  $env:GROQ_API_ENDPOINT = "https://api.groq.com/openai/v1/chat/completions"
	  ```
	- Linux/macOS:
	  ```bash
	  export GROQ_API_KEY="your_groq_api_key"
	  export GROQ_API_ENDPOINT="https://api.groq.com/openai/v1/chat/completions"
	  ```
2. Rebuild the CLI with Groq API support:
	```bash
	cargo build --release --features groq_api --manifest-path Month1-Submission/crates/cli/Cargo.toml
	```
3. Run the chatbot:
	```bash
	cargo run --manifest-path Month1-Submission/crates/cli/Cargo.toml --features groq_api -- chatbot --chat_model openai/gpt-oss-120b --system_prompt "You are a helpful assistant for SecureWipe. Keep your answers concise." -- "How do I securely wipe an SSD?"
	```
If you do not set the API key or build with the required feature, the chatbot will not work and you will see an error.
---

## 📑 Proof of Research & Datasets

- All research, standards, and best practices are cited in the relevant `docs/` files.
- Feedback and analytics datasets are included in `data/` for audit and reproducibility.
- The project is designed for transparency, safety, and verifiability at every step.

---


## 🚦 End-to-End Quickstart (All Features)

### Prerequisites
- Rust (rustup + cargo)
- Node.js (for frontend)
- Groq API key (for chatbot)

### 1. Set Environment Variables (Groq API for Chatbot)
**Windows (PowerShell):**
```powershell
$env:GROQ_API_KEY = "your_groq_api_key"
$env:GROQ_API_ENDPOINT = "https://api.groq.com/openai/v1/chat/completions"
```
**Linux/macOS:**
```bash
export GROQ_API_KEY="your_groq_api_key"
export GROQ_API_ENDPOINT="https://api.groq.com/openai/v1/chat/completions"
```

### 2. Start the Backend API (with Chatbot)
From the project root:
```sh
cargo run --bin main_api --manifest-path crates/core/Cargo.toml --features groq_api
```
This launches the REST API at http://127.0.0.1:8080 with device detection, wipe advisor, and chatbot endpoints.

### 3. Start the Frontend (React/Vite)
```sh
cd frontend/frontend-app
npm install
npm run dev
```
Visit http://localhost:5173 in your browser. You can:
- Scan for devices (real or simulated)
- View device details and compliance advisor
- Start a secure wipe (simulated by default)
- Use the integrated chatbot assistant

### 4. (Optional) Use the CLI for Device Scan & Advisor
```sh
cargo build --release --manifest-path crates/cli/Cargo.toml
./target/release/securewipe-cli scan
./target/release/securewipe-cli advise --device <device-id>
```

### 5. (Optional) Run the Chatbot from CLI
```sh
cargo run --manifest-path crates/cli/Cargo.toml --features groq_api -- chatbot --chat_model openai/gpt-oss-120b --system_prompt "You are a helpful assistant for SecureWipe. Keep your answers concise." -- "How do I securely wipe an SSD?"
```

### 6. Enable Real Erase (Lab Only!)
**WARNING: This will perform real data erasure.**
1. Compile with the `real_erase` feature:
	```sh
	cargo build --release --features real_erase --manifest-path crates/cli/Cargo.toml
	```
2. Set the admin flag:
	```sh
	$env:ENABLE_REAL_ERASE=1  # Windows
	export ENABLE_REAL_ERASE=1  # Linux/macOS
	```
3. Follow all lab safety and consent protocols (see docs/SAFETY.md)

---

## 🛠️ How to Run (Simulation/Legacy)

**Backend (CLI-only, no API):**
```sh
cargo build --release --manifest-path crates/cli/Cargo.toml
./target/release/securewipe-cli scan
./target/release/securewipe-cli advise --device dev-1
```

**Frontend:**
```sh
cd frontend/frontend-app
npm install
npm run dev
```

---

## 📝 Contributing & Safety

- Read `docs/SAFETY.md` before touching deletion/backends
- Work in feature branches: `feature/<name>`
- Include unit/integration tests for all new features
- PRs touching deletion logic must include a safety checklist, test plan, and mentor approval

---

## 📢 Contact / Maintainers

Owner: Palak-0704
For help with setup, safety gating, or lab procedures, open an issue and tag `safety` or `mentor`

---

