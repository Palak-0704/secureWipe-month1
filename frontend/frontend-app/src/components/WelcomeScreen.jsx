import React from 'react';

function WelcomeScreen({ onGetStarted }) {
  return (
    <div className="welcome-container">
      <div className="welcome-logo">S</div>
      <h1 className="welcome-title">SecureWipe</h1>
      <p className="welcome-description">
        Professional-grade data wiping solution that ensures complete, secure, and permanent deletion of sensitive information from your storage devices.
      </p>
      <button className="btn btn-primary" onClick={onGetStarted}>
        <span className="material-icons">arrow_forward</span>
        Get Started
      </button>
    </div>
  );
}

export default WelcomeScreen;
