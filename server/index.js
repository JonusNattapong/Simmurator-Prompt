import express from "express";
import cors from "cors";
import { createServer } from "http";
import { WebSocketServer } from "ws";

const app = express();
const PORT = 3001;

app.use(cors());
app.use(express.json());

// ──────────────────────────────────────────────
// In-memory stores
// ──────────────────────────────────────────────
const accessLog = []; // { id, timestamp, ip, userAgent, endpoint, method, statusCode, responseTime, deviceId }
const MAX_LOG = 500;
let sseClients = []; // SSE subscribers
let requestCounter = 0;

// ──────────────────────────────────────────────
// IoT Sensor Simulators
// ──────────────────────────────────────────────
function randomBetween(min, max, decimals = 2) {
  return parseFloat((Math.random() * (max - min) + min).toFixed(decimals));
}

const sensorGenerators = {
  temperature: () => ({
    sensorId: "TEMP-001",
    type: "temperature",
    value: randomBetween(22.0, 38.5, 1),
    unit: "°C",
    location: "Factory Floor A",
    status: Math.random() > 0.05 ? "normal" : "warning",
  }),
  humidity: () => ({
    sensorId: "HUM-002",
    type: "humidity",
    value: randomBetween(30.0, 85.0, 1),
    unit: "%RH",
    location: "Server Room B",
    status: Math.random() > 0.1 ? "normal" : "warning",
  }),
  "oil-level": () => ({
    sensorId: "OIL-003",
    type: "oil_level",
    value: randomBetween(15.0, 98.0, 1),
    unit: "%",
    tankCapacity: "5000L",
    currentVolume: `${randomBetween(750, 4900, 0)}L`,
    location: "Storage Tank C",
    status: Math.random() > 0.08 ? "normal" : "critical",
  }),
  "oil-pressure": () => ({
    sensorId: "OPR-004",
    type: "oil_pressure",
    value: randomBetween(1.5, 6.8, 2),
    unit: "bar",
    pipelineId: "PIPE-MAIN-01",
    flowRate: `${randomBetween(50, 320, 1)} L/min`,
    location: "Pipeline Section D",
    status: Math.random() > 0.06 ? "normal" : "warning",
  }),
  "air-quality": () => ({
    sensorId: "AQI-005",
    type: "air_quality",
    pm25: randomBetween(5, 120, 0),
    pm10: randomBetween(10, 200, 0),
    co2: randomBetween(350, 1200, 0),
    unit: "µg/m³",
    aqi: randomBetween(10, 180, 0),
    location: "Outdoor Station E",
    status: Math.random() > 0.15 ? "good" : "moderate",
  }),
  pressure: () => ({
    sensorId: "PRS-006",
    type: "pressure",
    value: randomBetween(980, 1050, 1),
    unit: "hPa",
    location: "Weather Station F",
    status: "normal",
  }),
  vibration: () => ({
    sensorId: "VIB-007",
    type: "vibration",
    amplitude: randomBetween(0.01, 5.5, 3),
    frequency: randomBetween(10, 500, 1),
    unit: "mm/s",
    machineId: "CNC-MILL-02",
    location: "Machine Shop G",
    status: Math.random() > 0.1 ? "normal" : "alert",
  }),
  "energy-meter": () => ({
    sensorId: "ENR-008",
    type: "energy",
    power: randomBetween(1.2, 45.0, 2),
    voltage: randomBetween(218, 242, 1),
    current: randomBetween(0.5, 18.0, 2),
    powerFactor: randomBetween(0.85, 0.99, 2),
    unit: "kW",
    location: "Main Panel H",
    status: "normal",
  }),
  amr: () => ({
    sensorId: "AMR-009",
    type: "amr_oil_pipeline",
    meterSerial: "AMR-PIPE-2024-09",
    pipelineId: "PIPE-AMR-01",
    location: "Oil Pipeline Station I",
    // Flow
    flowRate: randomBetween(80.0, 350.0, 2),
    flowRateUnit: "L/min",
    flowDirection: Math.random() > 0.05 ? "forward" : "reverse",
    cumulativeFlow: randomBetween(100000, 999999, 1),
    cumulativeFlowUnit: "m³",
    grossVolume: randomBetween(5000, 50000, 1),
    netVolume: randomBetween(4900, 49500, 1),
    volumeUnit: "L",
    // Pressure
    inletPressure: randomBetween(4.0, 8.5, 2),
    outletPressure: randomBetween(2.5, 6.0, 2),
    differentialPressure: randomBetween(0.5, 3.5, 2),
    pressureUnit: "bar",
    // Temperature & Physical Properties
    temperature: randomBetween(28.0, 65.0, 1),
    temperatureUnit: "°C",
    viscosity: randomBetween(5.0, 120.0, 2),
    viscosityUnit: "cSt",
    density: randomBetween(820.0, 920.0, 1),
    densityUnit: "kg/m³",
    apiGravity: randomBetween(20.0, 45.0, 1),
    // Oil Quality
    waterContent: randomBetween(0.01, 5.0, 2),
    waterContentUnit: "%",
    sulfurContent: randomBetween(0.01, 2.5, 3),
    sulfurContentUnit: "%",
    correctionFactor: randomBetween(0.99, 1.01, 4),
    // Pump & Valve
    pumpSpeed: randomBetween(800, 3000, 0),
    pumpSpeedUnit: "RPM",
    valveStatus: Math.random() > 0.1 ? "open" : "closed",
    valveOpenPercent: randomBetween(60, 100, 1),
    // Safety & Diagnostics
    leakDetected: Math.random() > 0.97,
    leakSensitivity: "high",
    batteryLevel: randomBetween(20.0, 100.0, 1),
    batteryUnit: "%",
    signalStrength: randomBetween(-90, -40, 0),
    signalUnit: "dBm",
    lastCalibration: "2025-12-01T08:00:00.000Z",
    nextCalibrationDue: "2026-06-01T08:00:00.000Z",
    // Status
    status: Math.random() > 0.06 ? "normal" : "warning",
    alarmCode: Math.random() > 0.95 ? "ALM-FLOW-HIGH" : null,
  }),
};

const ENDPOINTS = Object.keys(sensorGenerators);

// ──────────────────────────────────────────────
// Middleware: Log access
// ──────────────────────────────────────────────
function logAccess(req, res, next) {
  const start = Date.now();

  res.on("finish", () => {
    const entry = {
      id: ++requestCounter,
      timestamp: new Date().toISOString(),
      ip: req.headers["x-forwarded-for"] || req.ip || req.socket.remoteAddress,
      userAgent: req.headers["user-agent"] || "unknown",
      endpoint: req.originalUrl,
      method: req.method,
      statusCode: res.statusCode,
      responseTime: Date.now() - start,
      deviceId: req.headers["x-device-id"] || null,
    };

    accessLog.unshift(entry);
    if (accessLog.length > MAX_LOG) accessLog.length = MAX_LOG;

    // Push to SSE clients
    broadcastSSE({ type: "access", data: entry });
  });

  next();
}

// ──────────────────────────────────────────────
// SSE: Server-Sent Events for real-time log stream
// ──────────────────────────────────────────────
function broadcastSSE(payload) {
  const data = `data: ${JSON.stringify(payload)}\n\n`;
  sseClients = sseClients.filter((client) => {
    try {
      client.write(data);
      return true;
    } catch {
      return false;
    }
  });
}

app.get("/events", (req, res) => {
  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache",
    Connection: "keep-alive",
    "Access-Control-Allow-Origin": "*",
  });
  res.write(
    `data: ${JSON.stringify({ type: "connected", message: "SSE stream connected" })}\n\n`,
  );
  sseClients.push(res);

  req.on("close", () => {
    sseClients = sseClients.filter((c) => c !== res);
  });
});

// ──────────────────────────────────────────────
// API Routes
// ──────────────────────────────────────────────

// List all available endpoints
app.get("/api/v1/endpoints", (req, res) => {
  const list = ENDPOINTS.map((key) => ({
    name: key,
    url: `/api/v1/sensors/${key}`,
    method: "GET",
    description: `Returns simulated ${key.replace(/-/g, " ")} IoT sensor data`,
  }));
  res.json({ status: "ok", endpoints: list });
});

// Individual sensor endpoints
ENDPOINTS.forEach((key) => {
  app.get(`/api/v1/sensors/${key}`, logAccess, (req, res) => {
    // Simulate occasional slow response
    const delay =
      Math.random() > 0.9
        ? randomBetween(200, 800, 0)
        : randomBetween(5, 50, 0);

    setTimeout(() => {
      // Simulate occasional errors
      if (Math.random() > 0.95) {
        return res.status(500).json({
          status: "error",
          error: "Sensor temporarily unavailable",
          timestamp: new Date().toISOString(),
        });
      }

      const reading = sensorGenerators[key]();
      res.json({
        status: "ok",
        timestamp: new Date().toISOString(),
        data: reading,
      });
    }, delay);
  });
});

// Batch: get all sensors at once
app.get("/api/v1/sensors", logAccess, (req, res) => {
  const all = {};
  ENDPOINTS.forEach((key) => {
    all[key] = sensorGenerators[key]();
  });
  res.json({
    status: "ok",
    timestamp: new Date().toISOString(),
    data: all,
  });
});

// Access log endpoint (for dashboard)
app.get("/api/v1/access-log", (req, res) => {
  const limit = Math.min(parseInt(req.query.limit) || 50, MAX_LOG);
  res.json({
    status: "ok",
    total: requestCounter,
    entries: accessLog.slice(0, limit),
  });
});

// Stats endpoint
app.get("/api/v1/stats", (req, res) => {
  const perEndpoint = {};
  accessLog.forEach((entry) => {
    const ep = entry.endpoint;
    if (!perEndpoint[ep]) {
      perEndpoint[ep] = { count: 0, totalTime: 0, errors: 0 };
    }
    perEndpoint[ep].count++;
    perEndpoint[ep].totalTime += entry.responseTime;
    if (entry.statusCode >= 400) perEndpoint[ep].errors++;
  });

  Object.keys(perEndpoint).forEach((ep) => {
    perEndpoint[ep].avgResponseTime = Math.round(
      perEndpoint[ep].totalTime / perEndpoint[ep].count,
    );
  });

  res.json({
    status: "ok",
    totalRequests: requestCounter,
    activeConnections: sseClients.length,
    endpointStats: perEndpoint,
  });
});

// ──────────────────────────────────────────────
// WebSocket: Real-time sensor streaming
// ──────────────────────────────────────────────
// Protocol (client → server):
//   { "action": "subscribe",   "sensors": ["amr","temperature"], "interval": 1000 }
//   { "action": "unsubscribe", "sensors": ["temperature"] }
//   { "action": "list" }
//   { "action": "ping" }
//
// Protocol (server → client):
//   { "type": "welcome",      "availableSensors": [...], "message": "..." }
//   { "type": "subscribed",   "sensors": [...], "interval": 1000 }
//   { "type": "unsubscribed", "sensors": [...] }
//   { "type": "data",         "sensor": "amr", "data": {...}, "timestamp": "..." }
//   { "type": "sensors_list", "sensors": [...] }
//   { "type": "pong" }
//   { "type": "error",        "message": "..." }

const httpServer = createServer(app);

const wss = new WebSocketServer({ server: httpServer, path: "/ws/sensors" });

wss.on("connection", (ws, req) => {
  const clientIp =
    req.headers["x-forwarded-for"] || req.socket.remoteAddress || "unknown";

  // Per-client state
  const clientState = {
    subscriptions: new Set(), // sensor names
    interval: 1000, // ms between pushes
    timer: null,
  };

  const send = (payload) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(payload));
    }
  };

  const stopTimer = () => {
    if (clientState.timer) {
      clearInterval(clientState.timer);
      clientState.timer = null;
    }
  };

  const startTimer = () => {
    stopTimer();
    if (clientState.subscriptions.size === 0) return;
    clientState.timer = setInterval(() => {
      for (const name of clientState.subscriptions) {
        const gen = sensorGenerators[name];
        if (!gen) continue;
        send({
          type: "data",
          sensor: name,
          data: gen(),
          timestamp: new Date().toISOString(),
        });
      }
    }, clientState.interval);
  };

  // Welcome message
  send({
    type: "welcome",
    availableSensors: ENDPOINTS,
    message:
      'Connected to Simmurator WebSocket. Send {"action":"subscribe","sensors":[...],"interval":1000} to start streaming.',
  });

  ws.on("message", (raw) => {
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch {
      return send({ type: "error", message: "Invalid JSON" });
    }

    switch (msg.action) {
      case "subscribe": {
        const requested = Array.isArray(msg.sensors) ? msg.sensors : ENDPOINTS;
        const valid = requested.filter((s) => sensorGenerators[s]);
        const invalid = requested.filter((s) => !sensorGenerators[s]);

        valid.forEach((s) => clientState.subscriptions.add(s));

        if (msg.interval && Number.isInteger(msg.interval)) {
          clientState.interval = Math.max(100, Math.min(msg.interval, 60000));
        }

        send({
          type: "subscribed",
          sensors: [...clientState.subscriptions],
          interval: clientState.interval,
          ...(invalid.length && { unknown: invalid }),
        });
        startTimer();
        break;
      }

      case "unsubscribe": {
        const targets = Array.isArray(msg.sensors)
          ? msg.sensors
          : [...clientState.subscriptions];
        targets.forEach((s) => clientState.subscriptions.delete(s));

        send({
          type: "unsubscribed",
          sensors: targets,
          remaining: [...clientState.subscriptions],
        });
        startTimer(); // restart with fewer sensors (or stop if empty)
        break;
      }

      case "list":
        send({ type: "sensors_list", sensors: ENDPOINTS });
        break;

      case "ping":
        send({ type: "pong", timestamp: new Date().toISOString() });
        break;

      default:
        send({ type: "error", message: `Unknown action: "${msg.action}"` });
    }
  });

  ws.on("close", () => {
    stopTimer();
  });

  ws.on("error", () => {
    stopTimer();
  });
});

// ──────────────────────────────────────────────
// Start
// ──────────────────────────────────────────────
httpServer.listen(PORT, () => {
  console.log(
    `\n  🚀 Simmurator API Server running at http://localhost:${PORT}`,
  );
  console.log(`  📡 SSE stream at http://localhost:${PORT}/events`);
  console.log(`  🔌 WebSocket stream at ws://localhost:${PORT}/ws/sensors`);
  console.log(
    `  📋 Endpoints list: http://localhost:${PORT}/api/v1/endpoints\n`,
  );
});
