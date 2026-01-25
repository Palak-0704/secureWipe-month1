import React from 'react';

function DeviceSelectionScreen({ devices, scanning, selectedDevices, onDeviceSelection, onStartScan, onProceed }) {
  return (
    <div>
      <div style={{ marginBottom: '1.5rem' }}>
        <h2 style={{ marginBottom: '0.5rem' }}>Device Selection</h2>
        {devices.length === 0 && (
          <p style={{ color: 'var(--text-secondary)' }}>
            Click "Start Device Scan" to detect connected devices, then select devices for wiping.
          </p>
        )}
      </div>
      {devices.length === 0 && (
        <div className="scan-button-container">
          <button 
            className="scan-button" 
            onClick={onStartScan}
            disabled={scanning}
          >
            <span className="material-icons">search</span>
            {scanning ? 'Scanning...' : 'Start Device Scan'}
          </button>
        </div>
      )}
      {scanning && (
        <div className="loading-container">
          <div className="spinner"></div>
          <p className="loading-text">Scanning for devices...</p>
        </div>
      )}
      {devices.length > 0 && (
        <>
          <div className="device-list">
            {devices.map(device => (
              <div
                key={device.id}
                className={`device-card${selectedDevices.includes(device.id) ? ' selected' : ''}`}
                onClick={() => onDeviceSelection(device.id)}
              >
                <div className="device-icon">
                  <span className="material-icons">
                    {device.dev_type && device.dev_type.toLowerCase().includes('ssd') ? 'sd_storage' : 'storage'}
                  </span>
                </div>
                <div className="device-info">
                  <div className="device-name">{device.model || device.id}</div>
                  <div className="device-details">
                    <div><strong>Type:</strong> {device.dev_type || device.type}</div>
                    <div><strong>Total Size:</strong> {device.size_gb ? `${device.size_gb} GB` : 'Unknown'}</div>
                    {typeof device.allocated_gb === 'number' && (
                      <div><strong>Allocated:</strong> {device.allocated_gb} GB</div>
                    )}
                    <div><strong>Connection:</strong> {device.connection || 'Unknown'}</div>
                    <div><strong>Removable:</strong> {device.removable ? 'Yes' : 'No'}</div>
                    <div>
                      <strong>System Disk:</strong> {device.is_system ? 'This is your main system disk (⚠️)' : 'No'}
                    </div>
                    <div><strong>SMART Status:</strong> {device.smart_status ? device.smart_status : 'Unknown'}</div>
                    {device.error && (
                      <div style={{ color: 'red', marginTop: 4 }}><strong>Note:</strong> {device.error}</div>
                    )}
                  </div>
                </div>
                {selectedDevices.includes(device.id) && (
                  <span className="material-icons selected-check">check_circle</span>
                )}
              </div>
            ))}
          </div>
          <div style={{ marginTop: '2rem', display: 'flex', justifyContent: 'space-between' }}>
            <button
              className="btn btn-secondary"
              onClick={onStartScan}
              disabled={scanning}
            >
              <span className="material-icons">refresh</span>
              Rescan Devices
            </button>
            <button
              className="btn btn-primary"
              onClick={onProceed}
              disabled={selectedDevices.length === 0}
            >
              <span className="material-icons">arrow_forward</span>
              Continue ({selectedDevices.length} selected)
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export default DeviceSelectionScreen;
