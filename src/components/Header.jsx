import { useState, useEffect } from 'react';
import './Header.css';

export default function Header({ connected, totalRequests }) {
  const [time, setTime] = useState('');

  useEffect(() => {
    const tick = () => setTime(new Date().toLocaleTimeString('en-GB', { hour12: false }));
    tick();
    const id = setInterval(tick, 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <header className="header">
      <div className="header-left">
        <div className="logo">
          <div className="logo-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3">
              <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
            </svg>
          </div>
          <span className="logo-text">Prompt Simmurator</span>
        </div>
      </div>

      <div className="header-center">
        <div className={`status-dot ${connected ? 'connected' : ''}`}></div>
        <span className="status-text">{connected ? 'Live Monitoring Active' : 'Connecting System...'}</span>
      </div>

      <div className="header-right">
        <div className="req-box">
          <span className="req-label">Traffic Log</span>
          <span className="req-count">{totalRequests.toLocaleString()}</span>
        </div>
        <div className="clock-box">
          {time}
        </div>
      </div>
    </header>
  );
}
