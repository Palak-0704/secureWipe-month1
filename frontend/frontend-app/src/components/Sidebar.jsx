import React from 'react';

function Sidebar({ currentPage, onPageChange, isOpen }) {
  return (
    <div className={`sidebar ${isOpen ? 'open' : ''}`}>
      <div className="sidebar-header">
        <div className="logo">S</div>
        <div className="app-name">SecureWipe</div>
      </div>
      <nav className="nav-menu">
        <div 
          className={`nav-item ${currentPage === 'dashboard' ? 'active' : ''}`}
          onClick={() => onPageChange('dashboard')}
        >
          <span className="material-icons">dashboard</span>
          <span>Home</span>
        </div>
        <div 
          className={`nav-item ${currentPage === 'devices' ? 'active' : ''}`}
          onClick={() => onPageChange('devices')}
        >
          <span className="material-icons">storage</span>
          <span>Devices</span>
        </div>
        <div 
          className={`nav-item ${currentPage === 'advisor' ? 'active' : ''}`}
          onClick={() => onPageChange('advisor')}
        >
          <span className="material-icons">security</span>
          <span>Wipe Guidance</span>
        </div>
        <div 
          className={`nav-item ${currentPage === 'settings' ? 'active' : ''}`}
          onClick={() => onPageChange('settings')}
        >
          <span className="material-icons">settings</span>
          <span>Settings</span>
        </div>
      </nav>
    </div>
  );
}

export default Sidebar;
