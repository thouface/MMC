//! Embedded HTTP Server for HTML GUI

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Response, Server};

const HTML_GUI: &str = include_str!("../../../bindings/gui/index.html");

pub fn start_server(running: Arc<AtomicBool>) -> u16 {
    // 先用临时 listener 获取空闲端口
    let temp_listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port");
    let port = temp_listener.local_addr().expect("Failed to get local address").port();
    
    // 释放临时 listener，这样端口就会被释放
    drop(temp_listener);

    // 使用获取到的端口创建服务器
    let server = Server::http(format!("127.0.0.1:{}", port)).expect("Failed to start HTTP server");

    tracing::info!("Embedded HTTP server listening on port {}", port);

    // Handle requests in a separate thread
    let server_running = running.clone();
    thread::spawn(move || {
        while server_running.load(std::sync::atomic::Ordering::Relaxed) {
            match server.try_recv() {
                Ok(Some(request)) => {
                    let response = match request.url() {
                        "/" | "/index.html" => {
                            Response::from_string(HTML_GUI)
                                .with_header(
                                    Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
                                )
                        }
                        "/api/status" => {
                            let status = serde_json::json!({
                                "status": "online",
                                "version": crate::VERSION,
                                "platform": std::env::consts::OS,
                            });
                            Response::from_string(status.to_string())
                                .with_header(
                                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                                )
                        }
                        "/api/devices" => {
                            let devices = serde_json::json!([
                                {"id": "device-001", "name": "Xiaomi 14 Pro", "type": "Android", "online": true},
                                {"id": "device-002", "name": "ThinkPad X1 Carbon", "type": "Windows", "online": true},
                                {"id": "device-003", "name": "Apple TV 4K", "type": "tvOS", "online": false}
                            ]);
                            Response::from_string(devices.to_string())
                                .with_header(
                                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                                )
                        }
                        _ => {
                            Response::from_string("Not Found")
                                .with_status_code(404)
                        }
                    };

                    if let Err(e) = request.respond(response) {
                        tracing::warn!("Failed to send response: {}", e);
                    }
                }
                Ok(None) => {
                    // No request available, sleep briefly
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    tracing::error!("Server error: {}", e);
                    break;
                }
            }
        }

        tracing::info!("Embedded HTTP server stopped");
    });

    port
}
