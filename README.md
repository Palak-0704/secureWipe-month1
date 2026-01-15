

# SecureWipe-AI — Month 1 Submission

## Project Problem
Securely erasing sensitive data from storage devices is critical for privacy, compliance, and safety. Most tools are complex, lack intelligent guidance, and do not provide clear risk assessment or user consent workflows.

## Solution
SecureWipe-AI is a cross-platform tool that combines robust device detection, AI-powered wipe recommendations, risk scoring, and a user-friendly interface. It guides users through safe, compliant data erasure with clear explanations and audit logs.

## Features
- Real device detection (Windows/Linux)
- AI-powered wipe advisor (rule-based recommendations)
- Device classification and risk scoring
- Secure logging system (securewipe.log)
- Fully functional Rust AI chatbot (Groq API integration)
- Modular backend structure (Rust crates)
- Voice assistant structure (for future TTS integration)

## Month 1 Progress

### Completed
- Backend setup and device detection
- Logging system
- AI/ML wipe advisor, risk scoring, and chatbot

### Pending
- Frontend: GUI framework, wireframes, welcome screen, navigation
- Security: Safety/consent documentation, protocol, coding guidelines
- Mentor: Tech review, documentation setup

## How to Run
See PROGRESS_REPORT.md for a summary of completed and pending tasks.
To run the backend and AI chatbot, follow instructions in crates/core/README.md and use the CLI tools provided.