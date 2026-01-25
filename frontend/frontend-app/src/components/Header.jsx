import React from 'react';

function Header({ currentPage, onMenuToggle }) {
  const getPageTitle = () => {
    switch(currentPage) {
      case 'welcome': return 'Welcome';
      case 'dashboard': return 'Home Overview';
      case 'devices': return 'Device Selection';
      case 'advisor': return 'Wipe Guidance';
      case 'settings': return 'Settings';
      default: return 'SecureWipe';
    }
  };

  return (
    <header className="header">
      <button className="menu-toggle" onClick={onMenuToggle}>
        <span className="material-icons">menu</span>
      </button>
      <h1 className="page-title">{getPageTitle()}</h1>
      <div className="header-actions">
        <button className="icon-button" title="Notifications">
          <span className="material-icons">notifications</span>
        </button>
        <button className="icon-button" title="Help">
          <span className="material-icons">help_outline</span>
        </button>
      </div>
    </header>
  );
}

export default Header;
