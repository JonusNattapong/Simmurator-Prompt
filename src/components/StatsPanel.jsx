import './StatsPanel.css';

const SENSOR_ICONS = {
  temperature: '🌡️',
  humidity: '💧',
  'oil-level': '🛢️',
  'oil-pressure': '⛽',
  'air-quality': '🌬️',
  pressure: '📊',
  vibration: '📳',
  'energy-meter': '⚡',
};

export default function StatsPanel({ stats, totalRequests }) {
  const endpointStats = stats?.endpointStats || {};

  // Compute aggregates
  let totalErrors = 0;
  let totalAvgTime = 0;
  let endpointCount = 0;

  const rows = Object.entries(endpointStats)
    .filter(([key]) => key.includes('/sensors/'))
    .map(([endpoint, data]) => {
      totalErrors += data.errors;
      totalAvgTime += data.avgResponseTime;
      endpointCount++;

      const sensorKey = endpoint.split('/sensors/')[1]?.split('?')[0] || endpoint;
      return { endpoint, sensorKey, ...data };
    })
    .sort((a, b) => b.count - a.count);

  const avgOverall = endpointCount > 0 ? Math.round(totalAvgTime / endpointCount) : 0;
  const activeConnections = stats?.activeConnections || 0;

  return (
    <div className="stats-container">
      {/* Summary Cards */}
      <div className="stats-cards">
        <div className="s-card">
          <div className="s-card-icon s-blue">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
            </svg>
          </div>
          <div className="s-card-info">
            <span className="s-card-value">{totalRequests.toLocaleString()}</span>
            <span className="s-card-label">Total Requests</span>
          </div>
        </div>

        <div className="s-card">
          <div className="s-card-icon s-green">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" />
              <polyline points="12 6 12 12 16 14" />
            </svg>
          </div>
          <div className="s-card-info">
            <span className="s-card-value">{avgOverall}ms</span>
            <span className="s-card-label">Avg Response</span>
          </div>
        </div>

        <div className="s-card">
          <div className="s-card-icon s-red">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" />
              <line x1="15" y1="9" x2="9" y2="15" />
              <line x1="9" y1="9" x2="15" y2="15" />
            </svg>
          </div>
          <div className="s-card-info">
            <span className="s-card-value">{totalErrors}</span>
            <span className="s-card-label">Errors</span>
          </div>
        </div>

        <div className="s-card">
          <div className="s-card-icon s-purple">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4-4v2" />
              <circle cx="9" cy="7" r="4" />
              <path d="M23 21v-2a4 4 0 00-3-3.87" />
              <path d="M16 3.13a4 4 0 010 7.75" />
            </svg>
          </div>
          <div className="s-card-info">
            <span className="s-card-value">{activeConnections}</span>
            <span className="s-card-label">SSE Clients</span>
          </div>
        </div>
      </div>

      {/* Per-endpoint breakdown */}
      {rows.length > 0 && (
        <div className="panel stats-table-panel">
          <div className="panel-header">
            <h2 className="panel-title">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                <line x1="3" y1="9" x2="21" y2="9" />
                <line x1="9" y1="21" x2="9" y2="9" />
              </svg>
              Per-Endpoint Stats
            </h2>
          </div>
          <div className="stats-rows">
            {rows.map((r) => (
              <div className="stats-row" key={r.endpoint}>
                <span className="stats-row-icon">{SENSOR_ICONS[r.sensorKey] || '📦'}</span>
                <code className="stats-row-ep">{r.sensorKey}</code>
                <div className="stats-row-bar-wrap">
                  <div
                    className="stats-row-bar"
                    style={{ width: `${Math.min((r.count / (rows[0]?.count || 1)) * 100, 100)}%` }}
                  />
                </div>
                <span className="stats-row-count">{r.count} req</span>
                <span className="stats-row-avg">{r.avgResponseTime}ms</span>
                {r.errors > 0 && <span className="stats-row-err">{r.errors} err</span>}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
