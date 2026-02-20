use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, Query, State,
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

// Helper function: คำนวณ dew point จาก humidity และ temperature (Magnus formula)
fn temp_to_dewpoint(rh: f64, temp: f64) -> f64 {
    let a = 17.625;
    let b = 243.04;
    let alpha = (a * temp / (b + temp)).ln() + (rh / 100.0).ln();
    (b * alpha) / (a - alpha)
}

// Helper function: คำนวณ AQI จาก PM2.5 (simplified)
fn calculate_aqi_pm25(pm25: f64) -> i32 {
    if pm25 <= 12.0 { ((pm25 / 12.0) * 50.0) as i32 }
    else if pm25 <= 35.4 { 50 + ((pm25 - 12.0) / 23.4 * 49.0) as i32 }
    else if pm25 <= 55.4 { 100 + ((pm25 - 35.4) / 20.0 * 49.0) as i32 }
    else if pm25 <= 150.4 { 150 + ((pm25 - 55.4) / 95.0 * 49.0) as i32 }
    else if pm25 <= 250.4 { 200 + ((pm25 - 150.4) / 100.0 * 99.0) as i32 }
    else { 300 + ((pm25 - 250.4) / 149.6 * 99.0) as i32 }
}

// ============================================
// ISA-95 Equipment Hierarchy + OPC UA Standards
// ============================================

/// ISA-95 Equipment Hierarchy Level
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Isa95Equipment {
    site: String,
    area: String,
    line: String,
    unit: String,
    equipment: String,
}

/// OPC UA Node Information
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct OpcUaNode {
    node_id: String,
    browse_name: String,
    display_name: String,
    namespace_index: u16,
}

/// MQTT Sparkplug B Topic Structure
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct SparkplugTopic {
    version: String,
    group_id: String,
    message_type: String,
    edge_node_id: String,
    device_id: String,
}

/// UCUM Unit Codes (Unified Code for Units of Measure)
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct UcumUnit {
    code: String,
    display: String,
}

/// Data Quality Status (OPC UA Standard)
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
enum DataQuality {
    Good,
    GoodUncertain,
    Uncertain,
    Bad,
}

/// OPC UA Status Codes
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
enum OpcUaStatusCode {
    Good = 0x00000000,
    GoodUncertain = 0x00000001,
    UncertainInitialValue = 0x00200000,
    BadSensorFailure = 0x80040000,
    BadCommunicationError = 0x80050000,
    BadOutOfService = 0x80080000,
}

/// Unified Sensor Data Structure (ISA-95 + OPC UA + Sparkplug B)
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct UnifiedSensorData {
    // OPC UA Information Model
    opc_ua: OpcUaNode,
    
    // ISA-95 Equipment Hierarchy
    equipment_hierarchy: Isa95Equipment,
    
    // MQTT Sparkplug B Topic
    sparkplug_topic: SparkplugTopic,
    
    // Timestamps
    source_timestamp: String,
    server_timestamp: String,
    
    // Value and Quality
    value: serde_json::Value,
    data_quality: DataQuality,
    opc_ua_status_code: OpcUaStatusCode,
    
    // UCUM Unit
    unit: UcumUnit,
    
    // Sensor Type and Description
    sensor_type: String,
    description: String,
    
    // Additional Properties (sensor-specific)
    properties: serde_json::Value,
}

/// Generate ISA-95 Equipment Hierarchy
fn generate_isa95_hierarchy(equipment_name: &str, line: &str, area: &str) -> Isa95Equipment {
    Isa95Equipment {
        site: "Thailand-Plant-01".to_string(),
        area: area.to_string(),
        line: line.to_string(),
        unit: format!("{}-Unit", line),
        equipment: equipment_name.to_string(),
    }
}

/// Generate OPC UA Node Information
fn generate_opcua_node(sensor_id: &str, display_name: &str) -> OpcUaNode {
    OpcUaNode {
        node_id: format!("ns=2;s={}", sensor_id),
        browse_name: format!("2:{}", sensor_id),
        display_name: display_name.to_string(),
        namespace_index: 2,
    }
}

/// Generate MQTT Sparkplug B Topic
fn generate_sparkplug_topic(group_id: &str, device_id: &str) -> SparkplugTopic {
    SparkplugTopic {
        version: "spBv1.0".to_string(),
        group_id: group_id.to_string(),
        message_type: "DDATA".to_string(),
        edge_node_id: "Edge-Node-01".to_string(),
        device_id: device_id.to_string(),
    }
}

/// UCUM Unit Code Mapping
fn get_ucum_unit(unit: &str) -> UcumUnit {
    match unit {
        "°C" => UcumUnit { code: "Cel".to_string(), display: "°C".to_string() },
        "°F" => UcumUnit { code: "[degF]".to_string(), display: "°F".to_string() },
        "%RH" => UcumUnit { code: "%".to_string(), display: "%RH".to_string() },
        "bar" => UcumUnit { code: "bar".to_string(), display: "bar".to_string() },
        "hPa" => UcumUnit { code: "hPa".to_string(), display: "hPa".to_string() },
        "Pa" => UcumUnit { code: "Pa".to_string(), display: "Pa".to_string() },
        "mm/s" => UcumUnit { code: "mm/s".to_string(), display: "mm/s".to_string() },
        "Hz" => UcumUnit { code: "Hz".to_string(), display: "Hz".to_string() },
        "kW" => UcumUnit { code: "kW".to_string(), display: "kW".to_string() },
        "kVA" => UcumUnit { code: "kVA".to_string(), display: "kVA".to_string() },
        "kVAR" => UcumUnit { code: "kVAR".to_string(), display: "kVAR".to_string() },
        "V" => UcumUnit { code: "V".to_string(), display: "V".to_string() },
        "A" => UcumUnit { code: "A".to_string(), display: "A".to_string() },
        "m³/h" => UcumUnit { code: "m3/h".to_string(), display: "m³/h".to_string() },
        "L/min" => UcumUnit { code: "L/min".to_string(), display: "L/min".to_string() },
        "m³" => UcumUnit { code: "m3".to_string(), display: "m³".to_string() },
        "kg/m³" => UcumUnit { code: "kg/m3".to_string(), display: "kg/m³".to_string() },
        "cSt" => UcumUnit { code: "cSt".to_string(), display: "cSt".to_string() },
        "ppm" => UcumUnit { code: "ppm".to_string(), display: "ppm".to_string() },
        "µg/m³" => UcumUnit { code: "ug/m3".to_string(), display: "µg/m³".to_string() },
        "pH" => UcumUnit { code: "pH".to_string(), display: "pH".to_string() },
        "mV" => UcumUnit { code: "mV".to_string(), display: "mV".to_string() },
        "NTU" => UcumUnit { code: "NTU".to_string(), display: "NTU".to_string() },
        "µS/cm" => UcumUnit { code: "uS/cm".to_string(), display: "µS/cm".to_string() },
        "m" => UcumUnit { code: "m".to_string(), display: "m".to_string() },
        "mm" => UcumUnit { code: "mm".to_string(), display: "mm".to_string() },
        "%" => UcumUnit { code: "%".to_string(), display: "%".to_string() },
        "RPM" => UcumUnit { code: "rpm".to_string(), display: "RPM".to_string() },
        "dBm" => UcumUnit { code: "dBm".to_string(), display: "dBm".to_string() },
        _ => UcumUnit { code: unit.to_string(), display: unit.to_string() },
    }
}

/// Generate Data Quality based on value and thresholds
fn generate_data_quality(value: f64, min: f64, max: f64) -> DataQuality {
    if value >= min && value <= max {
        DataQuality::Good
    } else if value >= min * 0.9 && value <= max * 1.1 {
        DataQuality::Uncertain
    } else {
        DataQuality::Bad
    }
}

/// Generate OPC UA Status Code
fn generate_opcua_status_code(quality: &DataQuality) -> OpcUaStatusCode {
    match quality {
        DataQuality::Good => OpcUaStatusCode::Good,
        DataQuality::GoodUncertain => OpcUaStatusCode::GoodUncertain,
        DataQuality::Uncertain => OpcUaStatusCode::UncertainInitialValue,
        DataQuality::Bad => OpcUaStatusCode::BadSensorFailure,
    }
}

// ข้อมูลสถานี pipeline และโรงกลั่นน้ำมันในประเทศไทย (อ้างอิงจากข้อมูลจริง)
// แหล่งที่มา: PTT Pipeline Network, Thaioil, SPRC, โรงกลั่นในประเทศไทย
const THAI_OIL_STATIONS: &[(&str, &str, f64, f64)] = &[
    // กรุงเทพและปริมณฑล
    ("กรุงเทพมหานคร", "Bangkok Pipeline Terminal", 13.7563, 100.5018),
    ("ปทุมธานี", "Region 9 Pipeline Operations Center", 14.0208, 100.5250),
    ("สมุทรปราการ", "Bang Pa-in Oil Pipeline Station", 13.5951, 100.6114),
    
    // ภาคตะวันออก - แหล่งอุตสาหกรรมหลัก
    ("ระยอง", "Map Ta Phut Refinery Station", 12.6517, 101.1595),
    ("ระยอง", "SPRC Map Ta Phut Terminal", 12.6833, 101.2378),
    ("ชลบุรี", "Thaioil Sriracha Refinery", 13.1742, 100.9287),
    ("ชลบุรี", "Sriracha Oil Terminal", 13.1166, 100.8666),
    ("ชลบุรี", "Si Racha Pipeline Junction", 13.1339, 100.9500),
    
    // ภาคกลาง
    ("สระบุรี", "Saraburi Pipeline Station", 14.5289, 100.9103),
    ("สระบุรี", "Sao Hai District Oil Terminal", 14.5500, 101.0500),
    ("ลพบุรี", "Lopburi Pipeline Junction", 14.7995, 100.6537),
    
    // ภาคตะวันออกเฉียงเหนือ
    ("ขอนแก่น", "Khon Kaen Distribution Terminal", 16.4419, 102.8356),
    ("ขอนแก่น", "Ban Phai Pipeline Station", 16.0667, 102.7167),
    ("นครราชสีมา", "Korat Oil Terminal", 14.9799, 102.0977),
    ("อุดรธานี", "Udon Thani Pipeline Station", 17.4138, 102.7876),
    
    // ภาคเหนือ
    ("เชียงใหม่", "Chiang Mai Distribution Center", 18.7883, 98.9853),
    ("ลำปาง", "Lampang Oil Terminal", 18.2859, 99.5128),
    ("พิษณุโลก", "Phitsanulok Pipeline Station", 16.8295, 100.2615),
    ("กำแพงเพชร", "Kamphaeng Phet Terminal", 16.4828, 99.5222),
    
    // ภาคใต้
    ("สงขลา", "Songkhla Refinery Terminal", 7.1898, 100.5954),
    ("สุราษฎร์ธานี", "Surat Thani Distribution", 9.1347, 99.3331),
    ("ภูเก็ต", "Phuket Oil Terminal", 7.8804, 98.3923),
    
    // ภาคตะวันตก
    ("สมุทรสาคร", "Mahachai Pipeline Station", 13.5475, 100.2744),
    ("กาญจนบุรี", "Kanchanaburi Terminal", 14.0228, 99.5328),
    
    // ภาคตะวันออกเฉียงเหนือตอนล่าง
    ("นครสวรรค์", "Nakhon Sawan Junction", 15.6930, 100.1225),
    ("อุบลราชธานี", "Ubon Ratchathani Station", 15.2287, 104.8564),
    ("บุรีรัมย์", "Buriram Pipeline Terminal", 14.9930, 103.1029),
];

fn get_random_oil_station() -> (&'static str, &'static str, f64, f64) {
    let mut rng = rand::thread_rng();
    THAI_OIL_STATIONS[rng.gen_range(0..THAI_OIL_STATIONS.len())]
}

fn generate_sensor_data(key: &str) -> Option<serde_json::Value> {
    let mut rng = rand::thread_rng();
    let server_ts = Utc::now().to_rfc3339();
    
    match key {
        "temperature" => {
            let temp = random_between(18.0, 32.0);
            let quality = generate_data_quality(temp, 18.0, 27.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("TEMP-001", "Temperature Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("TEMP-001", "Production-Line-1", "Factory-Floor-A"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "TEMP-001"),
                source_timestamp: source_ts,
                server_timestamp: server_ts,
                value: serde_json::json!({
                    "value": format!("{:.1}", temp).parse::<f64>().unwrap(),
                    "minThreshold": 18.0,
                    "maxThreshold": 27.0,
                    "criticalHigh": 32.0,
                    "criticalLow": 15.0
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("°C"),
                sensor_type: "temperature".to_string(),
                description: "Industrial temperature sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "humidity" => {
            let humidity = random_between(25.0, 75.0);
            let quality = generate_data_quality(humidity, 40.0, 60.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("HUM-002", "Humidity Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("HUM-002", "Server-Room-B", "IT-Infrastructure"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "HUM-002"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "value": format!("{:.1}", humidity).parse::<f64>().unwrap(),
                    "optimalMin": 40.0,
                    "optimalMax": 60.0,
                    "allowableMin": 20.0,
                    "allowableMax": 80.0,
                    "dewPoint": format!("{:.1}", temp_to_dewpoint(humidity, random_between(20.0, 30.0))).parse::<f64>().unwrap()
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("%RH"),
                sensor_type: "humidity".to_string(),
                description: "Relative humidity sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "oil-level" => {
            let capacity_liters = rng.gen_range(10000..50001);
            let level_percent = random_between(15.0, 95.0);
            let current_volume = (capacity_liters as f64 * level_percent / 100.0) as i32;
            let quality = generate_data_quality(level_percent, 20.0, 90.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("OIL-003", "Oil Level Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("OIL-003", "Storage-Tank-C", "Tank-Farm"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "OIL-003"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "value": format!("{:.1}", level_percent).parse::<f64>().unwrap(),
                    "tankCapacityLiters": capacity_liters,
                    "tankCapacityM3": format!("{:.1}", capacity_liters as f64 / 1000.0).parse::<f64>().unwrap(),
                    "currentVolumeLiters": current_volume,
                    "currentVolumeM3": format!("{:.2}", current_volume as f64 / 1000.0).parse::<f64>().unwrap(),
                    "lowAlarmThreshold": 10.0,
                    "highAlarmThreshold": 95.0
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("%"),
                sensor_type: "oil_level".to_string(),
                description: "Industrial oil level sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "oil-pressure" => {
            let pressure = random_between(15.0, 200.0);
            let flow_rate = random_between(50.0, 500.0);
            let quality = generate_data_quality(pressure, 30.0, 180.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("OPR-004", "Oil Pressure Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("OPR-004", "Pipeline-D", "Process-Area"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "OPR-004"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "value": format!("{:.2}", pressure).parse::<f64>().unwrap(),
                    "flowRateLpm": format!("{:.1}", flow_rate).parse::<f64>().unwrap(),
                    "operatingRange": "10-200 bar",
                    "maxWorkingPressure": 250.0
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("bar"),
                sensor_type: "oil_pressure".to_string(),
                description: "Hydraulic oil pressure sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "air-quality" => {
            let pm25 = random_between(5.0, 75.0);
            let pm10 = pm25 * random_between(1.5, 2.5);
            let co2 = random_between(400.0, 1500.0);
            let voc = random_between(0.1, 2.0);
            let aqi = calculate_aqi_pm25(pm25);
            let quality = if aqi <= 100 { generate_data_quality(pm25, 0.0, 35.0) } else { DataQuality::Bad };
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("AQI-005", "Air Quality Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("AQI-005", "Outdoor-Station-E", "Environment"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "AQI-005"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "pm25": format!("{:.1}", pm25).parse::<f64>().unwrap(),
                    "pm10": format!("{:.1}", pm10).parse::<f64>().unwrap(),
                    "co2": format!("{:.0}", co2).parse::<f64>().unwrap(),
                    "voc": format!("{:.2}", voc).parse::<f64>().unwrap(),
                    "aqi": aqi,
                    "whoPm25Guideline": 15.0,
                    "whoPm10Guideline": 45.0,
                    "co2Threshold": 1000.0
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("µg/m³"),
                sensor_type: "air_quality".to_string(),
                description: "Multi-parameter air quality sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "pressure" => {
            let pressure = random_between(990.0, 1030.0);
            let altitude = random_between(0.0, 100.0);
            let sea_level_pressure = pressure * (1.0 + (altitude / 44330.0)).powf(5.255);
            let trend = if rng.gen_bool(0.5) { "rising" } else { "falling" };
            let quality = generate_data_quality(pressure, 980.0, 1050.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("PRS-006", "Atmospheric Pressure Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("PRS-006", "Weather-Station-F", "Environment"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "PRS-006"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "value": format!("{:.1}", pressure).parse::<f64>().unwrap(),
                    "seaLevelPressure": format!("{:.1}", sea_level_pressure).parse::<f64>().unwrap(),
                    "altitudeMeters": format!("{:.1}", altitude).parse::<f64>().unwrap(),
                    "standardPressure": 1013.25,
                    "trend": trend
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("hPa"),
                sensor_type: "pressure".to_string(),
                description: "Atmospheric pressure sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "vibration" => {
            let velocity_rms = random_between(0.5, 12.0);
            let frequency = random_between(10.0, 1000.0);
            let acceleration = velocity_rms * frequency * 2.0 * std::f64::consts::PI / 1000.0;
            let displacement = velocity_rms / (frequency * 2.0 * std::f64::consts::PI) * 1000.0;
            let quality = generate_data_quality(velocity_rms, 0.0, 7.1);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("VIB-007", "Vibration Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("VIB-007", "CNC-Machine-02", "Machine-Shop"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "VIB-007"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "velocityRms": format!("{:.3}", velocity_rms).parse::<f64>().unwrap(),
                    "frequency": format!("{:.1}", frequency).parse::<f64>().unwrap(),
                    "acceleration": format!("{:.3}", acceleration).parse::<f64>().unwrap(),
                    "displacement": format!("{:.4}", displacement).parse::<f64>().unwrap(),
                    "machineType": "Class II (Medium machines)",
                    "iso10816Limits": {
                        "good": 2.8,
                        "satisfactory": 7.1,
                        "unsatisfactory": 18.0
                    }
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("mm/s"),
                sensor_type: "vibration".to_string(),
                description: "ISO 10816 vibration monitoring sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "energy-meter" => {
            let voltage_l1 = random_between(218.0, 242.0);
            let voltage_l3 = voltage_l1 * 1.732;
            let current = random_between(5.0, 200.0);
            let power_factor = random_between(0.80, 0.98);
            let active_power = (voltage_l3 * current * power_factor * 1.732) / 1000.0;
            let apparent_power = (voltage_l3 * current * 1.732) / 1000.0;
            let reactive_power = (apparent_power.powi(2) - active_power.powi(2)).sqrt();
            let frequency = random_between(49.5, 50.5);
            let energy_kwh = random_between(10000.0, 500000.0);
            let quality = generate_data_quality(power_factor, 0.85, 1.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("ENR-008", "Energy Meter"),
                equipment_hierarchy: generate_isa95_hierarchy("ENR-008", "Main-Panel-H", "Electrical"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "ENR-008"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "activePower": format!("{:.2}", active_power).parse::<f64>().unwrap(),
                    "apparentPower": format!("{:.2}", apparent_power).parse::<f64>().unwrap(),
                    "reactivePower": format!("{:.2}", reactive_power).parse::<f64>().unwrap(),
                    "voltageL1": format!("{:.1}", voltage_l1).parse::<f64>().unwrap(),
                    "voltageL3": format!("{:.1}", voltage_l3).parse::<f64>().unwrap(),
                    "current": format!("{:.2}", current).parse::<f64>().unwrap(),
                    "powerFactor": format!("{:.3}", power_factor).parse::<f64>().unwrap(),
                    "frequency": format!("{:.2}", frequency).parse::<f64>().unwrap(),
                    "cumulativeEnergy": format!("{:.1}", energy_kwh).parse::<f64>().unwrap()
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("kW"),
                sensor_type: "energy".to_string(),
                description: "3-phase power quality meter".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "amr" => {
            let (province, location, lat, lng) = get_random_oil_station();
            let flow_rate_m3h = random_between(500.0, 2500.0);
            let flow_rate_lmin = flow_rate_m3h * 1000.0 / 60.0;
            let inlet_pressure = random_between(30.0, 80.0);
            let outlet_pressure = inlet_pressure - random_between(5.0, 20.0);
            let temperature = random_between(40.0, 70.0);
            let api_gravity = random_between(25.0, 35.0);
            let density = (141.5 / (api_gravity + 131.5)) * 998.0;
            let viscosity = random_between(10.0, 100.0);
            let cumulative = random_between(1000000.0, 50000000.0);
            let quality = generate_data_quality(inlet_pressure, 30.0, 80.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("AMR-009", "AMR Oil Pipeline Meter"),
                equipment_hierarchy: generate_isa95_hierarchy("AMR-009", "Pipeline-Station", "Oil-Gas"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "AMR-009"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "meterSerial": "AMR-PIPE-2024-09",
                    "pipelineId": "PIPE-AMR-01",
                    "location": location,
                    "province": province,
                    "coordinates": { "lat": lat, "lng": lng },
                    "flowRate": format!("{:.2}", flow_rate_lmin).parse::<f64>().unwrap(),
                    "flowRateM3H": format!("{:.2}", flow_rate_m3h).parse::<f64>().unwrap(),
                    "flowDirection": if rng.gen_bool(0.95) { "forward" } else { "reverse" },
                    "cumulativeFlow": format!("{:.1}", cumulative).parse::<f64>().unwrap(),
                    "inletPressure": format!("{:.2}", inlet_pressure).parse::<f64>().unwrap(),
                    "outletPressure": format!("{:.2}", outlet_pressure).parse::<f64>().unwrap(),
                    "differentialPressure": format!("{:.2}", inlet_pressure - outlet_pressure).parse::<f64>().unwrap(),
                    "temperature": format!("{:.1}", temperature).parse::<f64>().unwrap(),
                    "apiGravity": format!("{:.1}", api_gravity).parse::<f64>().unwrap(),
                    "density": format!("{:.1}", density).parse::<f64>().unwrap(),
                    "viscosity": format!("{:.2}", viscosity).parse::<f64>().unwrap(),
                    "waterContent": format!("{:.3}", random_between(0.1, 2.0)).parse::<f64>().unwrap(),
                    "pumpSpeed": rng.gen_range(1200..1800),
                    "valveStatus": if rng.gen_bool(0.85) { "open" } else { "throttled" },
                    "valveOpenPercent": format!("{:.1}", random_between(60.0, 100.0)).parse::<f64>().unwrap(),
                    "leakDetected": rng.gen_bool(0.02),
                    "batteryLevel": format!("{:.1}", random_between(70.0, 100.0)).parse::<f64>().unwrap(),
                    "signalStrength": rng.gen_range(-85..-50),
                    "lastCalibration": "2025-01-15T08:00:00.000Z",
                    "nextCalibrationDue": "2025-07-15T08:00:00.000Z"
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("L/min"),
                sensor_type: "amr_oil_pipeline".to_string(),
                description: "Automatic meter reading for oil pipeline".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        // ============================================
        // 5 NEW ENDPOINTS - Industrial IoT Sensors
        // ============================================
        "flow-meter" => {
            // อ้างอิงจาก industrial flow meters (Rosemount, Endress+Hauser)
            // Liquid: 0.3-4950 m³/hr, Gas: 3-46000 m³/hr, Steam: 1.6-540000 kg/hr
            let flow_type = ["liquid", "gas", "steam"][rng.gen_range(0..3)];
            let (flow_rate, unit, totalizer) = match flow_type {
                "liquid" => (random_between(10.0, 1000.0), "m³/h", random_between(10000.0, 500000.0)),
                "gas" => (random_between(100.0, 10000.0), "m³/h", random_between(100000.0, 5000000.0)),
                "steam" => (random_between(500.0, 50000.0), "kg/h", random_between(1000000.0, 50000000.0)),
                _ => (0.0, "m³/h", 0.0)
            };
            let temperature = random_between(20.0, 200.0);
            let pressure = random_between(1.0, 20.0);
            let density = if flow_type == "steam" { random_between(1.0, 50.0) } else { random_between(800.0, 1000.0) };
            let meter_types = ["electromagnetic", "vortex", "ultrasonic", "coriolis"];
            let meter_type = meter_types[rng.gen_range(0..4)];
            let quality = generate_data_quality(flow_rate, 10.0, 1000.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("FLW-010", "Flow Meter"),
                equipment_hierarchy: generate_isa95_hierarchy("FLW-010", "Process-Line-J", "Process"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "FLW-010"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "mediaType": flow_type,
                    "flowRate": format!("{:.2}", flow_rate).parse::<f64>().unwrap(),
                    "totalizer": format!("{:.1}", totalizer).parse::<f64>().unwrap(),
                    "temperature": format!("{:.1}", temperature).parse::<f64>().unwrap(),
                    "pressure": format!("{:.2}", pressure).parse::<f64>().unwrap(),
                    "density": format!("{:.1}", density).parse::<f64>().unwrap(),
                    "pipeSize": rng.gen_range(50..300),
                    "meterType": meter_type
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit(unit),
                sensor_type: "flow_meter".to_string(),
                description: "Industrial flow measurement".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "gas-detector" => {
            let co = random_between(0.0, 50.0);
            let h2s = random_between(0.0, 10.0);
            let o2 = random_between(19.5, 23.5);
            let lel = random_between(0.0, 20.0);
            let co_alarm = co > 35.0;
            let h2s_alarm = h2s > 10.0;
            let o2_alarm = o2 < 19.5 || o2 > 23.5;
            let lel_alarm = lel > 10.0;
            let quality = if co_alarm || h2s_alarm || o2_alarm || lel_alarm { DataQuality::Bad } else { DataQuality::Good };
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("GAS-011", "Gas Detector"),
                equipment_hierarchy: generate_isa95_hierarchy("GAS-011", "Confined-Space-K", "Safety"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "GAS-011"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "carbonMonoxide": format!("{:.1}", co).parse::<f64>().unwrap(),
                    "coAlarmSetpoint": 35.0,
                    "hydrogenSulfide": format!("{:.2}", h2s).parse::<f64>().unwrap(),
                    "h2sAlarmSetpoint": 10.0,
                    "oxygen": format!("{:.1}", o2).parse::<f64>().unwrap(),
                    "o2LowAlarm": 19.5,
                    "o2HighAlarm": 23.5,
                    "lel": format!("{:.1}", lel).parse::<f64>().unwrap(),
                    "lelAlarmSetpoint": 10.0,
                    "alarms": {
                        "co": co_alarm,
                        "h2s": h2s_alarm,
                        "o2": o2_alarm,
                        "lel": lel_alarm
                    }
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("ppm"),
                sensor_type: "gas_detector".to_string(),
                description: "4-gas safety monitor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "ph-sensor" => {
            let ph = random_between(4.0, 10.0);
            let orp = random_between(-500.0, 500.0);
            let temperature = random_between(15.0, 40.0);
            let conductivity = random_between(100.0, 5000.0);
            let turbidity = random_between(0.1, 100.0);
            let quality = generate_data_quality(ph, 6.0, 8.5);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("PH-012", "pH Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("PH-012", "Water-Treatment-L", "Water"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "PH-012"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "phValue": format!("{:.2}", ph).parse::<f64>().unwrap(),
                    "orp": format!("{:.1}", orp).parse::<f64>().unwrap(),
                    "temperature": format!("{:.1}", temperature).parse::<f64>().unwrap(),
                    "conductivity": format!("{:.1}", conductivity).parse::<f64>().unwrap(),
                    "turbidity": format!("{:.2}", turbidity).parse::<f64>().unwrap()
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("pH"),
                sensor_type: "ph_sensor".to_string(),
                description: "Water quality pH/ORP sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "level-sensor" => {
            let tank_height = random_between(5.0, 20.0);
            let level = random_between(0.5, tank_height - 0.5);
            let percentage = (level / tank_height) * 100.0;
            let volume = level * random_between(10.0, 100.0);
            let sensor_type = ["ultrasonic", "radar", "guided_wave", "pressure"][rng.gen_range(0..4)];
            let quality = generate_data_quality(percentage, 10.0, 90.0);
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();
            
            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("LVL-013", "Level Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("LVL-013", "Storage-Tank-M", "Tank-Farm"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "LVL-013"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "level": format!("{:.3}", level).parse::<f64>().unwrap(),
                    "tankHeight": format!("{:.1}", tank_height).parse::<f64>().unwrap(),
                    "percentage": format!("{:.2}", percentage).parse::<f64>().unwrap(),
                    "volume": format!("{:.2}", volume).parse::<f64>().unwrap(),
                    "sensorType": sensor_type,
                    "accuracy": "±3mm"
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("m"),
                sensor_type: "level_sensor".to_string(),
                description: "Tank level measurement sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        "proximity-sensor" => {
            let object_detected = rng.gen_bool(0.7);
            let distance = if object_detected { random_between(5.0, 50.0) } else { -1.0 };
            let sensor_type = ["inductive", "capacitive", "photoelectric", "ultrasonic"][rng.gen_range(0..4)];
            let detection_count = rng.gen_range(0..10000);
            let operating_time = random_between(1000.0, 50000.0);
            let quality = if object_detected { DataQuality::Good } else { DataQuality::Uncertain };
            let status_code = generate_opcua_status_code(&quality);
            let source_ts = Utc::now().to_rfc3339();

            let unified = UnifiedSensorData {
                opc_ua: generate_opcua_node("PRX-014", "Proximity Sensor"),
                equipment_hierarchy: generate_isa95_hierarchy("PRX-014", "Conveyor-Station-N", "Material-Handling"),
                sparkplug_topic: generate_sparkplug_topic("Plant-01", "PRX-014"),
                source_timestamp: source_ts,
                server_timestamp: server_ts.clone(),
                value: serde_json::json!({
                    "objectDetected": object_detected,
                    "distance": if distance > 0.0 { Some(format!("{:.1}", distance).parse::<f64>().unwrap()) } else { None },
                    "sensorType": sensor_type,
                    "detectionRange": random_between(1.0, 100.0),
                    "responseTime": random_between(0.1, 10.0),
                    "switchingFrequency": rng.gen_range(100..5000),
                    "detectionCount": detection_count,
                    "operatingTime": format!("{:.1}", operating_time).parse::<f64>().unwrap()
                }),
                data_quality: quality,
                opc_ua_status_code: status_code,
                unit: get_ucum_unit("mm"),
                sensor_type: "proximity_sensor".to_string(),
                description: "Object detection proximity sensor".to_string(),
                properties: serde_json::json!({}),
            };
            Some(serde_json::to_value(unified).unwrap())
        }
        _ => None,
    }
}

const AVAILABLE_SENSORS: &[&str] = &[
    "temperature", "humidity", "oil-level", "oil-pressure",
    "air-quality", "pressure", "vibration", "energy-meter", "amr",
    "flow-meter", "gas-detector", "ph-sensor", "level-sensor", "proximity-sensor"
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
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<SharedState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let method = req.method().to_string();
    let endpoint = req.uri().to_string();
    // Prefer X-Forwarded-For (set by reverse proxy), fall back to real socket IP
    let ip = req.headers().get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());
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

    // Skip noisy internal/polling endpoints from the access log
    let skip = endpoint.starts_with("/api/v1/access-log")
        || endpoint.starts_with("/api/v1/stats")
        || endpoint.starts_with("/events")
        || endpoint.starts_with("/ws/");
    if skip {
        return response;
    }

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

    let port = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4040u16);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("\n  🚀 Simmurator Rust Server running at http://localhost:{}", port);
    println!("  📡 SSE stream at http://localhost:{}/events", port);
    println!("  🔌 WebSocket stream at ws://localhost:{}/ws/sensors", port);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
