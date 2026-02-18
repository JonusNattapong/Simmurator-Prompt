import { useState, useRef, useEffect, useCallback } from 'react';
import './EndpointList.css';

const schemas = {
  temperature: { sensorId: 'TEMP-001', type: 'temperature', value: 28.5, unit: '°C', location: 'Factory Floor A', status: 'normal' },
  humidity: { sensorId: 'HUM-002', type: 'humidity', value: 62.3, unit: '%RH', location: 'Server Room B', status: 'normal' },
  'oil-level': { sensorId: 'OIL-003', type: 'oil_level', value: 74.1, unit: '%', tankCapacity: '5000L', currentVolume: '3705L', location: 'Storage Tank C', status: 'normal' },
  'oil-pressure': { sensorId: 'OPR-004', type: 'oil_pressure', value: 3.45, unit: 'bar', pipelineId: 'PIPE-MAIN-01', flowRate: '185.2 L/min', location: 'Pipeline Section D', status: 'normal' },
  'air-quality': { sensorId: 'AQI-005', type: 'air_quality', pm25: 42, pm10: 88, co2: 720, unit: 'µg/m³', aqi: 95, location: 'Outdoor Station E', status: 'good' },
  pressure: { sensorId: 'PRS-006', type: 'pressure', value: 1013.2, unit: 'hPa', location: 'Weather Station F', status: 'normal' },
  vibration: { sensorId: 'VIB-007', type: 'vibration', amplitude: 1.234, frequency: 120.5, unit: 'mm/s', machineId: 'CNC-MILL-02', location: 'Machine Shop G', status: 'normal' },
  'energy-meter': { sensorId: 'ENR-008', type: 'energy', power: 22.5, voltage: 230.1, current: 9.78, powerFactor: 0.94, unit: 'kW', location: 'Main Panel H', status: 'normal' },
  amr: {
    sensorId: 'AMR-009',
    type: 'amr_oil_pipeline',
    meterSerial: 'AMR-PIPE-2024-09',
    pipelineId: 'PIPE-AMR-01',
    location: 'Oil Pipeline Station I',
    flowRate: 215.4,
    flowRateUnit: 'L/min',
    flowDirection: 'forward',
    cumulativeFlow: 524381.2,
    cumulativeFlowUnit: 'm³',
    grossVolume: 27450.5,
    netVolume: 27100.8,
    volumeUnit: 'L',
    inletPressure: 6.32,
    outletPressure: 4.18,
    differentialPressure: 2.14,
    pressureUnit: 'bar',
    temperature: 45.3,
    temperatureUnit: '°C',
    viscosity: 32.5,
    viscosityUnit: 'cSt',
    density: 865.2,
    densityUnit: 'kg/m³',
    apiGravity: 32.1,
    waterContent: 0.52,
    waterContentUnit: '%',
    sulfurContent: 0.085,
    sulfurContentUnit: '%',
    correctionFactor: 0.9982,
    pumpSpeed: 1450,
    pumpSpeedUnit: 'RPM',
    valveStatus: 'open',
    valveOpenPercent: 87.5,
    leakDetected: false,
    leakSensitivity: 'high',
    batteryLevel: 78.4,
    batteryUnit: '%',
    signalStrength: -62,
    signalUnit: 'dBm',
    lastCalibration: '2025-12-01T08:00:00.000Z',
    nextCalibrationDue: '2026-06-01T08:00:00.000Z',
    status: 'normal',
    alarmCode: null,
  },
};

// ── WebSocket hook ────────────────────────────────────────────────────────────
function useWebSocket() {
  const wsRef = useRef(null);
  const [wsStatus, setWsStatus] = useState('disconnected'); // disconnected | connecting | connected | error
  const [liveData, setLiveData] = useState({});            // { sensorName: { data, timestamp } }
  const [subscribed, setSubscribed] = useState(new Set()); // Set of sensor names
  const [wsInterval, setWsInterval] = useState(1000);      // ms

  const connect = useCallback(() => {
    if (wsRef.current && wsRef.current.readyState < 2) return; // already open/connecting
    setWsStatus('connecting');
    const ws = new WebSocket(`${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws/sensors`);
    wsRef.current = ws;

    ws.onopen = () => setWsStatus('connected');
    ws.onerror = () => setWsStatus('error');
    ws.onclose = () => {
      setWsStatus('disconnected');
      setSubscribed(new Set());
    };
    ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data);
        if (msg.type === 'data') {
          setLiveData(prev => ({
            ...prev,
            [msg.sensor]: { data: msg.data, timestamp: msg.timestamp },
          }));
        } else if (msg.type === 'subscribed') {
          setSubscribed(new Set(msg.sensors));
          setWsInterval(msg.interval);
        } else if (msg.type === 'unsubscribed') {
          setSubscribed(new Set(msg.remaining));
        }
      } catch { /* ignore */ }
    };
  }, []);

  const disconnect = useCallback(() => {
    wsRef.current?.close();
  }, []);

  const send = useCallback((payload) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(payload));
    }
  }, []);

  const subscribe = useCallback((sensorName, interval) => {
    if (wsRef.current?.readyState !== WebSocket.OPEN) return;
    send({ action: 'subscribe', sensors: [sensorName], interval });
  }, [send]);

  const unsubscribe = useCallback((sensorName) => {
    send({ action: 'unsubscribe', sensors: [sensorName] });
  }, [send]);

  const changeInterval = useCallback((newInterval) => {
    setWsInterval(newInterval);
    // Re-subscribe all with new interval
    if (subscribed.size > 0) {
      send({ action: 'subscribe', sensors: [...subscribed], interval: newInterval });
    }
  }, [send, subscribed]);

  // Cleanup on unmount
  useEffect(() => () => wsRef.current?.close(), []);

  return { wsStatus, liveData, subscribed, wsInterval, connect, disconnect, subscribe, unsubscribe, changeInterval };
}

// ── Component ─────────────────────────────────────────────────────────────────
export default function EndpointList({ endpoints }) {
  const [copied, setCopied] = useState(null);
  const [loading, setLoading] = useState(null);
  const [expanded, setExpanded] = useState(null);
  const [testResults, setTestResults] = useState({});

  const baseUrl = window.location.origin;

  const { wsStatus, liveData, subscribed, wsInterval, connect, disconnect, subscribe, unsubscribe, changeInterval } = useWebSocket();

  const handleCopy = (url) => {
    navigator.clipboard.writeText(`${baseUrl}${url}`);
    setCopied(url);
    setTimeout(() => setCopied(null), 2000);
  };

  const toggleExpand = (name) => {
    setExpanded(expanded === name ? null : name);
  };

  const executeTest = async (e, ep) => {
    e.stopPropagation();
    setLoading(ep.url);
    const start = performance.now();
    try {
      const res = await fetch(ep.url);
      const data = await res.json();
      const time = Math.round(performance.now() - start);
      setTestResults(prev => ({
        ...prev,
        [ep.name]: { status: res.status, time, body: JSON.stringify(data, null, 2) }
      }));
    } catch (err) {
      setTestResults(prev => ({
        ...prev,
        [ep.name]: { status: 0, time: 0, body: `Error: ${err.message}` }
      }));
    }
    setLoading(null);
  };

  const handleLiveToggle = (e, sensorName) => {
    e.stopPropagation();
    if (wsStatus === 'disconnected' || wsStatus === 'error') {
      connect();
      // subscribe after brief connection delay
      setTimeout(() => subscribe(sensorName, wsInterval), 400);
      return;
    }
    if (subscribed.has(sensorName)) {
      unsubscribe(sensorName);
    } else {
      subscribe(sensorName, wsInterval);
    }
  };

  const wsStatusColor = {
    connected: '#22c55e',
    connecting: '#f59e0b',
    error: '#ef4444',
    disconnected: '#6b7280',
  }[wsStatus] ?? '#6b7280';

  const wsStatusLabel = {
    connected: '🔌 WS Connected',
    connecting: '⏳ Connecting...',
    error: '❌ WS Error',
    disconnected: '⚫ WS Disconnected',
  }[wsStatus] ?? '⚫ WS Disconnected';

  return (
    <div className="swagger-panel">
      <div className="swagger-info">
        <h1 className="swagger-title">Simmurator IoT API</h1>
        <div className="swagger-meta">
          <span className="swagger-version">v1.0.0</span>
          <code className="swagger-base">[{baseUrl}]</code>
        </div>
        <p className="swagger-desc">
          This is a simulated IoT API for testing report dashboards and data consumers.
          Use the endpoints below to fetch mock sensor readings.
        </p>

        {/* ── WebSocket Control Bar ── */}
        <div className="ws-control-bar">
          <span className={`ws-status-dot status-${wsStatus}`}>
            {wsStatusLabel}
          </span>

          {wsStatus === 'connected' && (
            <>
              <label className="ws-interval-label">
                Interval:
                <select
                  className="ws-interval-select"
                  value={wsInterval}
                  onChange={(e) => changeInterval(Number(e.target.value))}
                >
                  <option value={100}>100 ms</option>
                  <option value={500}>500 ms</option>
                  <option value={1000}>1 s</option>
                  <option value={2000}>2 s</option>
                  <option value={5000}>5 s</option>
                </select>
              </label>
              <span style={{ fontSize: '0.75rem', color: '#64748b' }}>
                {subscribed.size} sensor{subscribed.size !== 1 ? 's' : ''} streaming
              </span>
              <button className="sw-copy-btn" onClick={disconnect}>
                Disconnect
              </button>
            </>
          )}

          {(wsStatus === 'disconnected' || wsStatus === 'error') && (
            <button className="sw-execute-btn" style={{ marginLeft: 'auto', padding: '4px 14px', fontSize: '0.78rem' }} onClick={connect}>
              Connect WebSocket
            </button>
          )}
        </div>
      </div>

      <div className="swagger-section">
        <h2 className="swagger-section-title">Sensors</h2>
        <div className="swagger-endpoints">
          {endpoints.map((ep) => {
            const isLive = subscribed.has(ep.name);
            const live = liveData[ep.name];

            return (
              <div
                key={ep.url}
                className={`sw-item ${expanded === ep.name ? 'is-expanded' : ''} method-get`}
              >
                <div className="sw-header" onClick={() => toggleExpand(ep.name)}>
                  <span className="sw-method">GET</span>
                  <span className="sw-path">{ep.url}</span>
                  <span className="sw-summary">{ep.description}</span>

                  {/* Live badge */}
                  {isLive && (
                    <span className="sw-live-badge">
                      ● LIVE
                    </span>
                  )}

                  {/* Subscribe/Unsubscribe toggle */}
                  <button
                    className={isLive ? 'sw-copy-btn' : 'sw-execute-btn'}
                    onClick={(e) => handleLiveToggle(e, ep.name)}
                    title={isLive ? 'Unsubscribe from live stream' : 'Subscribe to live stream via WebSocket'}
                    style={{ marginLeft: 'auto', padding: '4px 10px', fontSize: '11px', width: 'auto', minWidth: '60px' }}
                  >
                    {isLive ? 'Stop' : 'Live'}
                  </button>

                  <button className="sw-expand-icon">
                    {expanded === ep.name ? '▲' : '▼'}
                  </button>
                </div>

                {/* ── Live data ticker (always visible when subscribed) ── */}
                {isLive && live && (
                  <div className="sw-live-ticker">
                    <div className="sw-live-ticker-meta">
                      <span style={{ color: '#22c55e', fontWeight: 600 }}>⚡ LIVE</span>
                      <span style={{ color: '#666' }}>{live.timestamp}</span>
                    </div>
                    <pre className="sw-result-body" style={{ maxHeight: 180 }}>
                      <code>{JSON.stringify(live.data, null, 2)}</code>
                    </pre>
                  </div>
                )}

                {expanded === ep.name && (
                  <div className="sw-content">
                    <div className="sw-section-label">Parameters</div>
                    <div className="sw-params-empty">No parameters</div>

                    <div className="sw-responses-wrapper">
                      <div className="sw-section-label">Responses</div>

                      <div className="sw-response-row">
                        <div className="sw-status-code">200</div>
                        <div className="sw-status-desc">Successful operation</div>
                      </div>

                      <div className="sw-schema-tabs">
                        <span className="sw-tab active">Example Value</span>
                        <span className="sw-tab">Model</span>
                      </div>

                      <pre className="sw-schema-box">
                        <code>{JSON.stringify({ status: 'ok', timestamp: '...', data: schemas[ep.name] || {} }, null, 2)}</code>
                      </pre>

                      <div className="sw-try-area">
                        <button
                          className={`sw-execute-btn ${loading === ep.url ? 'loading' : ''}`}
                          onClick={(e) => executeTest(e, ep)}
                          disabled={loading === ep.url}
                        >
                          {loading === ep.url ? 'Executing...' : 'Execute'}
                        </button>
                        <button
                          className="sw-copy-btn"
                          onClick={(e) => { e.stopPropagation(); handleCopy(ep.url); }}
                        >
                          {copied === ep.url ? 'Copied!' : 'Copy URL'}
                        </button>
                      </div>

                      {testResults[ep.name] && (
                        <div className="sw-result-area">
                          <div className="sw-result-meta">
                            <span>Code: <b>{testResults[ep.name].status}</b></span>
                            <span>Time: <b>{testResults[ep.name].time}ms</b></span>
                          </div>
                          <pre className="sw-result-body">
                            <code>{testResults[ep.name].body}</code>
                          </pre>
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
