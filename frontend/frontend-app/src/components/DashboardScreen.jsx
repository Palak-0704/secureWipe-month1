
import React from 'react';


function DashboardScreen({ devices = [], history = [], systemHealth = { health: '...', update_available: false }, securityStatus = { status: '...', protections_active: false }, scannedOnce = false, onPageChange }) {
  // If scan never performed, always show zero/empty
  const scanned = scannedOnce && devices && Array.isArray(devices) && devices.length > 0;
  const deviceCount = scannedOnce ? (devices.length || 0) : 0;
  // Find the timestamp of the most recent scan after opening the dashboard
  const lastScanIdx = history.map(e => e.explanation && e.explanation.toLowerCase().includes('scan')).lastIndexOf(true);
  // Include the most recent scan event itself in sessionHistory
  let sessionHistory = [];
  if (lastScanIdx !== -1) {
    sessionHistory = history.slice(lastScanIdx);
  }
  const wipeCount = scannedOnce && lastScanIdx !== -1
    ? sessionHistory.filter(e => e.explanation && e.explanation.toLowerCase().includes('wipe')).length
    : 0;
  let health = '';
  if (scannedOnce) {
    if (typeof systemHealth.health === 'string' && systemHealth.health.trim() !== '') {
      health = systemHealth.health;
    }
  }
  const updateAvailable = scannedOnce ? !!systemHealth.update_available : false;
  let secStatus = '';
  if (scannedOnce) {
    if (typeof securityStatus.status === 'string' && securityStatus.status.trim() !== '') {
      secStatus = securityStatus.status;
    }
  }
  const protectionsActive = scannedOnce ? !!securityStatus.protections_active : false;

  return (
    <div>
      <div className="dashboard-grid">
        <div className="card">
          <div className="card-header">
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--primary-color), var(--primary-dark))' }}>
              <span className="material-icons">storage</span>
            </div>
            <div className="card-title">Connected Devices</div>
          </div>
          <div className="card-content">
            <div className="card-value">{deviceCount}</div>
            <div className="status-indicator status-success">
              <span className="material-icons" style={{ fontSize: '16px' }}>check_circle</span>
              {scanned && deviceCount > 0 ? 'All devices ready' : 'No devices detected'}
            </div>
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--secondary-color), var(--secondary-dark))' }}>
              <span className="material-icons">verified</span>
            </div>
            <div className="card-title">Completed Wipes</div>
          </div>
          <div className="card-content">
            <div className="card-value">{wipeCount}</div>
            <div className="status-indicator status-success">
              <span className="material-icons" style={{ fontSize: '16px' }}>check_circle</span>
              {scanned && wipeCount > 0 ? 'All successful' : 'No wipes yet'}
            </div>
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--warning-color), #ff6f00)' }}>
              <span className="material-icons">warning</span>
            </div>
            <div className="card-title">System Health</div>
          </div>
          <div className="card-content">
            <div className="card-value">{health}</div>
            <div className="status-indicator status-warning">
              <span className="material-icons" style={{ fontSize: '16px' }}>info</span>
              {updateAvailable ? 'Update available' : 'Up to date'}
            </div>
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--accent-color), #00c853)' }}>
              <span className="material-icons">security</span>
            </div>
            <div className="card-title">Security Status</div>
          </div>
          <div className="card-content">
            <div className="card-value">{secStatus}</div>
            <div className="status-indicator status-success">
              <span className="material-icons" style={{ fontSize: '16px' }}>shield</span>
              {protectionsActive ? 'All protections active' : 'Some protections disabled'}
            </div>
          </div>
        </div>
      </div>
      <div className="dashboard-grid">
        <div className="card">
          <div className="card-header">
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--primary-color), var(--primary-dark))' }}>
              <span className="material-icons">speed</span>
            </div>
            <div className="card-title">Quick Actions</div>
          </div>
          <div className="card-content">
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <button className="btn btn-primary" style={{ width: '100%', justifyContent: 'center' }} onClick={() => onPageChange && onPageChange('devices')}>
                <span className="material-icons">add</span>
                Select Devices
              </button>
              <button className="btn btn-secondary" style={{ width: '100%', justifyContent: 'center' }} onClick={() => onPageChange && onPageChange('offline')}>
                <span className="material-icons">history</span>
                View History
              </button>
            </div>
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--secondary-color), var(--secondary-dark))' }}>
              <span className="material-icons">insights</span>
            </div>
            <div className="card-title">Recent Activity</div>
          </div>
          <div className="card-content">
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
              {(sessionHistory.length === 0) && <div>No recent activity</div>}
              {sessionHistory.length > 0 && Array.from(new Map(sessionHistory.slice(-10).reverse().map(e => [e.timestamp, e])).values()).slice(0,4).map((entry, idx) => {
                let label = 'Activity:';
                if (entry.explanation && entry.explanation.toLowerCase().includes('scan')) {
                  label = 'Device Scan:';
                } else if (entry.explanation && entry.explanation.toLowerCase().includes('wipe')) {
                  label = 'SecureWipe:';
                }
                return (
                  <div key={idx} style={{ fontWeight: 600, fontSize: '0.95em', color: 'var(--text-secondary)', padding: '2px 0' }}>
                    {`${label} ${new Date(entry.timestamp).toLocaleString()}`}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default DashboardScreen;
