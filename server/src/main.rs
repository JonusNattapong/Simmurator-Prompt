use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Json, Router,
};
use chrono::Utc;
use futures_util::stream::StreamExt;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::{Any, CorsLayer};

// ──────────────────────────────────────────────
// Models
// ──────────────────────────────────────────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct AccessLogEntry {
    id: usize,
    timestamp: String,
    ip: String,
    user_agent: String,
    endpoint: String,
    method: String,
    status_code: u16,
    response_time: u128,
    device_id: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "camelCase")]
enum SSEEvent {
    Connected { message: String },
    Access(AccessLogEntry),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "action")]
#[serde(rename_all = "camelCase")]
enum WSAction {
    Subscribe {
        sensors: Option<Vec<String>>,
        interval: Option<u64>,
    },
    Unsubscribe {
        sensors: Option<Vec<String>>,
    },
    List,
    Ping,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum WSMessage {
    Welcome {
        available_sensors: Vec<String>,
        message: String,
    },
    Subscribed {
        sensors: Vec<String>,
        interval: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        unknown: Option<Vec<String>>,
    },
    Unsubscribed {
        sensors: Vec<String>,
        remaining: Vec<String>,
    },
    Data {
        sensor: String,
        data: serde_json::Value,
        timestamp: String,
    },
    SensorsList {
        sensors: Vec<String>,
    },
    Pong {
        timestamp: String,
    },
    #[allow(dead_code)]
    Error {
        message: String,
    },
}

// ──────────────────────────────────────────────
// Sensor Simulators
// ──────────────────────────────────────────────

fn random_between(min: f64, max: f64) -> f64 {
    let mut rng = rand::thread_rng();
    rng.gen_range(min..max)
}

fn generate_sensor_data(key: &str) -> Option<serde_json::Value> {
    let mut rng = rand::thread_rng();
    match key {
        "temperature" => Some(serde_json::json!({
            "sensorId": "TEMP-001",
            "type": "temperature",
            "value": format!("{:.1}", random_between(22.0, 38.5)).parse::<f64>().unwrap(),
            "unit": "°C",
            "location": "Factory Floor A",
            "status": if rng.gen_bool(0.95) { "normal" } else { "warning" }
        })),
        "humidity" => Some(serde_json::json!({
            "sensorId": "HUM-002",
            "type": "humidity",
            "value": format!("{:.1}", random_between(30.0, 85.0)).parse::<f64>().unwrap(),
            "unit": "%RH",
            "location": "Server Room B",
            "status": if rng.gen_bool(0.9) { "normal" } else { "warning" }
        })),
        "oil-level" => Some(serde_json::json!({
            "sensorId": "OIL-003",
            "type": "oil_level",
            "value": format!("{:.1}", random_between(15.0, 98.0)).parse::<f64>().unwrap(),
            "unit": "%",
            "tankCapacity": "5000L",
            "currentVolume": format!("{}L", rng.gen_range(750..4900)),
            "location": "Storage Tank C",
            "status": if rng.gen_bool(0.92) { "normal" } else { "critical" }
        })),
        "oil-pressure" => Some(serde_json::json!({
            "sensorId": "OPR-004",
            "type": "oil_pressure",
            "value": format!("{:.2}", random_between(1.5, 6.8)).parse::<f64>().unwrap(),
            "unit": "bar",
            "pipelineId": "PIPE-MAIN-01",
            "flowRate": format!("{:.1} L/min", random_between(50.0, 320.0)),
            "location": "Pipeline Section D",
            "status": if rng.gen_bool(0.94) { "normal" } else { "warning" }
        })),
        "air-quality" => Some(serde_json::json!({
            "sensorId": "AQI-005",
            "type": "air_quality",
            "pm25": rng.gen_range(5..120),
            "pm10": rng.gen_range(10..200),
            "co2": rng.gen_range(350..1200),
            "unit": "µg/m³",
            "aqi": rng.gen_range(10..180),
            "location": "Outdoor Station E",
            "status": if rng.gen_bool(0.85) { "good" } else { "moderate" }
        })),
        "pressure" => Some(serde_json::json!({
            "sensorId": "PRS-006",
            "type": "pressure",
            "value": format!("{:.1}", random_between(980.0, 1050.0)).parse::<f64>().unwrap(),
            "unit": "hPa",
            "location": "Weather Station F",
            "status": "normal"
        })),
        "vibration" => Some(serde_json::json!({
            "sensorId": "VIB-007",
            "type": "vibration",
            "amplitude": format!("{:.3}", random_between(0.01, 5.5)).parse::<f64>().unwrap(),
            "frequency": format!("{:.1}", random_between(10.0, 500.0)).parse::<f64>().unwrap(),
            "unit": "mm/s",
            "machineId": "CNC-MILL-02",
            "location": "Machine Shop G",
            "status": if rng.gen_bool(0.9) { "normal" } else { "alert" }
        })),
        "energy-meter" => Some(serde_json::json!({
            "sensorId": "ENR-008",
            "type": "energy",
            "power": format!("{:.2}", random_between(1.2, 45.0)).parse::<f64>().unwrap(),
            "voltage": format!("{:.1}", random_between(218.0, 242.0)).parse::<f64>().unwrap(),
            "current": format!("{:.2}", random_between(0.5, 18.0)).parse::<f64>().unwrap(),
            "powerFactor": format!("{:.2}", random_between(0.85, 0.99)).parse::<f64>().unwrap(),
            "unit": "kW",
            "location": "Main Panel H",
            "status": "normal"
        })),
        "amr" => Some(serde_json::json!({
            "sensorId": "AMR-009",
            "type": "amr_oil_pipeline",
            "meterSerial": "AMR-PIPE-2024-09",
            "pipelineId": "PIPE-AMR-01",
            "location": "Oil Pipeline Station I",
            "flowRate": format!("{:.2}", random_between(80.0, 350.0)).parse::<f64>().unwrap(),
            "flowRateUnit": "L/min",
            "flowDirection": if rng.gen_bool(0.95) { "forward" } else { "reverse" },
            "cumulativeFlow": format!("{:.1}", random_between(100000.0, 999999.0)).parse::<f64>().unwrap(),
            "cumulativeFlowUnit": "m³",
            "status": if rng.gen_bool(0.94) { "normal" } else { "warning" }
        })),
        _ => None,
    }
}

const AVAILABLE_SENSORS: &[&str] = &[
    "temperature", "humidity", "oil-level", "oil-pressure",
    "air-quality", "pressure", "vibration", "energy-meter", "amr"
];

// ──────────────────────────────────────────────
// State
// ──────────────────────────────────────────────

struct AppState {
    access_log: Mutex<Vec<AccessLogEntry>>,
    request_counter: Mutex<usize>,
    sse_tx: broadcast::Sender<SSEEvent>,
}

type SharedState = Arc<AppState>;

// ──────────────────────────────────────────────
// Handlers
// ──────────────────────────────────────────────

async fn get_endpoints() -> Response {
    let endpoints: Vec<_> = AVAILABLE_SENSORS
        .iter()
        .map(|&key| serde_json::json!({
            "name": key,
            "url": format!("/api/v1/sensors/{}", key),
            "method": "GET",
            "description": format!("Returns simulated {} IoT sensor data", key.replace('-', " "))
        }))
        .collect();

    Json(serde_json::json!({
        "status": "ok",
        "endpoints": endpoints
    })).into_response()
}

#[axum::debug_handler]
async fn get_sensor_data(
    Path(key): Path<String>,
) -> Response {
    // Simulation logic (slow response & error simulation)
    let (delay, is_error) = {
        let mut rng = rand::thread_rng();
        let delay = if rng.gen_bool(0.1) { rng.gen_range(200..800) } else { rng.gen_range(5..50) };
        let is_error = rng.gen_bool(0.05);
        (delay, is_error)
    };
    tokio::time::sleep(Duration::from_millis(delay)).await;

    if is_error {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "error": "Sensor temporarily unavailable",
                "timestamp": Utc::now().to_rfc3339()
            })),
        ).into_response();
    }

    if let Some(data) = generate_sensor_data(&key) {
        Json(serde_json::json!({
            "status": "ok",
            "timestamp": Utc::now().to_rfc3339(),
            "data": data
        })).into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "error": "Sensor not found"
            })),
        ).into_response()
    }
}

async fn get_all_sensors() -> Response {
    let mut all = HashMap::new();
    for &key in AVAILABLE_SENSORS {
        if let Some(data) = generate_sensor_data(key) {
            all.insert(key, data);
        }
    }

    Json(serde_json::json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339(),
        "data": all
    })).into_response()
}

async fn get_access_log(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<SharedState>,
) -> Response {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<usize>().ok())
        .unwrap_or(50);

    let logs = state.access_log.lock().unwrap();
    let entries: Vec<_> = logs.iter().take(limit).cloned().collect();
    let total = *state.request_counter.lock().unwrap();

    Json(serde_json::json!({
        "status": "ok",
        "total": total,
        "entries": entries
    })).into_response()
}

async fn get_stats(State(state): State<SharedState>) -> Response {
    let logs = state.access_log.lock().unwrap();
    let total_requests = *state.request_counter.lock().unwrap();
    
    let mut per_endpoint: HashMap<String, serde_json::Value> = HashMap::new();
    
    for entry in logs.iter() {
        let ep = entry.endpoint.clone();
        let stats = per_endpoint.entry(ep).or_insert(serde_json::json!({
            "count": 0,
            "totalTime": 0,
            "errors": 0
        }));
        
        let count = stats["count"].as_u64().unwrap_or(0) + 1;
        let total_time = stats["totalTime"].as_u64().unwrap_or(0) + entry.response_time as u64;
        let mut errors = stats["errors"].as_u64().unwrap_or(0);
        if entry.status_code >= 400 {
            errors += 1;
        }
        
        *stats = serde_json::json!({
            "count": count,
            "totalTime": total_time,
            "errors": errors,
            "avgResponseTime": if count > 0 { total_time / count } else { 0 }
        });
    }

    Json(serde_json::json!({
        "status": "ok",
        "totalRequests": total_requests,
        "activeConnections": state.sse_tx.receiver_count(),
        "endpointStats": per_endpoint
    })).into_response()
}

async fn sse_handler(State(state): State<SharedState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse_tx.subscribe();
    
    // Initial welcome message
    let initial_stream = tokio_stream::once(Ok(Event::default().data(serde_json::to_string(&SSEEvent::Connected {
        message: "SSE stream connected".to_string(),
    }).unwrap())));

    let broadcast_stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(event) => Some(Ok(Event::default().data(serde_json::to_string(&event).unwrap()))),
            _ => None,
        }
    });

    Sse::new(initial_stream.chain(broadcast_stream))
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, _state: SharedState) {
    let mut subscriptions = HashSet::new();
    let mut interval_ms = 1000;
    
    // Welcome message
    let welcome = WSMessage::Welcome {
        available_sensors: AVAILABLE_SENSORS.iter().map(|&s| s.to_string()).collect(),
        message: "Connected to Simmurator WebSocket. Send subscribe action to start.".to_string(),
    };
    let _ = socket.send(Message::Text(serde_json::to_string(&welcome).unwrap())).await;

    let mut send_interval = tokio::time::interval(Duration::from_millis(interval_ms));

    loop {
        tokio::select! {
            // Check for client messages
            msg = socket.next() => {
                let msg = match msg {
                    Some(Ok(msg)) => msg,
                    _ => break, // client disconnected
                };

                if let Message::Text(text) = msg {
                    if let Ok(action) = serde_json::from_str::<WSAction>(&text) {
                        match action {
                            WSAction::Subscribe { sensors, interval } => {
                                let requested = sensors.unwrap_or_else(|| AVAILABLE_SENSORS.iter().map(|&s| s.to_string()).collect());
                                let mut valid = Vec::new();
                                let mut unknown = Vec::new();
                                
                                for s in requested {
                                    if AVAILABLE_SENSORS.contains(&s.as_str()) {
                                        subscriptions.insert(s.clone());
                                        valid.push(s);
                                    } else {
                                        unknown.push(s);
                                    }
                                }
                                
                                if let Some(i) = interval {
                                    interval_ms = i.clamp(100, 60000);
                                    send_interval = tokio::time::interval(Duration::from_millis(interval_ms));
                                }

                                let resp = WSMessage::Subscribed {
                                    sensors: subscriptions.iter().cloned().collect(),
                                    interval: interval_ms,
                                    unknown: if unknown.is_empty() { None } else { Some(unknown) },
                                };
                                let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                            }
                            WSAction::Unsubscribe { sensors } => {
                                let targets = sensors.unwrap_or_else(|| subscriptions.iter().cloned().collect());
                                for s in &targets {
                                    subscriptions.remove(s);
                                }
                                let resp = WSMessage::Unsubscribed {
                                    sensors: targets,
                                    remaining: subscriptions.iter().cloned().collect(),
                                };
                                let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                            }
                            WSAction::List => {
                                let resp = WSMessage::SensorsList {
                                    sensors: AVAILABLE_SENSORS.iter().map(|&s| s.to_string()).collect(),
                                };
                                let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                            }
                            WSAction::Ping => {
                                let resp = WSMessage::Pong { timestamp: Utc::now().to_rfc3339() };
                                let _ = socket.send(Message::Text(serde_json::to_string(&resp).unwrap())).await;
                            }
                        }
                    }
                }
            }
            // Send periodic sensor data
            _ = send_interval.tick() => {
                if !subscriptions.is_empty() {
                    for sensor in &subscriptions {
                        if let Some(data) = generate_sensor_data(sensor) {
                            let msg = WSMessage::Data {
                                sensor: sensor.clone(),
                                data,
                                timestamp: Utc::now().to_rfc3339(),
                            };
                            if let Err(_) = socket.send(Message::Text(serde_json::to_string(&msg).unwrap())).await {
                                return; // connection closed
                            }
                        }
                    }
                }
            }
        }
    }
}

// ──────────────────────────────────────────────
// Middleware: Log access
// ──────────────────────────────────────────────

async fn log_middleware(
    State(state): State<SharedState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let method = req.method().to_string();
    let endpoint = req.uri().to_string();
    let ip = req.headers().get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let user_agent = req.headers().get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let device_id = req.headers().get("x-device-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let response = next.run(req).await;
    
    let status_code = response.status().as_u16();
    let response_time = start.elapsed().as_millis();

    // Skip logging for WS and SSE if they generate too much noise, but for this app we log everything
    let mut counter = state.request_counter.lock().unwrap();
    *counter += 1;
    let id = *counter;

    let entry = AccessLogEntry {
        id,
        timestamp: Utc::now().to_rfc3339(),
        ip,
        user_agent,
        endpoint,
        method,
        status_code,
        response_time,
        device_id,
    };

    {
        let mut logs = state.access_log.lock().unwrap();
        logs.insert(0, entry.clone());
        if logs.len() > 500 {
            logs.truncate(500);
        }
    }

    let _ = state.sse_tx.send(SSEEvent::Access(entry));

    response
}

// ──────────────────────────────────────────────
// Main
// ──────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Shared state
    let (sse_tx, _) = broadcast::channel(100);
    let state = Arc::new(AppState {
        access_log: Mutex::new(Vec::with_capacity(500)),
        request_counter: Mutex::new(0),
        sse_tx,
    });

    // CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/events", get(sse_handler))
        .route("/ws/sensors", get(ws_handler))
        .route("/api/v1/endpoints", get(get_endpoints))
        .route("/api/v1/sensors", get(get_all_sensors))
        .route("/api/v1/sensors/:key", get(get_sensor_data))
        .route("/api/v1/access-log", get(get_access_log))
        .route("/api/v1/stats", get(get_stats))
        .layer(axum::middleware::from_fn_with_state(state.clone(), log_middleware))
        .fallback_service(tower_http::services::ServeDir::new("dist").fallback(tower_http::services::ServeFile::new("dist/index.html")))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("\n  🚀 Simmurator Rust Server running at http://localhost:8080");
    println!("  📡 SSE stream at http://localhost:8080/events");
    println!("  🔌 WebSocket stream at ws://localhost:8080/ws/sensors");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
