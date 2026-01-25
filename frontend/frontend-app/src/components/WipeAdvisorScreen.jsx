import React, { useEffect, useState } from 'react';

const COMPLIANCES = [
  { label: 'GDPR', value: 'GDPR' },
  { label: 'HIPAA', value: 'HIPAA' },
  { label: 'NIST', value: 'NIST' },
];

function WipeAdvisorScreen({ selectedDevices, devices, onStartWipe }) {
  const [advisors, setAdvisors] = useState({});
  useEffect(() => {
    async function fetchAll() {
      if (!selectedDevices.length) return;
      const results = {};
      for (const c of COMPLIANCES) {
        const res = await fetch('http://127.0.0.1:8080/api/advisor/recommend', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ device_ids: selectedDevices, compliance: c.value })
        });
        results[c.value] = await res.json();
      }
      setAdvisors(results);
    }
    fetchAll();
  }, [selectedDevices]);

  const selectedDeviceObjects = devices.filter(device => selectedDevices.includes(device.id));
  const totalStorage = selectedDeviceObjects.reduce((sum, d) => d.size_gb ? sum + d.size_gb : sum, 0);

  return (
    <div className="advisor-container">
      <div className="risk-score">
        <h2>Risk Assessment</h2>
        {(() => {
          // Calculate risk percent and label
          const firstAdvisor = Object.values(advisors)[0];
          let riskPercent = 0;
          let riskValue = '?';
          let riskLabel = 'No Data';
          if (firstAdvisor && firstAdvisor.risk_level) {
            const risk = firstAdvisor.risk_level.toUpperCase();
            if (risk === 'LOW') { riskPercent = 25; riskValue = 25; riskLabel = 'Low Risk'; }
            else if (risk === 'MEDIUM') { riskPercent = 75; riskValue = 75; riskLabel = 'Medium Risk'; }
            else if (risk === 'HIGH') { riskPercent = 100; riskValue = 100; riskLabel = 'High Risk'; }
          }
          return (
            <div
              className="score-circle"
              style={{
                background: `conic-gradient(var(--primary-color) 0% ${riskPercent}%, var(--background-light) ${riskPercent}% 100%)`
              }}
            >
              <div className="score-inner">
                <div className="score-value">{riskValue}</div>
                <div className="score-label">{riskLabel}</div>
              </div>
            </div>
          );
        })()}
        {/* Dynamic summary sentence below risk assessment */}
        <p style={{ color: 'var(--text-secondary)', marginTop: '1rem', marginBottom: '0.5rem' }}>
          {(() => {
            const firstAdvisor = Object.values(advisors)[0];
            if (firstAdvisor && firstAdvisor.method) {
              return (
                <>
                  Based on device type and data sensitivity, we recommend a <b>{firstAdvisor.method}</b> wipe with verification. This recommendation is made after analyzing your device characteristics, compliance requirements, and risk factors.
                </>
              );
            }
            // Generic default statement
            return (
              <>
                Based on device type and data sensitivity, we recommend a secure wipe with verification. This recommendation is made after analyzing your device characteristics, compliance requirements, and risk factors.
              </>
            );
          })()}
        </p>
        {/* Progress bar dynamically reflects backend risk level */}
        <div className="progress-container" style={{ marginBottom: '1rem' }}>
          {(() => {
            const firstAdvisor = Object.values(advisors)[0];
            let percent = 0;
            if (firstAdvisor && firstAdvisor.risk_level) {
              const risk = firstAdvisor.risk_level.toUpperCase();
              if (risk === 'LOW') percent = 25;
              else if (risk === 'MEDIUM') percent = 75;
              else if (risk === 'HIGH') percent = 100;
            }
            // Restore original color (use previous color or CSS variable)
            return (
                <div className="progress-bar" style={{ width: '100%', background: 'var(--background-light)', height: '12px', borderRadius: '6px', overflow: 'hidden' }}>
                  <div style={{ width: `${percent}%`, height: '100%', background: 'linear-gradient(90deg, var(--primary-color), var(--secondary-color))', borderRadius: '6px', transition: 'width 0.5s' }}></div>
                </div>
            );
          })()}
        </div>
      </div>

      <div className="recommendations">
        {/* Method Recommendation */}
        <div className="recommendation-item">
          <span className="material-icons recommendation-icon">check_circle</span>
          <div>
            <h3>Method: {(() => {
              const firstAdvisor = Object.values(advisors)[0];
              if (firstAdvisor && firstAdvisor.method) return firstAdvisor.method;
              // Generic default statement
              return 'A recommended wipe method will be applied.';
            })()}</h3>
            <p style={{ color: 'var(--text-secondary)' }}>
              {(() => {
                const firstAdvisor = Object.values(advisors)[0];
                if (firstAdvisor && firstAdvisor.description) return firstAdvisor.description;
                // Generic default statement
                return 'Your data will be securely wiped according to best practices.';
              })()}
            </p>
          </div>
        </div>
        {/* Verification Recommendation */}
          <div className="recommendation-item">
            <span className="material-icons recommendation-icon">check_circle</span>
            <div>
              {(() => {
                const confidences = COMPLIANCES.map(c => {
                  const advisor = advisors[c.value];
                  return advisor && advisor.confidence !== undefined ? advisor.confidence : null;
                }).filter(c => c !== null);
                const avg = confidences.length > 0 ? (confidences.reduce((a, b) => a + b, 0) / confidences.length) : null;
                const firstAdvisor = Object.values(advisors)[0];
                let verificationEnabled = true;
                let verificationText = 'Your wipe will be checked to confirm completion.';
                if (firstAdvisor && typeof firstAdvisor.verification !== 'undefined') {
                  verificationEnabled = !!firstAdvisor.verification;
                  verificationText = firstAdvisor.verification
                    ? 'Post-wipe verification is enabled.'
                    : 'Verification is not enabled for this method.';
                }
                return (
                  <>
                    <h3>Verification: {verificationEnabled ? 'Enabled' : 'Disabled'}</h3>
                    <p style={{ color: 'var(--text-secondary)' }}>
                      {verificationText}
                      {avg !== null && confidences.length > 0 && (
                        <>
                          <br />
                          Average Confidence: {(avg * 100).toFixed(0)}%
                        </>
                      )}
                    </p>
                  </>
                );
              })()}
            </div>
          </div>
        {/* Estimated Time */}
        <div className="recommendation-item">
          <span className="material-icons recommendation-icon">check_circle</span>
          <div>
            <h3>Estimated Time: {(() => {
              const firstAdvisor = Object.values(advisors)[0];
              if (firstAdvisor && typeof firstAdvisor.estimated_minutes === 'number' && firstAdvisor.estimated_minutes > 0) {
                const min = firstAdvisor.estimated_minutes;
                const hrs = Math.floor(min / 60);
                const mins = min % 60;
                if (hrs > 0 && mins > 0) return `${hrs} hr ${mins} min`;
                if (hrs > 0) return `${hrs} hr`;
                return `${mins} min`;
              }
              return 'N/A';
            })()}</h3>
            <p style={{ color: 'var(--text-secondary)' }}>
              {(() => {
                const firstAdvisor = Object.values(advisors)[0];
                if (firstAdvisor && typeof firstAdvisor.estimated_minutes === 'number' && firstAdvisor.estimated_minutes > 0) {
                  return `Based on backend analysis of device(s) and method.`;
                }
                return 'Based on total storage capacity of selected devices.';
              })()}
            </p>
          </div>
        </div>

        {/* Selected Devices */}
        <div style={{ marginTop: '2rem' }}>
          <h3>Selected Devices:</h3>
          <ul style={{ listStyle: 'none', marginTop: '1rem' }}>
            {selectedDeviceObjects.map(device => (
              <li key={device.id} style={{ marginBottom: '0.5rem', display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                <span className="material-icons" style={{ fontSize: '18px', color: 'var(--primary-color)' }}>storage</span>
                <span>
                  {device.name ? `${device.name}` : 'Device'}
                  {device.size_gb ? `: This device has a total capacity of ${device.size_gb} GB.` : ': Size unknown.'}
                </span>
              </li>
            ))}
          </ul>
        </div>

        {/* Start Wipe Button */}
        <div style={{ marginTop: '2rem', display: 'flex', justifyContent: 'flex-end' }}>
          <button
            className="btn btn-primary"
            onClick={onStartWipe}
            disabled={selectedDeviceObjects.length === 0}
            style={selectedDeviceObjects.length === 0 ? { opacity: 0.5, cursor: 'not-allowed' } : {}}
          >
            <span className="material-icons">security</span>
            Start Wipe Process
          </button>
        </div>
      </div>
    </div>
  );
}

export default WipeAdvisorScreen;
