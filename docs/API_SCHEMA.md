# Simmurator IoT API Schema Documentation

เอกสารนี้ระบุโครงสร้าง JSON (Schema) ของ API ในทุก Endpoints เพื่อใช้สำหรับการพัฒนา Report หรือ Dashboard ต่อเนื่อง

## 1. Common Response Wrapper

ทุก API จะถูกหุ้มด้วย Wrapper นี้:

```json
{
  "status": "ok",      // หรือ "error"
  "timestamp": "ISO-8601",
  "data": { ... }      // ข้อมูลของแต่ละ sensor
}
```

---

## 2. Sensor Data Models

### 🛢️ Oil Level (`/api/v1/sensors/oil-level`)

```json
{
  "sensorId": "OIL-003",
  "type": "oil_level",
  "value": 45.5,        // เปอร์เซ็นต์คงเหลือ (%)
  "unit": "%",
  "tankCapacity": "5000L",
  "currentVolume": "2275L",
  "location": "Storage Tank C",
  "status": "normal"    // normal, warning, critical
}
```

### ⛽ Oil Pressure (`/api/v1/sensors/oil-pressure`)

```json
{
  "sensorId": "OPR-004",
  "type": "oil_pressure",
  "value": 4.25,        // แรงดัน (bar)
  "unit": "bar",
  "pipelineId": "PIPE-MAIN-01",
  "flowRate": "120.5 L/min",
  "location": "Pipeline Section D",
  "status": "normal"
}
```

### 🌡️ Temperature (`/api/v1/sensors/temperature`)

```json
{
  "sensorId": "TEMP-001",
  "type": "temperature",
  "value": 32.5,
  "unit": "°C",
  "location": "Factory Floor A",
  "status": "normal"
}
```

### 🌬️ Air Quality (`/api/v1/sensors/air-quality`)

```json
{
  "sensorId": "AQI-005",
  "type": "air_quality",
  "pm25": 15,
  "pm10": 22,
  "co2": 450,
  "aqi": 35,
  "unit": "µg/m³",
  "location": "Outdoor Station E",
  "status": "good"
}
```

*(ดูเพิ่มเติมใน `server/index.js` สำหรับ humidity, pressure, vibration, energy-meter)*

---

## 3. Monitoring & Stats Schema

### Access Log Entry

```json
{
  "id": 1,
  "timestamp": "ISO-8601",
  "ip": "127.0.0.1",
  "userAgent": "curl/7.81.0",
  "endpoint": "/api/v1/sensors/oil-level",
  "method": "GET",
  "statusCode": 200,
  "responseTime": 15,    // ms
  "deviceId": null       // (Optional) จาก Header x-device-id
}
```

### Stats Summary

```json
{
  "status": "ok",
  "totalRequests": 1500,
  "activeConnections": 1,
  "endpointStats": {
    "/api/v1/sensors/oil-level": {
      "count": 450,
      "avgResponseTime": 12,
      "errors": 2
    }
  }
}
```

---

## 4. Suggested Database Schema (SQL)

หากต้องการเก็บข้อมูลลง Database เพื่อทำประวัติย้อนหลัง แนะนำโครงสร้างดังนี้:

### Table: `sensor_readings`

| Column | Type | Description |
|---|---|---|
| `id` | BIGINT (PK) | Auto increment |
| `sensor_id` | VARCHAR(50) | เช่น TEMP-001 |
| `sensor_type` | VARCHAR(50) | เช่น oil_level |
| `value` | DECIMAL | ค่าที่อ่านได้ |
| `unit` | VARCHAR(10) | หน่วย |
| `location` | VARCHAR(100) | |
| `status` | VARCHAR(20) | |
| `payload` | JSON / TEXT | เก็บ JSON เต็มๆ ไว้ดูย้อนหลัง |
| `created_at` | TIMESTAMP | |

### Table: `api_access_logs`

| Column | Type | Description |
|---|---|---|
| `id` | BIGINT (PK) | |
| `endpoint` | VARCHAR(255) | |
| `client_ip` | VARCHAR(45) | |
| `user_agent` | TEXT | |
| `status_code` | INT | |
| `response_ms` | INT | |
| `requested_at` | TIMESTAMP | |
