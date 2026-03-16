import React, { useState, useEffect, useCallback, useRef } from 'react';

const API = 'http://127.0.0.1:8080';

const PHASE_STEP = {
  in_app_prepared: 2,
  usb_prepared: 3,
  reboot_pending: 3,
  offline_started: 4,
  wiping: 4,
  verified: 5,
  certified: 5,
  completed: 5,
  failed: 5,
};

const PHASE_LABEL = {
  in_app_prepared: 'Session Created',
  usb_prepared: 'USB Ready',
  reboot_pending: 'Awaiting Reboot',
  offline_started: 'Offline Started',
  wiping: 'Wiping',
  verified: 'Verified',
  certified: 'Certified',
  completed: 'Completed',
  failed: 'Failed',
};

const PHASE_COLOR = {
  in_app_prepared: 'var(--primary-color)',
  usb_prepared: 'var(--secondary-color)',
  reboot_pending: 'var(--warning-color)',
  offline_started: 'var(--warning-color)',
  wiping: 'var(--warning-color)',
  verified: 'var(--accent-color)',
  certified: 'var(--accent-color)',
  completed: 'var(--accent-color)',
  failed: 'var(--error-color)',
};

function PhaseBadge({ phase }) {
  const label = PHASE_LABEL[phase] || phase;
  const color = PHASE_COLOR[phase] || 'var(--text-secondary)';
  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 10px',
      borderRadius: '12px',
      fontSize: '0.78em',
      fontWeight: 600,
      background: `${color}26`,
      color,
      border: `1px solid ${color}`,
      letterSpacing: '0.02em',
    }}>
      {label}
    </span>
  );
}

const STEPS = [
  { num: 1, title: 'Create Session', icon: 'add_circle_outline' },
  { num: 2, title: 'Prepare Handoff Package', icon: 'usb' },
  { num: 3, title: 'Execute Offline Wipe', icon: 'play_circle_outline' },
  { num: 4, title: 'Ingest Result', icon: 'cloud_upload' },
  { num: 5, title: 'Certificate', icon: 'workspace_premium' },
];

function StepIndicator({ currentStep }) {
  return (
    <div className="wizard-step-indicator">
      {STEPS.map((s, i) => (
        <React.Fragment key={s.num}>
          <div className={`step-circle ${s.num < currentStep ? 'completed' : s.num === currentStep ? 'active' : 'future'}`}>
            {s.num < currentStep
              ? <span className="material-icons" style={{ fontSize: '15px' }}>check</span>
              : s.num}
          </div>
          {i < STEPS.length - 1 && (
            <div className={`step-line ${s.num < currentStep ? 'completed' : ''}`} />
          )}
        </React.Fragment>
      ))}
    </div>
  );
}

function StepCard({ stepNum, currentStep, title, icon, children }) {
  const isActive = stepNum === currentStep;
  const isDone = stepNum < currentStep;
  return (
    <div className={`wizard-step-card ${isActive ? 'active' : isDone ? 'done' : 'locked'}`}>
      <div className="wizard-step-header">
        <div className="wizard-step-num-icon" style={{
          background: isActive ? 'var(--primary-color)' : isDone ? 'var(--accent-color)' : 'var(--surface-light)',
          color: isActive || isDone ? '#fff' : 'var(--text-disabled)',
        }}>
          {isDone
            ? <span className="material-icons" style={{ fontSize: '18px' }}>check</span>
            : <span className="material-icons" style={{ fontSize: '18px' }}>{icon}</span>}
        </div>
        <h3 style={{ color: isActive ? 'var(--text-primary)' : isDone ? 'var(--accent-color)' : 'var(--text-disabled)', margin: 0, fontSize: '1rem', fontWeight: 600 }}>
          {stepNum}. {title}
        </h3>
        {isDone && <span className="material-icons" style={{ marginLeft: 'auto', color: 'var(--accent-color)' }}>check_circle</span>}
      </div>
      {isActive && <div className="wizard-step-body">{children}</div>}
    </div>
  );
}

export default function OfflineSessionScreen({ devices: propDevices, initialTargetDeviceId = '', onDataChanged }) {
  const [sessions, setSessions] = useState([]);
  const [activeSession, setActiveSession] = useState(null);
  const [activeManifest, setActiveManifest] = useState(null);
  const [step, setStep] = useState(1);
  const [usbDevices, setUsbDevices] = useState([]);
  const [devices, setDevices] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [statusData, setStatusData] = useState(null);
  const pollRef = useRef(null);

  // Step 1
  const [createForm, setCreateForm] = useState({ target_device_id: '', compliance: 'GDPR' });
  // Step 2
  const [usbForm, setUsbForm] = useState({ usb_device_id: '', usb_overwrite_confirmation_text: '' });
  const [usbResult, setUsbResult] = useState(null);
  // Step 3
  const [executeConfirm, setExecuteConfirm] = useState('');
  const [executeResult, setExecuteResult] = useState(null);
  // Step 4
  const [ingestForm, setIngestForm] = useState({
    completion_status: 'verified',
    verification_notes: '',
    sample_blocks_checked: 8,
    sample_blocks_anomalies: 0,
    checksum_algorithm: 'sha256',
    verification_tool: 'securewipe_offline_runtime',
    operator_id: '',
  });
  const [ingestResult, setIngestResult] = useState(null);
  // Step 5
  const [certReview, setCertReview] = useState(null);

  const fetchSessions = useCallback(async () => {
    try {
      const r = await fetch(`${API}/api/wipe/sessions`);
      if (r.ok) {
        const data = await r.json();
        setSessions(data);
        if (activeSession) {
          const manifest = data.find(session => session.session_id === activeSession);
          if (manifest) {
            setActiveManifest(manifest);
            setStatusData(prev => prev ? {
              ...prev,
              phase: prev.phase || manifest.phase,
              progress_percent: prev.progress_percent ?? manifest.progress_percent,
              resume_required: prev.resume_required ?? manifest.resume_required,
              resume_hint: prev.resume_hint || manifest.resume_hint,
            } : {
              phase: manifest.phase,
              progress_percent: manifest.progress_percent,
              resume_required: manifest.resume_required,
              resume_hint: manifest.resume_hint,
            });
          }
        }
      }
    } catch { /* API may not be running */ }
  }, [activeSession]);

  useEffect(() => {
    fetchSessions();
    fetch(`${API}/api/usb/devices`).then(r => r.ok ? r.json() : []).then(setUsbDevices).catch(() => {});
    if (propDevices && propDevices.length > 0) {
      setDevices(propDevices);
    } else {
      fetch(`${API}/api/devices`).then(r => r.ok ? r.json() : []).then(setDevices).catch(() => {});
    }
  }, [fetchSessions, propDevices]);

  useEffect(() => {
    if (!initialTargetDeviceId) return;
    beginNewSession();
    setCreateForm({ target_device_id: initialTargetDeviceId, compliance: 'GDPR' });
  }, [initialTargetDeviceId]);

  // Poll session status while waiting for offline execution (step 3)
  // Track session status via SSE (with REST polling fallback) while the session is active.
  useEffect(() => {
    const cleanup = () => {
      if (pollRef.current) {
        if (typeof pollRef.current.close === 'function') pollRef.current.close();
        else clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
    if (!activeSession || step >= 5) { cleanup(); return; }

    if (typeof EventSource !== 'undefined') {
      const es = new EventSource(`${API}/api/wipe/session/${activeSession}/progress/stream`);
      pollRef.current = es;
      es.addEventListener('progress', ev => {
        try {
          const data = JSON.parse(ev.data);
          setStatusData({ phase: data.phase, progress_percent: data.progress_percent, resume_required: data.resume_required, resume_hint: data.resume_hint });
          const ns = PHASE_STEP[data.phase];
          if (ns && ns > step) setStep(ns);
          if (data.done) { es.close(); pollRef.current = null; }
        } catch { /* ignore malformed event */ }
      });
      es.onerror = () => { es.close(); pollRef.current = null; };
      return cleanup;
    }

    // Fallback: REST polling every 3 s for environments without EventSource
    const id = setInterval(async () => {
      try {
        const r = await fetch(`${API}/api/wipe/session/${activeSession}/status`);
        if (r.ok) {
          const data = await r.json();
          setStatusData(data);
          const ns = PHASE_STEP[data.phase];
          if (ns && ns > step) setStep(ns);
        }
      } catch { /* noop */ }
    }, 3000);
    pollRef.current = id;
    return cleanup;
  }, [activeSession, step]);

  function beginNewSession() {
    if (pollRef.current) {
      if (typeof pollRef.current.close === 'function') pollRef.current.close();
      else clearInterval(pollRef.current);
      pollRef.current = null;
    }
    setActiveSession(null);
    setActiveManifest(null);
    setStep(1);
    setError('');
    setUsbResult(null);
    setExecuteResult(null);
    setIngestResult(null);
    setCertReview(null);
    setStatusData(null);
    setCreateForm({ target_device_id: '', compliance: 'GDPR' });
    setUsbForm({ usb_device_id: '', usb_overwrite_confirmation_text: '' });
    setExecuteConfirm('');
    setIngestForm({ completion_status: 'verified', verification_notes: '', sample_blocks_checked: 8, sample_blocks_anomalies: 0, checksum_algorithm: 'sha256', verification_tool: 'securewipe_offline_runtime', operator_id: '' });
  }

  function loadSession(manifest) {
    if (pollRef.current) {
      if (typeof pollRef.current.close === 'function') pollRef.current.close();
      else clearInterval(pollRef.current);
      pollRef.current = null;
    }
    setActiveSession(manifest.session_id);
    setActiveManifest(manifest);
    const s = PHASE_STEP[manifest.phase] || 2;
    setStep(s);
    setError('');
    setUsbResult(null);
    setExecuteResult(null);
    setIngestResult(null);
    setCertReview(null);
    setStatusData({ phase: manifest.phase, progress_percent: manifest.progress_percent });
    if (['verified', 'certified', 'completed', 'failed'].includes(manifest.phase)) {
      fetch(`${API}/api/certificate/${manifest.session_id}/review`)
        .then(r => r.ok ? r.json() : null)
        .then(data => { if (data) setCertReview(data); })
        .catch(() => {});
    }
  }

  async function handleResumeSession() {
    if (!activeSession) return;
    setLoading(true); setError('');
    try {
      const r = await fetch(`${API}/api/wipe/session/${activeSession}/resume`, {
        method: 'POST',
      });
      const data = await r.json();
      if (!r.ok) { setError(data.error || data.message || 'Resume failed'); return; }
      setStatusData({
        phase: data.phase,
        progress_percent: data.progress_percent,
        resume_required: data.resume_required,
        resume_hint: data.resume_hint,
      });
      const nextStep = PHASE_STEP[data.phase];
      if (nextStep) setStep(nextStep);
      await fetchSessions();
      if (onDataChanged) await onDataChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function refreshCertificateReview() {
    if (!activeSession) return;
    setLoading(true); setError('');
    try {
      const r = await fetch(`${API}/api/certificate/${activeSession}/review`);
      const data = await r.json();
      if (!r.ok) { setError(data.error || data.message || 'Failed to refresh certificate review'); return; }
      setCertReview(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleCreateSession() {
    if (!createForm.target_device_id) { setError('Please select a target device.'); return; }
    setLoading(true); setError('');
    try {
      const r = await fetch(`${API}/api/wipe/session/create`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode: 'offline', target_device_id: createForm.target_device_id, compliance: createForm.compliance || undefined }),
      });
      const data = await r.json();
      if (!r.ok) { setError(data.error || data.message || 'Failed to create session'); return; }
      const sessionId = data.session_id;
      setActiveSession(sessionId);
      setStatusData({ phase: data.phase, progress_percent: data.progress_percent });
      setStep(2);
      const sessR = await fetch(`${API}/api/wipe/sessions`);
      if (sessR.ok) {
        const all = await sessR.json();
        setSessions(all);
        const manifest = all.find(s => s.session_id === sessionId);
        if (manifest) setActiveManifest(manifest);
      }
      if (onDataChanged) await onDataChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handlePrepareUsb() {
    const usbId = usbForm.usb_device_id.trim() || usbDevices[0]?.id || '';
    if (!usbId) { setError('Enter a USB device ID (or plug in a removable device).'); return; }
    setLoading(true); setError('');
    try {
      const r = await fetch(`${API}/api/usb/prepare`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          session_id: activeSession,
          usb_device_id: usbId,
          ...(usbForm.usb_overwrite_confirmation_text.trim()
            ? { usb_overwrite_confirmation_text: usbForm.usb_overwrite_confirmation_text.trim() }
            : {}),
        }),
      });
      const data = await r.json();
      if (!r.ok) { setError(data.error || data.message || 'Failed to prepare handoff package'); return; }
      setUsbResult(data);
      setStatusData({ phase: data.phase, progress_percent: data.progress_percent });
      setStep(3);
      await fetchSessions();
      if (onDataChanged) await onDataChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleExecute() {
    if (executeConfirm.trim().toUpperCase() !== 'ERASE') { setError('You must type ERASE to confirm.'); return; }
    setLoading(true); setError('');
    try {
      const r = await fetch(`${API}/api/offline/wipe/execute`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ session_id: activeSession, confirmation_text: 'ERASE' }),
      });
      const data = await r.json();
      if (!r.ok) { setError(data.error || data.message || 'Execution failed'); return; }
      setExecuteResult(data);
      setStatusData({ phase: data.phase, progress_percent: data.progress_percent });
      const ns = PHASE_STEP[data.phase];
      if (ns && ns > step) setStep(ns);
      else setStep(4);
      await fetchSessions();
      if (onDataChanged) await onDataChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleIngest() {
    setLoading(true); setError('');
    const isVerified = ingestForm.completion_status === 'verified';
    const payload = {
      session_id: activeSession,
      target_device_id: activeManifest?.target_device_id || '',
      target_device_model: activeManifest?.target_device_model || '',
      target_device_size_gb: activeManifest?.target_device_size_gb || 0,
      completion_status: ingestForm.completion_status,
      verification_passed: isVerified,
      verification_notes: ingestForm.verification_notes || undefined,
      ...(isVerified ? {
        verification_evidence: {
          sample_blocks_checked: Number(ingestForm.sample_blocks_checked),
          sample_blocks_anomalies: Number(ingestForm.sample_blocks_anomalies),
          checksum_algorithm: ingestForm.checksum_algorithm,
          verification_tool: ingestForm.verification_tool,
          operator_id: ingestForm.operator_id || undefined,
        }
      } : {}),
    };
    try {
      const r = await fetch(`${API}/api/offline/result/ingest`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = await r.json();
      if (!r.ok) { setError(data.error || data.message || 'Ingest failed'); return; }
      setIngestResult(data);
      setStatusData({ phase: data.phase, progress_percent: data.progress_percent });
      setStep(5);
      await fetchSessions();
      if (onDataChanged) await onDataChanged();
      const cr = await fetch(`${API}/api/certificate/${activeSession}/review`);
      if (cr.ok) setCertReview(await cr.json());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  function handleDownloadCert() {
    window.open(`${API}/api/certificate/${activeSession}/pdf`, '_blank');
  }

  const currentPhase = statusData?.phase || activeManifest?.phase;
  const progressPct = statusData?.progress_percent ?? 0;

  return (
    <div className="offline-session-screen">

      {/* ── Sessions history panel ── */}
      <div className="card" style={{ marginBottom: '1.5rem' }}>
        <div className="card-header" style={{ justifyContent: 'space-between', alignItems: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--primary-color), var(--primary-dark))' }}>
              <span className="material-icons">history</span>
            </div>
            <div className="card-title">Offline Sessions</div>
          </div>
          <div style={{ display: 'flex', gap: '8px' }}>
            <button className="btn btn-secondary" onClick={fetchSessions} style={{ padding: '6px 12px', fontSize: '0.88em' }} title="Refresh sessions">
              <span className="material-icons" style={{ fontSize: '16px' }}>refresh</span>
            </button>
            <button className="btn btn-primary" onClick={beginNewSession} style={{ padding: '6px 16px', fontSize: '0.88em' }}>
              <span className="material-icons" style={{ fontSize: '16px' }}>add</span>
              New Session
            </button>
          </div>
        </div>
        <div className="card-content">
          {sessions.length === 0 ? (
            <div style={{ color: 'var(--text-secondary)', padding: '0.5rem 0' }}>
              No sessions yet. Click <b>New Session</b> to start an offline wipe flow.
            </div>
          ) : (
            <div className="sessions-list">
              {sessions.map(s => (
                <div
                  key={s.session_id}
                  className={`session-card ${activeSession === s.session_id ? 'active' : ''}`}
                  onClick={() => loadSession(s)}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: '8px' }}>
                    <div>
                      <div style={{ fontWeight: 600, fontSize: '0.88em', color: 'var(--text-primary)', fontFamily: 'monospace' }}>
                        {s.session_id.length > 30 ? s.session_id.slice(0, 30) + '…' : s.session_id}
                      </div>
                      <div style={{ color: 'var(--text-secondary)', fontSize: '0.82em', marginTop: '2px' }}>
                        {s.target_device_model} · {s.target_device_size_gb} GB
                      </div>
                    </div>
                    <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                      <PhaseBadge phase={s.phase} />
                      {s.resume_required && (
                        <span title="Resume required" style={{
                          display: 'inline-block', padding: '2px 6px', borderRadius: '8px',
                          fontSize: '0.72em', fontWeight: 700,
                          background: 'rgba(255,193,7,0.18)', color: 'var(--warning-color)',
                          border: '1px solid var(--warning-color)', cursor: 'default',
                        }}>↩ Resume</span>
                      )}
                    </div>
                  </div>
                  <div style={{ color: 'var(--text-disabled)', fontSize: '0.78em', marginTop: '4px' }}>
                    {new Date(s.created_at).toLocaleString()}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* ── Wizard ── */}
      <div className="card">
        <div className="card-header">
          <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--secondary-color), var(--secondary-dark))' }}>
            <span className="material-icons">manage_history</span>
          </div>
          <div className="card-title">
            {activeSession ? `Session: ${activeSession.slice(0, 22)}…` : 'Start Offline Wipe Session'}
          </div>
          {currentPhase && (
            <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: '10px' }}>
              <PhaseBadge phase={currentPhase} />
              {progressPct > 0 && (
                <span style={{ fontSize: '0.82em', color: 'var(--text-secondary)' }}>{progressPct}%</span>
              )}
            </div>
          )}
        </div>
        <div className="card-content">
          <StepIndicator currentStep={step} />

          {activeManifest?.resume_required && (
            <div style={{
              display: 'flex', alignItems: 'flex-start', gap: '10px', margin: '12px 0 4px',
              background: 'rgba(255,193,7,0.10)', border: '1px solid var(--warning-color)',
              borderRadius: '8px', padding: '10px 14px',
            }}>
              <span className="material-icons" style={{ color: 'var(--warning-color)', fontSize: '20px', flexShrink: 0 }}>warning_amber</span>
              <div>
                <div style={{ fontWeight: 600, color: 'var(--warning-color)', fontSize: '0.92em' }}>Session resume required</div>
                {(activeManifest.resume_hint || statusData?.resume_hint) && (
                  <div style={{ fontSize: '0.83em', color: 'var(--text-secondary)', marginTop: '3px' }}>
                    {activeManifest.resume_hint || statusData?.resume_hint}
                  </div>
                )}
                <button
                  className="btn btn-secondary"
                  style={{ marginTop: '10px' }}
                  onClick={handleResumeSession}
                  disabled={loading}
                >
                  <span className="material-icons" style={{ fontSize: '18px' }}>restart_alt</span>
                  {loading ? 'Resuming…' : 'Resume Session'}
                </button>
              </div>
            </div>
          )}

          {error && (
            <div className="offline-error-box">
              <span className="material-icons" style={{ fontSize: '18px' }}>error_outline</span>
              {error}
            </div>
          )}

          <div className="wizard-steps-list">

            {/* ── Step 1: Create Session ── */}
            <StepCard stepNum={1} currentStep={step} title="Create Session" icon="add_circle_outline">
              <div className="form-group">
                <label>Target Device</label>
                <select
                  className="form-select"
                  value={createForm.target_device_id}
                  onChange={e => setCreateForm(f => ({ ...f, target_device_id: e.target.value }))}
                >
                  <option value="">-- select device --</option>
                  {devices.map(d => (
                    <option key={d.id} value={d.id}>
                      {d.model} ({d.id}) — {d.size_gb} GB
                    </option>
                  ))}
                </select>
              </div>
              <div className="form-group">
                <label>Compliance Framework</label>
                <select
                  className="form-select"
                  value={createForm.compliance}
                  onChange={e => setCreateForm(f => ({ ...f, compliance: e.target.value }))}
                >
                  <option value="GDPR">GDPR</option>
                  <option value="HIPAA">HIPAA</option>
                  <option value="NIST">NIST</option>
                  <option value="">None</option>
                </select>
              </div>
              <button className="btn btn-primary" onClick={handleCreateSession} disabled={loading}>
                {loading ? 'Creating…' : 'Create Session'}
              </button>
            </StepCard>

            {/* ── Step 2: Prepare Handoff Package ── */}
            <StepCard stepNum={2} currentStep={step} title="Prepare Handoff Package" icon="usb">
              {activeManifest && (
                <div className="info-row" style={{ marginBottom: '1rem' }}>
                  <span className="material-icons info-icon">info</span>
                  <div>
                    <div style={{ fontWeight: 600 }}>Session ID</div>
                    <div style={{ fontFamily: 'monospace', fontSize: '0.85em', color: 'var(--text-secondary)' }}>{activeSession}</div>
                    <div style={{ marginTop: '4px', color: 'var(--text-secondary)', fontSize: '0.85em' }}>
                      Target: {activeManifest.target_device_model} · {activeManifest.target_device_size_gb} GB · {activeManifest.method}
                    </div>
                  </div>
                </div>
              )}
              <div className="form-group">
                <label>USB / Target Device ID</label>
                {usbDevices.length > 0 ? (
                  <select
                    className="form-select"
                    value={usbForm.usb_device_id}
                    onChange={e => setUsbForm(f => ({ ...f, usb_device_id: e.target.value }))}
                  >
                    <option value="">-- auto-select first removable --</option>
                    {usbDevices.map(d => (
                      <option key={d.id} value={d.id}>{d.model} ({d.id})</option>
                    ))}
                  </select>
                ) : (
                  <input
                    className="form-input"
                    placeholder="e.g. usb0 or disk2"
                    value={usbForm.usb_device_id}
                    onChange={e => setUsbForm(f => ({ ...f, usb_device_id: e.target.value }))}
                  />
                )}
              </div>
              <div className="form-group" style={{ marginTop: '0.75rem' }}>
                <label>
                  Overwrite Confirmation{' '}
                  <span style={{ fontWeight: 400, fontSize: '0.82em', color: 'var(--text-secondary)' }}>
                    (only required when <code>SECUREWIPE_USB_PROVISION_MODE=real</code>)
                  </span>
                </label>
                <input
                  className="form-input"
                  placeholder='Type "ERASE_USB" to confirm real-mode overwrite'
                  value={usbForm.usb_overwrite_confirmation_text}
                  onChange={e => setUsbForm(f => ({ ...f, usb_overwrite_confirmation_text: e.target.value }))}
                />
                <div style={{ fontSize: '0.78em', color: 'var(--text-secondary)', marginTop: '4px' }}>
                  Leave blank in simulation mode. In real provisioning mode the server requires this
                  phrase before writing to the USB device.
                </div>
              </div>
              <button className="btn btn-primary" onClick={handlePrepareUsb} disabled={loading}>
                {loading ? 'Preparing…' : 'Prepare Handoff Package'}
              </button>
              {usbResult && (
                <div className="result-box success" style={{ marginTop: '1rem' }}>
                  <span className="material-icons" style={{ fontSize: '18px' }}>check_circle</span>
                  <div>
                    <div style={{ fontWeight: 600 }}>Handoff package created</div>
                    <div style={{ fontFamily: 'monospace', fontSize: '0.82em', color: 'var(--text-secondary)' }}>{usbResult.output_path}</div>
                    <div style={{ fontSize: '0.82em', marginTop: '4px', color: 'var(--text-secondary)' }}>{usbResult.next_step}</div>
                  </div>
                </div>
              )}
            </StepCard>

            {/* ── Step 3: Execute Offline Wipe ── */}
            <StepCard stepNum={3} currentStep={step} title="Execute Offline Wipe" icon="play_circle_outline">
              <div className="info-row" style={{ marginBottom: '1rem' }}>
                <span className="material-icons info-icon" style={{ color: 'var(--warning-color)' }}>info</span>
                <div style={{ fontSize: '0.88em', color: 'var(--text-secondary)' }}>
                  In a real deployment, boot the target machine into the prepared offline environment and run the
                  <code style={{ background: 'var(--surface-color)', padding: '1px 5px', borderRadius: '4px', margin: '0 4px' }}>offline_runtime</code>
                  binary from the handoff package. For demo/test, use the button below to simulate execution via the API.
                </div>
              </div>
              {currentPhase && (
                <div style={{ marginBottom: '1rem' }}>
                  <span style={{ fontSize: '0.85em', color: 'var(--text-secondary)' }}>Current phase: </span>
                  <PhaseBadge phase={currentPhase} />
                  {statusData?.progress_percent > 0 && (
                    <span style={{ marginLeft: '8px', fontSize: '0.82em', color: 'var(--text-secondary)' }}>
                      {statusData.progress_percent}%
                    </span>
                  )}
                </div>
              )}
              <div className="form-group">
                <label>Confirmation — type ERASE to proceed</label>
                <input
                  className="form-input"
                  placeholder="ERASE"
                  value={executeConfirm}
                  onChange={e => setExecuteConfirm(e.target.value)}
                />
              </div>
              <button
                className="btn btn-danger"
                onClick={handleExecute}
                disabled={loading || executeConfirm.trim().toUpperCase() !== 'ERASE'}
              >
                {loading ? 'Executing…' : 'Simulate Execute (API)'}
              </button>
              {executeResult && (
                <div className="result-box success" style={{ marginTop: '1rem' }}>
                  <span className="material-icons" style={{ fontSize: '18px' }}>check_circle</span>
                  <div>
                    <div style={{ fontWeight: 600 }}>Execution complete</div>
                    <div style={{ fontSize: '0.82em', color: 'var(--text-secondary)' }}>{executeResult.message} · mode: {executeResult.mode}</div>
                  </div>
                </div>
              )}
            </StepCard>

            {/* ── Step 4: Ingest Result ── */}
            <StepCard stepNum={4} currentStep={step} title="Ingest Offline Result" icon="cloud_upload">
              {activeManifest && (
                <div className="info-row" style={{ marginBottom: '1rem' }}>
                  <span className="material-icons info-icon">info</span>
                  <div style={{ fontSize: '0.85em', color: 'var(--text-secondary)' }}>
                    Target: <b style={{ color: 'var(--text-primary)' }}>{activeManifest.target_device_model}</b> · {activeManifest.target_device_size_gb} GB · ID: <code style={{ background: 'var(--surface-color)', padding: '1px 4px', borderRadius: '4px' }}>{activeManifest.target_device_id}</code>
                  </div>
                </div>
              )}
              <div className="form-group">
                <label>Completion Status</label>
                <select
                  className="form-select"
                  value={ingestForm.completion_status}
                  onChange={e => setIngestForm(f => ({ ...f, completion_status: e.target.value }))}
                >
                  <option value="verified">Verified (all blocks clean)</option>
                  <option value="failed">Failed</option>
                  <option value="partial">Partial</option>
                  <option value="inconclusive">Inconclusive</option>
                </select>
              </div>

              {ingestForm.completion_status === 'verified' && (
                <>
                  <div className="form-row">
                    <div className="form-group">
                      <label>Blocks Checked</label>
                      <input
                        className="form-input"
                        type="number"
                        min="1"
                        value={ingestForm.sample_blocks_checked}
                        onChange={e => setIngestForm(f => ({ ...f, sample_blocks_checked: e.target.value }))}
                      />
                    </div>
                    <div className="form-group">
                      <label>Anomalies (must be 0)</label>
                      <input
                        className="form-input"
                        type="number"
                        min="0"
                        value={ingestForm.sample_blocks_anomalies}
                        onChange={e => setIngestForm(f => ({ ...f, sample_blocks_anomalies: e.target.value }))}
                      />
                    </div>
                  </div>
                  <div className="form-row">
                    <div className="form-group">
                      <label>Checksum Algorithm</label>
                      <select
                        className="form-select"
                        value={ingestForm.checksum_algorithm}
                        onChange={e => setIngestForm(f => ({ ...f, checksum_algorithm: e.target.value }))}
                      >
                        <option value="sha256">SHA-256</option>
                        <option value="sha512">SHA-512</option>
                        <option value="md5">MD5</option>
                      </select>
                    </div>
                    <div className="form-group">
                      <label>Verification Tool</label>
                      <input
                        className="form-input"
                        value={ingestForm.verification_tool}
                        onChange={e => setIngestForm(f => ({ ...f, verification_tool: e.target.value }))}
                      />
                    </div>
                  </div>
                  <div className="form-group">
                    <label>Operator ID <span style={{ color: 'var(--text-disabled)' }}>(optional)</span></label>
                    <input
                      className="form-input"
                      placeholder="e.g. op-1234"
                      value={ingestForm.operator_id}
                      onChange={e => setIngestForm(f => ({ ...f, operator_id: e.target.value }))}
                    />
                  </div>
                </>
              )}

              <div className="form-group">
                <label>Verification Notes <span style={{ color: 'var(--text-disabled)' }}>(optional)</span></label>
                <textarea
                  className="form-input"
                  rows={2}
                  style={{ resize: 'vertical' }}
                  placeholder="Any observations from the offline wipe process…"
                  value={ingestForm.verification_notes}
                  onChange={e => setIngestForm(f => ({ ...f, verification_notes: e.target.value }))}
                />
              </div>

              <button className="btn btn-primary" onClick={handleIngest} disabled={loading}>
                {loading ? 'Ingesting…' : 'Ingest Result'}
              </button>
              {ingestResult && (
                <div className="result-box success" style={{ marginTop: '1rem' }}>
                  <span className="material-icons" style={{ fontSize: '18px' }}>check_circle</span>
                  <div>
                    <div style={{ fontWeight: 600 }}>Result ingested</div>
                    <div style={{ fontSize: '0.82em', color: 'var(--text-secondary)' }}>{ingestResult.message}</div>
                  </div>
                </div>
              )}
            </StepCard>

            {/* ── Step 5: Certificate ── */}
            <StepCard stepNum={5} currentStep={step} title="Certificate" icon="workspace_premium">
              {certReview ? (
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '1rem' }}>
                    <span className="material-icons" style={{ color: certReview.certificate_eligible ? 'var(--accent-color)' : 'var(--warning-color)', fontSize: '28px' }}>
                      {certReview.certificate_eligible ? 'verified' : 'warning'}
                    </span>
                    <div>
                      <div style={{ fontWeight: 700, color: certReview.certificate_eligible ? 'var(--accent-color)' : 'var(--warning-color)' }}>
                        {certReview.certificate_eligible ? 'Certificate Ready' : 'Review Required'}
                      </div>
                      <div style={{ fontSize: '0.85em', color: 'var(--text-secondary)' }}>{certReview.status}</div>
                    </div>
                  </div>

                  <div className="cert-meta-grid">
                    <div><span className="cert-label">Completion</span><span className="cert-value">{certReview.completion_status}</span></div>
                    <div><span className="cert-label">Verification</span><span className="cert-value">{certReview.verification_passed ? '✓ Passed' : '✗ Not Passed'}</span></div>
                    <div><span className="cert-label">Signature</span><span className="cert-value">{certReview.signature_verified ? '✓ Verified' : '✗ Not Verified'}</span></div>
                    <div><span className="cert-label">Phase</span><span className="cert-value">{String(certReview.manifest_phase).replace(/_/g, ' ')}</span></div>
                  </div>

                  {certReview.verification_evidence && (
                    <div style={{ marginTop: '0.75rem', padding: '0.75rem', borderRadius: '8px', background: 'var(--surface-light)', border: '1px solid var(--border-color)' }}>
                      <div style={{ fontWeight: 600, fontSize: '0.82em', color: 'var(--text-secondary)', marginBottom: '0.5rem', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Verification Evidence</div>
                      <div className="cert-meta-grid">
                        <div><span className="cert-label">Blocks Checked</span><span className="cert-value">{certReview.verification_evidence.sample_blocks_checked}</span></div>
                        <div><span className="cert-label">Anomalies</span><span className="cert-value" style={{ color: certReview.verification_evidence.sample_blocks_anomalies === 0 ? 'var(--accent-color)' : 'var(--error-color)' }}>{certReview.verification_evidence.sample_blocks_anomalies}</span></div>
                        {certReview.verification_evidence.checksum_algorithm && (
                          <div><span className="cert-label">Algorithm</span><span className="cert-value">{certReview.verification_evidence.checksum_algorithm}</span></div>
                        )}
                        {certReview.verification_evidence.verification_tool && (
                          <div><span className="cert-label">Tool</span><span className="cert-value">{certReview.verification_evidence.verification_tool}</span></div>
                        )}
                        {certReview.verification_evidence.operator_id && (
                          <div><span className="cert-label">Operator</span><span className="cert-value">{certReview.verification_evidence.operator_id}</span></div>
                        )}
                      </div>
                    </div>
                  )}

                  {certReview.issues && certReview.issues.length > 0 && (
                    <div style={{ marginTop: '0.75rem' }}>
                      {certReview.issues.map((issue, i) => (
                        <div
                          key={i}
                          style={{
                            display: 'flex',
                            alignItems: 'flex-start',
                            gap: '6px',
                            color: issue.toLowerCase().includes('anomaly detector') ? 'var(--error-color)' : 'var(--warning-color)',
                            fontSize: '0.83em',
                            marginBottom: '4px',
                            background: issue.toLowerCase().includes('anomaly detector') ? 'rgba(255,82,82,0.08)' : 'transparent',
                            border: issue.toLowerCase().includes('anomaly detector') ? '1px solid rgba(255,82,82,0.25)' : 'none',
                            borderRadius: issue.toLowerCase().includes('anomaly detector') ? '6px' : '0',
                            padding: issue.toLowerCase().includes('anomaly detector') ? '6px 8px' : '0',
                          }}
                        >
                          <span className="material-icons" style={{ fontSize:'15px', marginTop:'2px' }}>
                            {issue.toLowerCase().includes('anomaly detector') ? 'report_problem' : 'warning_amber'}
                          </span>
                          {issue}
                        </div>
                      ))}
                    </div>
                  )}

                  <div style={{ marginTop: '1rem', fontSize: '0.85em', color: 'var(--text-secondary)' }}>
                    {certReview.recommended_action}
                  </div>

                  <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap', marginTop: '1rem' }}>
                    <button className="btn btn-secondary" onClick={refreshCertificateReview} disabled={loading}>
                      <span className="material-icons" style={{ fontSize: '18px' }}>refresh</span>
                      Refresh Review
                    </button>
                    <button
                      className="btn btn-secondary"
                      onClick={() => window.open(`${API}/api/certificate/${activeSession}/review`, '_blank')}
                    >
                      <span className="material-icons" style={{ fontSize: '18px' }}>preview</span>
                      Open Review JSON
                    </button>
                    {certReview.certificate_eligible && (
                      <button className="btn btn-primary" onClick={handleDownloadCert}>
                        <span className="material-icons" style={{ fontSize: '18px' }}>download</span>
                        Download Certificate PDF
                      </button>
                    )}
                  </div>
                </div>
              ) : (
                <div style={{ color: 'var(--text-secondary)', display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span className="material-icons">hourglass_empty</span>
                  Awaiting result ingest to generate certificate.
                </div>
              )}
            </StepCard>

          </div>
        </div>
      </div>
    </div>
  );
}
