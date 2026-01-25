import React from 'react';

function WipeProgress({ progress }) {
  return (
    <div className="loading-container">
      <div className="spinner"></div>
      <h2 style={{ margin: '1rem 0' }}>Wiping in Progress</h2>
      <div className="progress-container" style={{ width: '300px' }}>
        <div className="progress-bar" style={{ width: `${progress}%` }}></div>
      </div>
      <p className="loading-text">{progress}% Complete</p>
      <p style={{ color: 'var(--text-secondary)', marginTop: '1rem' }}>Please do not disconnect your devices</p>
    </div>
  );
}

export default WipeProgress;
