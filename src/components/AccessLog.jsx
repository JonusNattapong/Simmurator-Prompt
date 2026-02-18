import { useRef, useEffect } from 'react';
import './AccessLog.css';

function timeAgo(ts) {
  const diff = Date.now() - new Date(ts).getTime();
  if (diff < 1000) return 'just now';
  if (diff < 60000) return `${Math.floor(diff / 1000)}s ago`;
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  return `${Math.floor(diff / 3600000)}h ago`;
}

function statusClass(code) {
  if (code >= 500) return 'err';
  if (code >= 400) return 'warn';
  if (code === 304) return 'cache';
  return 'ok';
}

function parseUA(ua) {
  if (!ua || ua === 'unknown') return 'Unknown Client';
  if (ua.includes('curl')) return 'cURL';
  if (ua.includes('Postman')) return 'Postman';
  if (ua.includes('python')) return 'Python';
  if (ua.includes('axios')) return 'Axios';
  if (ua.includes('node-fetch') || ua.includes('undici')) return 'Node.js';
  if (ua.includes('Chrome')) return 'Chrome';
  if (ua.includes('Firefox')) return 'Firefox';
  if (ua.includes('Safari')) return 'Safari';
  if (ua.includes('Edge')) return 'Edge';
  return ua.slice(0, 20);
}

export default function AccessLog({ entries }) {
  const listRef = useRef(null);

  // Auto-scroll to top on new entry
  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = 0;
    }
  }, [entries.length]);

  return (
    <div className="panel log-panel">
      <div className="panel-header">
        <h2 className="panel-title">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
            <polyline points="14 2 14 8 20 8" />
            <line x1="16" y1="13" x2="8" y2="13" />
            <line x1="16" y1="17" x2="8" y2="17" />
            <polyline points="10 9 9 9 8 9" />
          </svg>
          Access Log
        </h2>
        <span className="panel-badge">{entries.length} entries</span>
      </div>

      <div className="log-table-head">
        <span className="log-col log-col-time">Time</span>
        <span className="log-col log-col-method">Method</span>
        <span className="log-col log-col-endpoint">Endpoint</span>
        <span className="log-col log-col-status">Status</span>
        <span className="log-col log-col-ms">Resp</span>
        <span className="log-col log-col-ip">IP Address</span>
        <span className="log-col log-col-ua">Client</span>
      </div>

      <div className="log-list" ref={listRef}>
        {entries.length === 0 && (
          <div className="log-empty">
            <span className="log-empty-icon">📡</span>
            <p>No requests yet</p>
            <p className="log-empty-hint">Try hitting an API endpoint to see access logs here</p>
          </div>
        )}
        {entries.map((entry) => (
          <div className="log-row" key={entry.id}>
            <span className="log-col log-col-time" title={entry.timestamp}>
              {timeAgo(entry.timestamp)}
            </span>
            <span className="log-col log-col-method">
              <span className="log-method-badge">{entry.method}</span>
            </span>
            <span className="log-col log-col-endpoint">
              <code>{entry.endpoint}</code>
            </span>
            <span className="log-col log-col-status">
              <span className={`log-status ${statusClass(entry.statusCode)}`}>
                {entry.statusCode}
              </span>
            </span>
            <span className="log-col log-col-ms">
              <span className={`log-ms ${entry.responseTime > 200 ? 'slow' : ''}`}>
                {entry.responseTime}ms
              </span>
            </span>
            <span className="log-col log-col-ip">
              <code>{entry.ip}</code>
            </span>
            <span className="log-col log-col-ua" title={entry.userAgent}>
              {parseUA(entry.userAgent)}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
