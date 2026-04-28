use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::Ordering,
    thread,
};

use base64::{engine::general_purpose, Engine as _};
use chrono::Local;
use serde::Serialize;

use crate::{app_state::AppState, models::AppStatus, recorder, summary::SummaryService};

const DEV_BRIDGE_ADDR: &str = "127.0.0.1:1421";

pub fn start(state: AppState) {
    thread::spawn(move || {
        let listener = match TcpListener::bind(DEV_BRIDGE_ADDR) {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("MindBack dev bridge unavailable at {DEV_BRIDGE_ADDR}: {error}");
                return;
            }
        };

        eprintln!("MindBack dev bridge listening at http://{DEV_BRIDGE_ADDR}");

        for stream in listener.incoming() {
            let state = state.clone();
            match stream {
                Ok(mut stream) => {
                    thread::spawn(move || {
                        let response = read_http_request(&mut stream)
                            .and_then(|request| handle_request(&state, request))
                            .unwrap_or_else(|error| HttpResponse::text(500, &error));
                        let _ = write_http_response(&mut stream, response);
                    });
                }
                Err(error) => eprintln!("MindBack dev bridge connection failed: {error}"),
            }
        }
    });
}

#[derive(Debug, Clone)]
struct DevBridgeRequest {
    method: String,
    path: String,
    query: String,
    body: String,
}

impl DevBridgeRequest {
    fn new(method: &str, uri: &str, body: &str) -> Self {
        let (path, query) = split_uri(uri);
        Self {
            method: method.to_string(),
            path,
            query,
            body: body.to_string(),
        }
    }

    fn query_value(&self, key: &str) -> Option<String> {
        self.query.split('&').find_map(|pair| {
            let (pair_key, value) = pair.split_once('=')?;
            if percent_decode(pair_key) == key {
                return Some(percent_decode(value));
            }
            None
        })
    }
}

#[derive(Debug)]
struct HttpResponse {
    status_code: u16,
    content_type: &'static str,
    body: String,
}

impl HttpResponse {
    fn empty(status_code: u16) -> Self {
        Self {
            status_code,
            content_type: "text/plain; charset=utf-8",
            body: String::new(),
        }
    }

    fn text(status_code: u16, body: &str) -> Self {
        Self {
            status_code,
            content_type: "text/plain; charset=utf-8",
            body: body.to_string(),
        }
    }
}

fn handle_request(state: &AppState, request: DevBridgeRequest) -> Result<HttpResponse, String> {
    if request.method == "OPTIONS" {
        return Ok(HttpResponse::empty(204));
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/config") => {
            json_response(&state.storage.read_config().map_err(to_bridge_error)?)
        }
        ("POST", "/api/config") => {
            let config = serde_json::from_str(&request.body).map_err(|error| error.to_string())?;
            state
                .storage
                .save_config(&config)
                .map_err(to_bridge_error)?;
            json_response(&config)
        }
        ("GET", "/api/status") => json_response(&status_for_state(state)?),
        ("POST", "/api/start-recording") => {
            state.start_recording_worker();
            json_response(&status_for_state(state)?)
        }
        ("POST", "/api/stop-recording") => {
            state.stop_recording_worker();
            json_response(&status_for_state(state)?)
        }
        ("POST", "/api/record-once") => {
            let config = state.storage.read_config().map_err(to_bridge_error)?;
            match recorder::record_once(&state.storage, &config) {
                Ok(entry) => json_response(&entry),
                Err(error) => {
                    if let Ok(mut last_error) = state.last_error.lock() {
                        *last_error = Some(error.to_string());
                    }
                    Err(error.to_string())
                }
            }
        }
        ("GET", "/api/today-entries") => json_response(
            &state
                .storage
                .list_today_entries()
                .map_err(to_bridge_error)?,
        ),
        ("GET", "/api/today-thumbnail") => {
            let screenshot_thumb = request
                .query_value("screenshot_thumb")
                .ok_or_else(|| "missing screenshot_thumb".to_string())?;
            let bytes = state
                .storage
                .read_today_thumb(&screenshot_thumb)
                .map_err(to_bridge_error)?;
            let data_url = format!(
                "data:image/jpeg;base64,{}",
                general_purpose::STANDARD.encode(bytes)
            );
            json_response(&data_url)
        }
        ("POST", "/api/summary") => {
            let path = state
                .storage
                .write_today_summary()
                .map_err(to_bridge_error)?;
            json_response(&path.display().to_string())
        }
        ("GET", "/api/summary-blocks") => json_response(
            &SummaryService::new(&state.storage)
                .today_summary_blocks()
                .map_err(to_bridge_error)?,
        ),
        ("POST", "/api/summarize-previous-half-hour") => {
            let config = state.storage.read_config().map_err(to_bridge_error)?;
            let block = SummaryService::new(&state.storage)
                .summarize_previous_half_hour(&config)
                .map_err(to_bridge_error)?;
            json_response(&block)
        }
        _ => Ok(HttpResponse::text(404, "not found")),
    }
}

fn status_for_state(state: &AppState) -> Result<AppStatus, String> {
    let config = state.storage.read_config().map_err(to_bridge_error)?;
    let last_error = state
        .last_error
        .lock()
        .map_err(|error| error.to_string())?
        .clone();

    Ok(AppStatus {
        is_recording: state.recording.load(Ordering::SeqCst),
        today: Local::now().date_naive().format("%Y-%m-%d").to_string(),
        project_name: config.project_name,
        last_error,
    })
}

fn to_bridge_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn json_response<T: Serialize>(value: &T) -> Result<HttpResponse, String> {
    Ok(HttpResponse {
        status_code: 200,
        content_type: "application/json; charset=utf-8",
        body: serde_json::to_string(value).map_err(|error| error.to_string())?,
    })
}

fn read_http_request(stream: &mut TcpStream) -> Result<DevBridgeRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];

    loop {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
        if header_end(&buffer).is_some() {
            break;
        }
    }

    let header_end = header_end(&buffer).ok_or_else(|| "invalid HTTP request".to_string())?;
    let header_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = content_length(&header_text);
    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let count = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
    }

    let body = String::from_utf8_lossy(&buffer[body_start..buffer.len()]).to_string();
    parse_http_request(&format!("{header_text}\r\n\r\n{body}"))
}

fn parse_http_request(raw: &str) -> Result<DevBridgeRequest, String> {
    let (headers, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid HTTP request".to_string())?;
    let request_line = headers
        .lines()
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?;
    let uri = parts
        .next()
        .ok_or_else(|| "missing request uri".to_string())?;

    Ok(DevBridgeRequest::new(method, uri, body))
}

fn write_http_response(stream: &mut TcpStream, response: HttpResponse) -> std::io::Result<()> {
    let reason = match response.status_code {
        200 => "OK",
        204 => "No Content",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    let body = response.body.as_bytes();
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        response.status_code,
        reason,
        response.content_type,
        body.len()
    )?;
    stream.write_all(body)
}

fn split_uri(uri: &str) -> (String, String) {
    match uri.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (uri.to_string(), String::new()),
    }
}

fn header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                return value.trim().parse().ok();
            }
            None
        })
        .unwrap_or(0)
}

fn percent_decode(value: &str) -> String {
    let mut decoded = String::new();
    let mut bytes = value.as_bytes().iter().copied();

    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let first = bytes.next();
            let second = bytes.next();
            if let (Some(first), Some(second)) = (first, second) {
                if let Ok(hex) = u8::from_str_radix(&String::from_utf8_lossy(&[first, second]), 16)
                {
                    decoded.push(hex as char);
                    continue;
                }
                decoded.push('%');
                decoded.push(first as char);
                decoded.push(second as char);
                continue;
            }
            decoded.push('%');
            continue;
        }

        decoded.push(if byte == b'+' { ' ' } else { byte as char });
    }

    decoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    use crate::{app_state::AppState, models::AppConfig, storage::Storage};

    #[test]
    fn parses_query_value_with_encoded_slash() {
        let request = parse_http_request(
            "GET /api/today-thumbnail?screenshot_thumb=thumbs%2Fone.jpg HTTP/1.1\r\n\r\n",
        )
        .unwrap();

        assert_eq!(
            request.query_value("screenshot_thumb"),
            Some("thumbs/one.jpg".to_string())
        );
    }

    #[test]
    fn status_route_returns_json_status() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        storage
            .save_config(&AppConfig {
                project_name: "MindBack MVP".to_string(),
                ..AppConfig::default()
            })
            .unwrap();
        let state = AppState::new_with_storage(storage);

        let response =
            handle_request(&state, DevBridgeRequest::new("GET", "/api/status", "")).unwrap();

        assert_eq!(response.status_code, 200);
        assert!(response.body.contains("\"project_name\":\"MindBack MVP\""));
    }

    #[test]
    fn summary_blocks_route_returns_json_blocks() {
        let dir = tempdir().unwrap();
        let storage = Storage::new(dir.path()).unwrap();
        let state = AppState::new_with_storage(storage);

        let response = handle_request(
            &state,
            DevBridgeRequest::new("GET", "/api/summary-blocks", ""),
        )
        .unwrap();

        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, "[]");
    }
}
