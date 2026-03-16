import React from 'react';

class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error) {
    return { hasError: true, error };
  }

  componentDidCatch(error, info) {
    // Keep console output for local debugging without exposing internals in UI.
    console.error('UI crash captured by ErrorBoundary:', error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div style={{
          minHeight: '100vh',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          background: 'var(--background-dark)',
          padding: '1rem',
        }}>
          <div className="card" style={{ maxWidth: '720px', width: '100%' }}>
            <div className="card-header">
              <div className="card-icon" style={{ background: 'linear-gradient(135deg, var(--error-color), #c62828)' }}>
                <span className="material-icons">error</span>
              </div>
              <div className="card-title">Unexpected UI Error</div>
            </div>
            <div className="card-content">
              <p style={{ color: 'var(--text-secondary)', marginBottom: '1rem' }}>
                The interface hit an unexpected error. No wipe action was started by this fallback.
              </p>
              <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
                <button className="btn btn-primary" onClick={this.handleRetry}>
                  <span className="material-icons">restart_alt</span>
                  Retry UI
                </button>
                <button className="btn btn-secondary" onClick={() => window.location.reload()}>
                  <span className="material-icons">refresh</span>
                  Reload App
                </button>
              </div>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;