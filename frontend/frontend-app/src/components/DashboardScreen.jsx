
import React, { useEffect, useState } from 'react';


function DashboardScreen() {
  const [devices, setDevices] = useState([]);
  const [history, setHistory] = useState([]);
  const [systemHealth, setSystemHealth] = useState({ health: '...', update_available: false });
  const [securityStatus, setSecurityStatus] = useState({ status: '...', protections_active: false });

  useEffect(() => {
    fetch('http://127.0.0.1:8080/api/devices')
      .then(res => res.json())
      .then(setDevices)
      .catch(() => setDevices([]));
    fetch('http://127.0.0.1:8080/api/wipe/history')
      .then(res => res.json())
      .then(setHistory)
      .catch(() => setHistory([]));
    fetch('http://127.0.0.1:8080/api/system/health')
      .then(res => res.json())
      .then(setSystemHealth)
      .catch(() => setSystemHealth({ health: 'Unknown', update_available: false }));
    fetch('http://127.0.0.1:8080/api/system/security')
      .then(res => res.json())
      .then(setSecurityStatus)
      .catch(() => setSecurityStatus({ status: 'Unknown', protections_active: false }));
  }, []);

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
            <div className="card-value">{devices.length}</div>
            <div className="status-indicator status-success">
              <span className="material-icons" style={{ fontSize: '16px' }}>check_circle</span>
              {devices.length > 0 ? 'All devices ready' : 'No devices detected'}
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
            <div className="card-value">{history.length}</div>
            <div className="status-indicator status-success">
              <span className="material-icons" style={{ fontSize: '16px' }}>check_circle</span>
              {history.length > 0 ? 'All successful' : 'No wipes yet'}
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
            <div className="card-value">{systemHealth.health}</div>
            <div className="status-indicator status-warning">
              <span className="material-icons" style={{ fontSize: '16px' }}>info</span>
              {systemHealth.update_available ? 'Update available' : 'Up to date'}
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
            <div className="card-value">{securityStatus.status}</div>
            <div className="status-indicator status-success">
              <span className="material-icons" style={{ fontSize: '16px' }}>shield</span>
              {securityStatus.protections_active ? 'All protections active' : 'Some protections disabled'}
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
              <button className="btn btn-primary" style={{ width: '100%', justifyContent: 'center' }}>
                <span className="material-icons">add</span>
                Select Devices
              </button>
              <button className="btn btn-secondary" style={{ width: '100%', justifyContent: 'center' }}>
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
              {history.length === 0 && <div>No recent activity</div>}
              {history.slice(-3).reverse().map((entry, idx) => (
                <div key={idx} style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span>{entry.model || entry.device_id} Wipe Complete</span>
                  <span style={{ color: 'var(--text-secondary)' }}>{new Date(entry.timestamp).toLocaleString()}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default DashboardScreen;
