import React from 'react';

const ENV_FLAGS = [
  { name: 'SECUREWIPE_STRICT_TARGETING', description: 'Only allow removable or explicitly allowlisted devices for destructive wipe operations.', default: 'true (recommended)' },
  { name: 'SECUREWIPE_TARGET_ALLOWLIST', description: 'Comma-separated device IDs allowed under strict targeting.', default: '(empty)' },
  { name: 'SECUREWIPE_ALLOW_UNKNOWN_DETECTION_CONFIDENCE', description: 'Allow wipe execution when device detection confidence is unknown. Disable in production.', default: 'false' },
  { name: 'SECUREWIPE_OFFLINE_RUNTIME_BINARY', description: 'Absolute path to the offline_runtime binary for auto-bundling into handoff packages.', default: '(auto-discovered)' },
  { name: 'SECUREWIPE_DISABLE_OFFLINE_RUNTIME_AUTO_DISCOVERY', description: 'Set to 1 to skip auto-discovery of the offline_runtime binary during USB prepare.', default: 'false' },
  { name: 'GROQ_API_KEY', description: 'API key for the Groq LLM service powering the chatbot assistant.', default: '(required for chatbot)' },
];

const SCHEMA_TABLE = [
  { name: 'Wipe Session Manifest', version: 'v1', path: 'data/wipe_sessions/{session_id}.json' },
  { name: 'Offline Result Record', version: 'v1', path: 'data/offline_results/{session_id}.json' },
  { name: 'Wipe History', version: 'array', path: 'data/feedback_history.json' },
  { name: 'Confirmation State', version: 'object', path: 'data/confirmations/{wipe_id}.json' },
];

export default function SettingsScreen() {
  return (
    <div>
      {/* API and server info */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--primary-color), var(--primary-dark))' }}>
            <span className="material-icons">api</span>
          </div>
          <div className="card-title">API Configuration</div>
        </div>
        <div className="card-content">
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem' }}>
            <div className="settings-info-row">
              <span className="settings-label">API Endpoint</span>
              <code className="settings-value">http://127.0.0.1:8080</code>
            </div>
            <div className="settings-info-row">
              <span className="settings-label">CORS Origin</span>
              <code className="settings-value">http://localhost:5173</code>
            </div>
            <div className="settings-info-row">
              <span className="settings-label">Working Directory</span>
              <code className="settings-value">Month1-Submission/</code>
            </div>
            <div className="settings-info-row">
              <span className="settings-label">Session Schema</span>
              <code className="settings-value">v1</code>
            </div>
          </div>
        </div>
      </div>

      {/* Environment flags */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--secondary-color), var(--secondary-dark))' }}>
            <span className="material-icons">tune</span>
          </div>
          <div className="card-title">Runtime Environment Flags</div>
        </div>
        <div className="card-content">
          <div style={{ fontSize: '0.85em', color: 'var(--text-secondary)', marginBottom: '0.75rem' }}>
            These are set via environment variables before starting the API server. See <code style={{ background: 'var(--surface-color)', padding: '1px 5px', borderRadius: '4px' }}>.env.example</code>.
          </div>
          <div className="settings-flags-table">
            {ENV_FLAGS.map(f => (
              <div key={f.name} className="settings-flag-row">
                <div>
                  <code className="flag-name">{f.name}</code>
                  <div style={{ fontSize: '0.82em', color: 'var(--text-secondary)', marginTop: '2px' }}>{f.description}</div>
                </div>
                <div style={{ fontSize: '0.82em', color: 'var(--text-disabled)', whiteSpace: 'nowrap', marginLeft: '1rem' }}>
                  Default: <code style={{ color: 'var(--text-secondary)' }}>{f.default}</code>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Data schema reference */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header">
          <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--accent-color), #00c853)' }}>
            <span className="material-icons">folder_open</span>
          </div>
          <div className="card-title">Data Storage</div>
        </div>
        <div className="card-content">
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.88em' }}>
            <thead>
              <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.1)', color: 'var(--text-secondary)' }}>
                <th style={{ textAlign: 'left', padding: '4px 8px 8px 0' }}>Artifact</th>
                <th style={{ textAlign: 'left', padding: '4px 8px 8px 0' }}>Schema</th>
                <th style={{ textAlign: 'left', padding: '4px 8px 8px 0' }}>Path</th>
              </tr>
            </thead>
            <tbody>
              {SCHEMA_TABLE.map(row => (
                <tr key={row.name} style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
                  <td style={{ padding: '7px 8px 7px 0', fontWeight: 600, color: 'var(--text-primary)' }}>{row.name}</td>
                  <td style={{ padding: '7px 8px 7px 0', color: 'var(--primary-light)' }}>{row.version}</td>
                  <td style={{ padding: '7px 8px 7px 0' }}><code style={{ color: 'var(--text-secondary)', fontSize: '0.92em' }}>{row.path}</code></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Offline workflow reference */}
      <div className="card">
        <div className="card-header">
          <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--warning-color), #ff6f00)' }}>
            <span className="material-icons">offline_bolt</span>
          </div>
          <div className="card-title">Offline Wipe Flow Reference</div>
        </div>
        <div className="card-content">
          <ol style={{ paddingLeft: '1.2rem', color: 'var(--text-secondary)', fontSize: '0.88em', lineHeight: '1.8' }}>
            <li><b style={{ color: 'var(--text-primary)' }}>Create Session</b> — <code>POST /api/wipe/session/create</code> with target device and compliance.</li>
            <li><b style={{ color: 'var(--text-primary)' }}>Prepare Handoff Package</b> — <code>POST /api/usb/prepare</code> writes manifest, run-scripts, and the offline_runtime binary to <code>data/bootable_usb/&#123;session_id&#125;/</code>.</li>
            <li><b style={{ color: 'var(--text-primary)' }}>Execute Offline</b> — Run <code>offline_runtime --manifest wipe_manifest.json --confirmation-text ERASE --output-dir .</code> from the handoff package directory.</li>
            <li><b style={{ color: 'var(--text-primary)' }}>Ingest Result</b> — <code>POST /api/offline/result/ingest</code> with the <code>offline_result_ingest.json</code> produced by the runtime. Verified results require structured verification evidence.</li>
            <li><b style={{ color: 'var(--text-primary)' }}>Download Certificate</b> — <code>GET /api/certificate/&#123;session_id&#125;/pdf</code> for a signed PDF certificate of completion.</li>
          </ol>
        </div>
      </div>
    </div>
  );
}
