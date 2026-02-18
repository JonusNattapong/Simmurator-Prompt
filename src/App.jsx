import { useState, useEffect, useRef, useCallback } from 'react';
import Header from './components/Header.jsx';
import EndpointList from './components/EndpointList.jsx';
import AccessLog from './components/AccessLog.jsx';
import StatsPanel from './components/StatsPanel.jsx';
import './App.css';

export default function App() {
  const [endpoints, setEndpoints] = useState([]);
  const [accessEntries, setAccessEntries] = useState([]);
  const [totalRequests, setTotalRequests] = useState(0);
  const [stats, setStats] = useState(null);
  const [connected, setConnected] = useState(false);
  const eventSourceRef = useRef(null);

  // Fetch endpoints list
  useEffect(() => {
    fetch('/api/v1/endpoints')
      .then(r => r.json())
      .then(data => setEndpoints(data.endpoints || []))
      .catch(() => {});
  }, []);

  // Fetch initial access log & stats
  const refreshData = useCallback(() => {
    fetch('/api/v1/access-log?limit=100')
      .then(r => r.json())
      .then(data => {
        setAccessEntries(data.entries || []);
        setTotalRequests(data.total || 0);
      })
      .catch(() => {});

    fetch('/api/v1/stats')
      .then(r => r.json())
      .then(data => setStats(data))
      .catch(() => {});
  }, []);

  useEffect(() => {
    refreshData();
    const interval = setInterval(refreshData, 5000);
    return () => clearInterval(interval);
  }, [refreshData]);

  // SSE: listen for real-time access events
  useEffect(() => {
    const es = new EventSource('/events');
    eventSourceRef.current = es;

    es.onopen = () => setConnected(true);
    es.onerror = () => setConnected(false);

    es.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data);
        if (payload.type === 'connected') {
          setConnected(true);
        } else if (payload.type === 'access') {
          setAccessEntries(prev => {
            const next = [payload.data, ...prev];
            return next.slice(0, 200);
          });
          setTotalRequests(payload.data.id);
        }
      } catch { /* ignore */ }
    };

    return () => es.close();
  }, []);

  return (
    <div className="app">
      <div className="bg-effects">
        <div className="bg-grid" />
        <div className="bg-orb bg-orb-1" />
        <div className="bg-orb bg-orb-2" />
      </div>

      <Header connected={connected} totalRequests={totalRequests} />

      <main className="main">
        <div className="main-top">
          <StatsPanel stats={stats} totalRequests={totalRequests} />
        </div>

        <div className="main-grid">
          <EndpointList endpoints={endpoints} />
          <AccessLog entries={accessEntries} />
        </div>
      </main>
    </div>
  );
}
