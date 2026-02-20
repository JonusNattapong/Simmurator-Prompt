import { useState, useRef, useEffect, useCallback } from 'react';
import './css/EndpointList.css';

// ISA-95 + OPC UA + MQTT Sparkplug B Unified Schema
const unifiedSchema = {
  opcUa: {
    nodeId: 'ns=2;s=SENSOR-001',
    browseName: '2:SENSOR-001',
    displayName: 'Sensor Display Name',
    namespaceIndex: 2,
  },
  equipmentHierarchy: {
    site: 'Thailand-Plant-01',
    area: 'Area-Name',
    line: 'Production-Line',
    unit: 'Unit-Name',
    equipment: 'SENSOR-001',
  },
  sparkplugTopic: {
    version: 'spBv1.0',
    groupId: 'Plant-01',
    messageType: 'DDATA',
    edgeNodeId: 'Edge-Node-01',
    deviceId: 'SENSOR-001',
  },
  sourceTimestamp: '2025-02-20T08:30:00.000Z',
  serverTimestamp: '2025-02-20T08:30:00.000Z',
  value: {},
  dataQuality: 'good',
  opcUaStatusCode: 'good',
  unit: { code: 'Cel', display: '°C' },
  sensorType: 'sensor_type',
  description: 'Sensor description',
  properties: {},
};

// All 14 endpoints with unified ISA-95/OPC UA schema
const schemas = {
  temperature: {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=TEMP-001', browseName: '2:TEMP-001', displayName: 'Temperature Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Factory-Floor-A', line: 'Production-Line-1', unit: 'Production-Line-1-Unit', equipment: 'TEMP-001' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'TEMP-001' },
    value: { value: 24.5, minThreshold: 18.0, maxThreshold: 27.0, criticalHigh: 32.0, criticalLow: 15.0 },
    unit: { code: 'Cel', display: '°C' },
    sensorType: 'temperature',
    description: 'Industrial temperature sensor',
  },
  humidity: {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=HUM-002', browseName: '2:HUM-002', displayName: 'Humidity Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'IT-Infrastructure', line: 'Server-Room-B', unit: 'Server-Room-B-Unit', equipment: 'HUM-002' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'HUM-002' },
    value: { value: 52.3, optimalMin: 40.0, optimalMax: 60.0, allowableMin: 20.0, allowableMax: 80.0, dewPoint: 14.2 },
    unit: { code: '%', display: '%RH' },
    sensorType: 'humidity',
    description: 'Relative humidity sensor',
  },
  'oil-level': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=OIL-003', browseName: '2:OIL-003', displayName: 'Oil Level Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Tank-Farm', line: 'Storage-Tank-C', unit: 'Storage-Tank-C-Unit', equipment: 'OIL-003' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'OIL-003' },
    value: { value: 74.1, tankCapacityLiters: 25000, tankCapacityM3: 25.0, currentVolumeLiters: 18525, currentVolumeM3: 18.53, lowAlarmThreshold: 10.0, highAlarmThreshold: 95.0 },
    unit: { code: '%', display: '%' },
    sensorType: 'oil_level',
    description: 'Industrial oil level sensor',
  },
  'oil-pressure': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=OPR-004', browseName: '2:OPR-004', displayName: 'Oil Pressure Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Process-Area', line: 'Pipeline-D', unit: 'Pipeline-D-Unit', equipment: 'OPR-004' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'OPR-004' },
    value: { value: 85.5, flowRateLpm: 285.5, operatingRange: '10-200 bar', maxWorkingPressure: 250.0 },
    unit: { code: 'bar', display: 'bar' },
    sensorType: 'oil_pressure',
    description: 'Hydraulic oil pressure sensor',
  },
  'air-quality': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=AQI-005', browseName: '2:AQI-005', displayName: 'Air Quality Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Environment', line: 'Outdoor-Station-E', unit: 'Outdoor-Station-E-Unit', equipment: 'AQI-005' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'AQI-005' },
    value: { pm25: 18.5, pm10: 42.3, co2: 685, voc: 0.45, aqi: 65, whoPm25Guideline: 15.0, whoPm10Guideline: 45.0, co2Threshold: 1000.0 },
    unit: { code: 'ug/m3', display: 'µg/m³' },
    sensorType: 'air_quality',
    description: 'Multi-parameter air quality sensor',
  },
  pressure: {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=PRS-006', browseName: '2:PRS-006', displayName: 'Atmospheric Pressure Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Environment', line: 'Weather-Station-F', unit: 'Weather-Station-F-Unit', equipment: 'PRS-006' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'PRS-006' },
    value: { value: 1012.5, seaLevelPressure: 1013.8, altitudeMeters: 12.5, standardPressure: 1013.25, trend: 'rising' },
    unit: { code: 'hPa', display: 'hPa' },
    sensorType: 'pressure',
    description: 'Atmospheric pressure sensor',
  },
  vibration: {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=VIB-007', browseName: '2:VIB-007', displayName: 'Vibration Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Machine-Shop', line: 'CNC-Machine-02', unit: 'CNC-Machine-02-Unit', equipment: 'VIB-007' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'VIB-007' },
    value: { velocityRms: 3.45, frequency: 120.5, acceleration: 2.612, displacement: 0.0046, machineType: 'Class II (Medium machines)', iso10816Limits: { good: 2.8, satisfactory: 7.1, unsatisfactory: 18.0 } },
    unit: { code: 'mm/s', display: 'mm/s' },
    sensorType: 'vibration',
    description: 'ISO 10816 vibration monitoring sensor',
  },
  'energy-meter': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=ENR-008', browseName: '2:ENR-008', displayName: 'Energy Meter' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Electrical', line: 'Main-Panel-H', unit: 'Main-Panel-H-Unit', equipment: 'ENR-008' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'ENR-008' },
    value: { activePower: 45.25, apparentPower: 52.8, reactivePower: 27.15, voltageL1: 230.5, voltageL3: 399.2, current: 78.5, powerFactor: 0.857, frequency: 50.02, cumulativeEnergy: 125847.5 },
    unit: { code: 'kW', display: 'kW' },
    sensorType: 'energy',
    description: '3-phase power quality meter',
  },
  amr: {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=AMR-009', browseName: '2:AMR-009', displayName: 'AMR Oil Pipeline Meter' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Oil-Gas', line: 'Pipeline-Station', unit: 'Pipeline-Station-Unit', equipment: 'AMR-009' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'AMR-009' },
    value: { meterSerial: 'AMR-PIPE-2024-09', pipelineId: 'PIPE-AMR-01', location: 'Map Ta Phut Refinery Station', province: 'ระยอง', coordinates: { lat: 12.6517, lng: 101.1595 }, flowRate: 20833.5, flowRateM3H: 1250.0, flowDirection: 'forward', cumulativeFlow: 15243891.2, inletPressure: 52.4, outletPressure: 38.6, differentialPressure: 13.8, temperature: 58.5, apiGravity: 31.2, density: 868.4, viscosity: 45.8, waterContent: 0.85, pumpSpeed: 1450, valveStatus: 'open', valveOpenPercent: 92.5, leakDetected: false, batteryLevel: 87.3, signalStrength: -68, lastCalibration: '2025-01-15T08:00:00.000Z', nextCalibrationDue: '2025-07-15T08:00:00.000Z' },
    unit: { code: 'L/min', display: 'L/min' },
    sensorType: 'amr_oil_pipeline',
    description: 'Automatic meter reading for oil pipeline',
  },
  'flow-meter': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=FLW-010', browseName: '2:FLW-010', displayName: 'Flow Meter' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Process', line: 'Process-Line-J', unit: 'Process-Line-J-Unit', equipment: 'FLW-010' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'FLW-010' },
    value: { mediaType: 'liquid', flowRate: 485.5, totalizer: 125847.5, temperature: 45.2, pressure: 8.5, density: 985.5, pipeSize: 150, meterType: 'electromagnetic' },
    unit: { code: 'm3/h', display: 'm³/h' },
    sensorType: 'flow_meter',
    description: 'Industrial flow measurement',
  },
  'gas-detector': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=GAS-011', browseName: '2:GAS-011', displayName: 'Gas Detector' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Safety', line: 'Confined-Space-K', unit: 'Confined-Space-K-Unit', equipment: 'GAS-011' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'GAS-011' },
    value: { carbonMonoxide: 12.5, coAlarmSetpoint: 35.0, hydrogenSulfide: 3.25, h2sAlarmSetpoint: 10.0, oxygen: 20.9, o2LowAlarm: 19.5, o2HighAlarm: 23.5, lel: 5.5, lelAlarmSetpoint: 10.0, alarms: { co: false, h2s: false, o2: false, lel: false } },
    unit: { code: 'ppm', display: 'ppm' },
    sensorType: 'gas_detector',
    description: '4-gas safety monitor',
  },
  'ph-sensor': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=PH-012', browseName: '2:PH-012', displayName: 'pH Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Water', line: 'Water-Treatment-L', unit: 'Water-Treatment-L-Unit', equipment: 'PH-012' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'PH-012' },
    value: { phValue: 7.25, orp: 125.5, temperature: 24.5, conductivity: 1250.5, turbidity: 2.85 },
    unit: { code: 'pH', display: 'pH' },
    sensorType: 'ph_sensor',
    description: 'Water quality pH/ORP sensor',
  },
  'level-sensor': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=LVL-013', browseName: '2:LVL-013', displayName: 'Level Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Tank-Farm', line: 'Storage-Tank-M', unit: 'Storage-Tank-M-Unit', equipment: 'LVL-013' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'LVL-013' },
    value: { level: 8.525, tankHeight: 12.0, percentage: 71.04, volume: 85.25, sensorType: 'ultrasonic', accuracy: '±3mm' },
    unit: { code: 'm', display: 'm' },
    sensorType: 'level_sensor',
    description: 'Tank level measurement sensor',
  },
  'proximity-sensor': {
    ...unifiedSchema,
    opcUa: { ...unifiedSchema.opcUa, nodeId: 'ns=2;s=PRX-014', browseName: '2:PRX-014', displayName: 'Proximity Sensor' },
    equipmentHierarchy: { ...unifiedSchema.equipmentHierarchy, area: 'Material-Handling', line: 'Conveyor-Station-N', unit: 'Conveyor-Station-N-Unit', equipment: 'PRX-014' },
    sparkplugTopic: { ...unifiedSchema.sparkplugTopic, deviceId: 'PRX-014' },
    value: { objectDetected: true, distance: 25.5, sensorType: 'inductive', detectionRange: 50.0, responseTime: 2.5, switchingFrequency: 1500, detectionCount: 45872, operatingTime: 25847.5 },
    unit: { code: 'mm', display: 'mm' },
    sensorType: 'proximity_sensor',
    description: 'Object detection proximity sensor',
  },
};

// ── WebSocket hook ────────────────────────────────────────────────────────────
function useWebSocket() {
  const wsRef = useRef(null);
  const [wsStatus, setWsStatus] = useState('disconnected');
  const [liveData, setLiveData] = useState({});
  const [subscribed, setSubscribed] = useState(new Set());
  const [wsInterval, setWsInterval] = useState(1000);

  const connect = useCallback(() => {
    if (wsRef.current && wsRef.current.readyState < 2) return;
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

  const subscribe = useCallback((sensor) => {
    send({ action: 'subscribe', sensor });
  }, [send]);

  const unsubscribe = useCallback((sensor) => {
    send({ action: 'unsubscribe', sensor });
  }, [send]);

  return { wsStatus, connect, disconnect, subscribe, unsubscribe, liveData, subscribed, wsInterval };
}

// ── Components ───────────────────────────────────────────────────────────────

function StatusBadge({ status }) {
  const map = {
    good: 'status-good',
    uncertain: 'status-warn',
    bad: 'status-bad',
    normal: 'status-good',
    optimal: 'status-good',
    warning: 'status-warn',
    alarm: 'status-bad',
    detected: 'status-good',
    clear: 'status-idle',
  };
  return <span className={`status-badge ${map[status] || 'status-idle'}`}>{status}</span>;
}

function DataQualityBadge({ quality }) {
  const map = {
    good: 'status-good',
    uncertain: 'status-warn',
    bad: 'status-bad',
    GoodUncertain: 'status-warn',
  };
  return <span className={`status-badge ${map[quality] || 'status-idle'}`}>{quality}</span>;
}

function Timestamp({ label, value }) {
  if (!value) return null;
  const date = new Date(value);
  const formatted = date.toLocaleString('th-TH', { 
    hour12: false,
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  });
  return (
    <div className="timestamp-row">
      <span className="timestamp-label">{label}:</span>
      <span className="timestamp-value">{formatted}</span>
    </div>
  );
}

function EquipmentHierarchy({ hierarchy }) {
  return (
    <div className="hierarchy-section">
      <h4>ISA-95 Equipment Hierarchy</h4>
      <div className="hierarchy-tree">
        <div className="hierarchy-item"><span className="hierarchy-label">Site:</span> {hierarchy.site}</div>
        <div className="hierarchy-item"><span className="hierarchy-label">Area:</span> {hierarchy.area}</div>
        <div className="hierarchy-item"><span className="hierarchy-label">Line:</span> {hierarchy.line}</div>
        <div className="hierarchy-item"><span className="hierarchy-label">Unit:</span> {hierarchy.unit}</div>
        <div className="hierarchy-item"><span className="hierarchy-label">Equipment:</span> {hierarchy.equipment}</div>
      </div>
    </div>
  );
}

function OpcUaInfo({ opcUa }) {
  return (
    <div className="opcua-section">
      <h4>OPC UA Information Model</h4>
      <div className="opcua-grid">
        <div className="opcua-item"><span className="opcua-label">NodeId:</span> <code>{opcUa.nodeId}</code></div>
        <div className="opcua-item"><span className="opcua-label">BrowseName:</span> <code>{opcUa.browseName}</code></div>
        <div className="opcua-item"><span className="opcua-label">DisplayName:</span> {opcUa.displayName}</div>
        <div className="opcua-item"><span className="opcua-label">Namespace:</span> {opcUa.namespaceIndex}</div>
      </div>
    </div>
  );
}

function SparkplugTopic({ topic }) {
  const fullTopic = `${topic.version}/${topic.groupId}/${topic.messageType}/${topic.edgeNodeId}/${topic.deviceId}`;
  return (
    <div className="sparkplug-section">
      <h4>MQTT Sparkplug B</h4>
      <div className="sparkplug-topic">
        <span className="sparkplug-label">Topic:</span>
        <code className="sparkplug-value">{fullTopic}</code>
      </div>
    </div>
  );
}

function ValueDisplay({ value, unit }) {
  if (typeof value === 'object' && value !== null) {
    return (
      <div className="value-object">
        {Object.entries(value).map(([key, val]) => (
          <div key={key} className="value-item">
            <span className="value-key">{key}:</span>
            <span className="value-val">
              {typeof val === 'object' ? JSON.stringify(val) : String(val)}
            </span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="value-simple">
      <span className="value-number">{value}</span>
      <span className="value-unit">{unit?.display || unit}</span>
    </div>
  );
}

function SensorCard({ name, schema, live, isSubscribed, onToggle }) {
  const data = live?.data || schema;
  const isLive = !!live?.data;

  return (
    <div className={`sensor-card ${isLive ? 'live' : ''}`}>
      <div className="sensor-header">
        <div className="sensor-title">
          <h3>{name}</h3>
          <span className="sensor-type">{data.sensorType}</span>
        </div>
        <div className="sensor-actions">
          <button
            className={`btn-subscribe ${isSubscribed ? 'subscribed' : ''}`}
            onClick={() => onToggle(name)}
          >
            {isSubscribed ? 'Unsubscribe' : 'Subscribe'}
          </button>
        </div>
      </div>

      <div className="sensor-description">{data.description}</div>

      <div className="sensor-main">
        <div className="value-section">
          <h4>Value</h4>
          <ValueDisplay value={data.value} unit={data.unit} />
        </div>

        <div className="quality-section">
          <h4>Data Quality</h4>
          <div className="quality-row">
            <DataQualityBadge quality={data.dataQuality} />
            <span className="status-code">({data.opcUaStatusCode})</span>
          </div>
        </div>
      </div>

      <div className="sensor-details">
        <EquipmentHierarchy hierarchy={data.equipmentHierarchy} />
        <OpcUaInfo opcUa={data.opcUa} />
        <SparkplugTopic topic={data.sparkplugTopic} />
        
        <div className="timestamps-section">
          <h4>Timestamps</h4>
          <Timestamp label="Source" value={data.sourceTimestamp} />
          <Timestamp label="Server" value={data.serverTimestamp} />
        </div>

        <div className="unit-section">
          <h4>Unit (UCUM)</h4>
          <div className="unit-info">
            <span className="unit-code">Code: <code>{data.unit?.code}</code></span>
            <span className="unit-display">Display: {data.unit?.display}</span>
          </div>
        </div>
      </div>

      {isLive && (
        <div className="live-indicator">
          <span className="live-dot"></span>
          <span className="live-text">LIVE</span>
          <span className="live-time">{new Date(live.timestamp).toLocaleTimeString('th-TH')}</span>
        </div>
      )}
    </div>
  );
}

// ── Main Component ───────────────────────────────────────────────────────────
export default function EndpointList() {
  const [endpoints, setEndpoints] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const { wsStatus, connect, disconnect, subscribe, unsubscribe, liveData, subscribed } = useWebSocket();

  useEffect(() => {
    fetch('/api/v1/endpoints')
      .then(r => r.json())
      .then(d => {
        if (d.status === 'ok') {
          setEndpoints(d.endpoints);
        } else {
          setError('Failed to load endpoints');
        }
        setLoading(false);
      })
      .catch(() => {
        setError('Failed to load endpoints');
        setLoading(false);
      });
  }, []);

  useEffect(() => {
    if (wsStatus === 'connected') {
      endpoints.forEach(ep => subscribe(ep.name));
    }
    return () => {
      if (wsStatus === 'connected') {
        endpoints.forEach(ep => unsubscribe(ep.name));
      }
    };
  }, [wsStatus, endpoints, subscribe, unsubscribe]);

  const toggleSubscription = (name) => {
    if (subscribed.has(name)) {
      unsubscribe(name);
    } else {
      subscribe(name);
    }
  };

  if (loading) return <div className="endpoint-loading">Loading endpoints…</div>;
  if (error) return <div className="endpoint-error">{error}</div>;

  return (
    <div className="endpoint-container">
      <header className="endpoint-header">
        <h1>Industrial IoT Sensors</h1>
        <p className="standards-info">ISA-95 | OPC UA | MQTT Sparkplug B | UCUM</p>
        <div className="ws-controls">
          <span className={`ws-status ${wsStatus}`}>WebSocket: {wsStatus}</span>
          {wsStatus === 'disconnected' && (
            <button className="btn-connect" onClick={connect}>Connect</button>
          )}
          {wsStatus === 'connected' && (
            <button className="btn-disconnect" onClick={disconnect}>Disconnect</button>
          )}
        </div>
      </header>

      <div className="endpoint-grid">
        {endpoints.map(ep => (
          <SensorCard
            key={ep.name}
            name={ep.name}
            schema={schemas[ep.name] || unifiedSchema}
            live={liveData[ep.name]}
            isSubscribed={subscribed.has(ep.name)}
            onToggle={toggleSubscription}
          />
        ))}
      </div>
    </div>
  );
}
