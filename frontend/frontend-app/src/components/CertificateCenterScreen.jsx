import React, { useCallback, useEffect, useMemo, useState } from 'react';

const API = 'http://127.0.0.1:8080';

function ReviewBadge({ value }) {
  const ok = value === true;
  return (
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '6px',
        borderRadius: '999px',
        padding: '3px 10px',
        fontSize: '0.82rem',
        fontWeight: 700,
        color: ok ? 'var(--accent-color)' : 'var(--warning-color)',
        border: `1px solid ${ok ? 'var(--accent-color)' : 'var(--warning-color)'}`,
        background: ok ? 'rgba(0,230,118,0.10)' : 'rgba(255,171,64,0.12)',
      }}
    >
      <span className="material-icons" style={{ fontSize: '16px' }}>
        {ok ? 'verified' : 'warning_amber'}
      </span>
      {ok ? 'Ready' : 'Needs Review'}
    </span>
  );
}

export default function CertificateCenterScreen() {
  const [sessions, setSessions] = useState([]);
  const [selectedSessionId, setSelectedSessionId] = useState('');
  const [review, setReview] = useState(null);
  const [loadingSessions, setLoadingSessions] = useState(false);
  const [loadingReview, setLoadingReview] = useState(false);
  const [error, setError] = useState('');
  const [query, setQuery] = useState('');

  const filteredSessions = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return sessions;
    return sessions.filter((s) => {
      const hay = [
        s.session_id,
        s.target_device_id,
        s.target_device_model,
        s.phase,
      ]
        .filter(Boolean)
        .join(' ')
        .toLowerCase();
      return hay.includes(q);
    });
  }, [sessions, query]);

  const fetchSessions = useCallback(async () => {
    setLoadingSessions(true);
    setError('');
    try {
      const r = await fetch(`${API}/api/wipe/sessions`);
      if (!r.ok) throw new Error('Failed to load sessions.');
      const data = await r.json();
      setSessions(Array.isArray(data) ? data : []);
      if (!selectedSessionId && Array.isArray(data) && data.length > 0) {
        setSelectedSessionId(data[0].session_id || '');
      }
    } catch (e) {
      setError(String(e));
      setSessions([]);
    } finally {
      setLoadingSessions(false);
    }
  }, [selectedSessionId]);

  const fetchReview = async (sessionId) => {
    if (!sessionId) {
      setReview(null);
      return;
    }
    setLoadingReview(true);
    setError('');
    try {
      const r = await fetch(`${API}/api/certificate/${encodeURIComponent(sessionId)}/review`);
      const data = await r.json();
      if (!r.ok) {
        throw new Error(data?.error || data?.message || 'Failed to load certificate review.');
      }
      setReview(data);
    } catch (e) {
      setReview(null);
      setError(String(e));
    } finally {
      setLoadingReview(false);
    }
  };

  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  useEffect(() => {
    fetchReview(selectedSessionId);
  }, [selectedSessionId]);

  return (
    <div>
      <div className="dashboard-grid" style={{ marginBottom: '1rem' }}>
        <div className="card">
          <div className="card-header" style={{ justifyContent: 'space-between' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--secondary-color), var(--secondary-dark))' }}>
                <span className="material-icons">workspace_premium</span>
              </div>
              <div className="card-title">Certificate Center</div>
            </div>
            <button className="btn btn-secondary" onClick={fetchSessions} disabled={loadingSessions}>
              <span className="material-icons">refresh</span>
              {loadingSessions ? 'Refreshing...' : 'Refresh Sessions'}
            </button>
          </div>
          <div className="card-content">
            <div style={{ color: 'var(--text-secondary)', marginBottom: '10px', fontSize: '0.9rem' }}>
              Review certificate readiness, inspect verification issues, and download signed certificate PDFs.
            </div>
            <input
              className="form-input"
              placeholder="Search by session ID, device model, device ID, or phase"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
        </div>
      </div>

      {error && (
        <div className="card" style={{ marginBottom: '1rem', border: '1px solid var(--error-color)' }}>
          <div className="card-content" style={{ color: 'var(--error-color)' }}>{error}</div>
        </div>
      )}

      <div className="dashboard-grid" style={{ gridTemplateColumns: '1fr 1.4fr' }}>
        <div className="card">
          <div className="card-header">
            <div className="card-title">Sessions</div>
          </div>
          <div className="card-content" style={{ maxHeight: '62vh', overflowY: 'auto' }}>
            {loadingSessions ? (
              <div style={{ color: 'var(--text-secondary)' }}>Loading sessions...</div>
            ) : filteredSessions.length === 0 ? (
              <div style={{ color: 'var(--text-secondary)' }}>No sessions found.</div>
            ) : (
              filteredSessions.map((s) => {
                const selected = s.session_id === selectedSessionId;
                return (
                  <button
                    key={s.session_id}
                    type="button"
                    onClick={() => setSelectedSessionId(s.session_id || '')}
                    style={{
                      width: '100%',
                      textAlign: 'left',
                      borderRadius: '10px',
                      border: selected ? '1px solid var(--primary-color)' : '1px solid rgba(255,255,255,0.08)',
                      background: selected ? 'rgba(0,172,193,0.12)' : 'var(--surface-color)',
                      color: 'var(--text-primary)',
                      padding: '10px',
                      marginBottom: '8px',
                      cursor: 'pointer',
                    }}
                  >
                    <div style={{ fontFamily: 'monospace', fontSize: '0.83rem', fontWeight: 700 }}>{s.session_id}</div>
                    <div style={{ color: 'var(--text-secondary)', fontSize: '0.82rem' }}>
                      {s.target_device_model} • {s.target_device_id} • {s.target_device_size_gb} GB
                    </div>
                    <div style={{ color: 'var(--text-disabled)', fontSize: '0.78rem' }}>phase: {s.phase}</div>
                  </button>
                );
              })
            )}
          </div>
        </div>

        <div className="card">
          <div className="card-header" style={{ justifyContent: 'space-between' }}>
            <div className="card-title">Certificate Status</div>
            <button
              className="btn btn-secondary"
              onClick={() => fetchReview(selectedSessionId)}
              disabled={!selectedSessionId || loadingReview}
            >
              <span className="material-icons">refresh</span>
              {loadingReview ? 'Loading...' : 'Refresh Review'}
            </button>
          </div>
          <div className="card-content">
            {!selectedSessionId ? (
              <div style={{ color: 'var(--text-secondary)' }}>Select a session to inspect certificate status.</div>
            ) : loadingReview ? (
              <div style={{ color: 'var(--text-secondary)' }}>Loading review...</div>
            ) : !review ? (
              <div style={{ color: 'var(--text-secondary)' }}>No review data available for this session.</div>
            ) : (
              <>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
                  <ReviewBadge value={review.certificate_eligible} />
                  <div style={{ fontFamily: 'monospace', color: 'var(--text-secondary)', fontSize: '0.82rem' }}>{selectedSessionId}</div>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px', marginBottom: '12px' }}>
                  <div><strong>Status:</strong> {review.status}</div>
                  <div><strong>Phase:</strong> {String(review.manifest_phase || '')}</div>
                  <div><strong>Completion:</strong> {String(review.completion_status || '')}</div>
                  <div><strong>Signature:</strong> {review.signature_verified ? 'Verified' : 'Not verified'}</div>
                </div>

                {Array.isArray(review.issues) && review.issues.length > 0 && (
                  <div style={{ marginBottom: '12px' }}>
                    <div style={{ fontWeight: 700, marginBottom: '6px' }}>Issues</div>
                    {review.issues.map((issue, idx) => (
                      <div
                        key={`${idx}-${issue}`}
                        style={{
                          marginBottom: '6px',
                          borderLeft: '3px solid var(--warning-color)',
                          padding: '6px 8px',
                          borderRadius: '6px',
                          background: 'rgba(255,171,64,0.08)',
                          color: 'var(--text-secondary)',
                          fontSize: '0.88rem',
                        }}
                      >
                        {issue}
                      </div>
                    ))}
                  </div>
                )}

                {review.recommended_action && (
                  <div style={{ marginBottom: '12px', color: 'var(--text-secondary)' }}>
                    <strong>Recommended action:</strong> {review.recommended_action}
                  </div>
                )}

                <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
                  <button
                    className="btn btn-secondary"
                    onClick={() => window.open(`${API}/api/certificate/${encodeURIComponent(selectedSessionId)}/review`, '_blank')}
                  >
                    <span className="material-icons">preview</span>
                    Open Review JSON
                  </button>
                  <button
                    className="btn btn-primary"
                    onClick={() => window.open(`${API}/api/certificate/${encodeURIComponent(selectedSessionId)}/pdf`, '_blank')}
                  >
                    <span className="material-icons">download</span>
                    Download PDF
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}