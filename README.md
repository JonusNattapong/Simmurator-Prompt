# 🚀 Simmurator: IoT API Monitoring Dashboard

**Simmurator** is a high-performance, real-time IoT API simulator and monitoring dashboard. Designed for developers and data consumers, it provides a stunning high-tech interface to explore, test, and stream simulated sensor data from a virtual factory environment.

![Orange-White Theme](https://img.shields.io/badge/Theme-Orange--White-orange?style=for-the-badge)
![React](https://img.shields.io/badge/React-19-blue?style=for-the-badge&logo=react)
![Vite](https://img.shields.io/badge/Vite-6-purple?style=for-the-badge&logo=vite)
![Rust](https://img.shields.io/badge/Rust-1.75+-brown?style=for-the-badge&logo=rust)
![Axum](https://img.shields.io/badge/Axum-0.7-blue?style=for-the-badge)

---

## ✨ Key Features

### 🚥 Real-time Traffic Monitoring (SSE)

* **Live Access Logs:** Watch system traffic as it happens with Server-Sent Events (SSE).
* **Detailed Insights:** Track IP addresses, User Agents, Response Times, and Status Codes for every request.

### 🔌 Live Sensor Streaming (WebSockets)

* **Bi-directional Stream:** Connect via WebSocket to receive high-frequency sensor updates.
* **Subscription Model:** Selectively subscribe to specific sensors (e.g., AMR Oil Pipeline, Temperature, Energy Meters).
* **Adjustable Interval:** Control streaming speed from 100ms up to 5 seconds per update.

### 📚 Swagger-Inspired Documentation UI

* **Interactive Explorer:** Explore API endpoints with a familiar, professional documentation interface.
* **Try It Out:** Execute API calls directly from the dashboard and see live response bodies and performance metrics.
* **Schema Viewer:** View expected JSON structures for all IoT sensor models.

### 📊 Advanced Analytics

* **Stats Engine:** Real-time calculation of total requests, average response times, and error rates.
* **Per-Endpoint Insight:** Horizontal metrics bars showing usage distribution across your API suite.

---

## 🎨 Design Aesthetic: "Orange-White Premium"

The dashboard features a **High-Tech Next-Gen UI** designed to impress:

* **Glassmorphism:** Elegant blur effects and semi-transparent panels.
* **Ambient Animations:** Floating orbs and pulse-glow indicators for system health.
* **Neo-Brutalist Depth:** Sharp shadows and high-contrast typography using the *Outfit* and *JetBrains Mono* fonts.

---

## 🛠 Tech Stack

* **Frontend:** React 19, Vite, Vanilla CSS (Custom Design System)
* **Backend:** Rust (Axum, Tokio)
* **Communication:**
  * **REST API:** Standard HTTP endpoints for sensor data retrieval.
  * **SSE:** For real-time event broadcasting.
  * **WebSocket (ws):** For high-frequency, stateful data streaming.
* **Simulation Engine:** Custom-built sensor generators with randomized telemetry logic.

---

## 🚀 Getting Started

### 1. Prerequisites

* [Node.js](https://nodejs.org/) (Project developed with v23+)
* npm

### 2. Installation

Clone the repository and install dependencies for both the environment and the server:

```bash
npm install
```

### 3. Run Locally

Simmurator uses `concurrently` to start both the Express backend and the Vite frontend simultaneously:

```bash
npm run dev
```

* **Dashboard Display:** <http://localhost:5173>
* **API Base:** <http://localhost:8080/api/v1>
* **SSE Stream:** <http://localhost:8080/events>
* **WebSocket:** ws://localhost:8080/ws/sensors

---

## 📡 Simulated Sensors

The system simulates a variety of industrial IoT sensors:

* **Temperature & Humidity:** Environmental monitoring.
* **Oil Level & Pressure:** Pipeline and storage tank diagnostics.
* **Energy Meter:** Power consumption, voltage, and power factor.
* **Vibration:** Machine health and CNC monitoring.
* **AMR Pipeline:** Advanced mechanical readings for oil pipelines.

---

## 📝 License

This project is open-source and intended for demonstration and testing purposes.

---
*Created with ❤️ by the Simmurator Team*
