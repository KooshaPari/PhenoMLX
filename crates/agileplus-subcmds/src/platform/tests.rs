use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

fn environment_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn health_listener() -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind health listener");
    listener
        .set_nonblocking(true)
        .expect("make health listener nonblocking");
    let address = listener.local_addr().expect("health listener address");
    let handle = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_nonblocking(false)
                        .expect("make accepted health stream blocking");
                    let mut request = [0_u8; 1024];
                    let bytes = stream.read(&mut request).expect("read health request");
                    let request = String::from_utf8_lossy(&request[..bytes]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or_default()
                        .to_owned();
                    if path == "/health" {
                        stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}")
                            .expect("reply to health request");
                    }
                    return path;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if std::time::Instant::now() >= deadline {
                        return "<no request>".to_string();
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("accept health request: {error}"),
            }
        }
    });
    (format!("http://{address}"), handle)
}

fn set_environment(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    // Tests hold `environment_lock` while mutating process-global environment state.
    unsafe { std::env::set_var(key, value) };
}

fn remove_environment(key: &str) {
    // Tests hold `environment_lock` while mutating process-global environment state.
    unsafe { std::env::remove_var(key) };
}

#[test]
fn test_service_status_display() {
    assert_eq!(ServiceStatus::Healthy.to_string(), "Healthy");
    assert_eq!(ServiceStatus::Degraded.to_string(), "Degraded");
    assert_eq!(ServiceStatus::Unhealthy.to_string(), "Unhealthy");
    assert_eq!(ServiceStatus::Unknown.to_string(), "Unknown");
}

#[test]
fn test_overall_status_display() {
    assert_eq!(OverallStatus::Healthy.to_string(), "HEALTHY");
    assert_eq!(OverallStatus::Degraded.to_string(), "DEGRADED");
    assert_eq!(OverallStatus::Down.to_string(), "DOWN");
}

#[test]
fn test_synthetic_platform_health() {
    let h = health::synthetic_platform_health();
    assert_eq!(h.services.len(), 6);
    assert_eq!(h.overall, OverallStatus::Healthy);
    assert!(h
        .services
        .iter()
        .all(|s| s.status == ServiceStatus::Healthy || s.status == ServiceStatus::Ready));
}

#[test]
fn test_platform_status_down_when_api_unreachable() {
    let health = health::fetch_platform_health("http://127.0.0.1:19999/health");
    assert_eq!(health.overall, OverallStatus::Down);
    assert_eq!(health.services[0].status, ServiceStatus::Unknown);
    assert_eq!(health.services[0].port, Some(19_999));
}

#[test]
fn platform_status_uses_the_resolved_health_url_when_no_flag_is_supplied() {
    let args = PlatformStatusArgs {
        api_url: "http://127.0.0.1:3000".to_string(),
    };

    assert_eq!(
        status::status_probe_target(&args, Some("http://127.0.0.1:3014"), None)
            .expect("resolved target"),
        "http://127.0.0.1:3014/health"
    );
}

#[test]
fn platform_status_loads_the_persisted_runtime_health_target() {
    let runtime_dir = tempfile::tempdir().expect("temporary runtime directory");
    let runtime_file = runtime_dir.path().join("local-ports.env");
    std::fs::write(
        &runtime_file,
        "AGILEPLUS_API_PORT=3014\nAGILEPLUS_GRPC_PORT=5014\nAGILEPLUS_API_URL=http://127.0.0.1:3014\nAGILEPLUS_API_HEALTH_URL=http://127.0.0.1:3014/health\n",
    )
    .expect("write persisted runtime");
    let args = PlatformStatusArgs {
        api_url: "http://127.0.0.1:3000".to_string(),
    };

    assert_eq!(
        status::status_probe_target(&args, None, Some(runtime_file.as_path()))
            .expect("resolved target"),
        "http://127.0.0.1:3014/health"
    );
}

#[test]
fn public_platform_status_uses_health_from_environment_override() {
    let _guard = environment_lock().lock().expect("lock environment");
    let (api_url, listener) = health_listener();
    let prior_api_url = std::env::var("AGILEPLUS_API_URL").ok();
    set_environment("AGILEPLUS_API_URL", &api_url);

    let result = run_platform_status(PlatformStatusArgs {
        api_url: "http://127.0.0.1:3000".to_string(),
    });

    match prior_api_url {
        Some(value) => set_environment("AGILEPLUS_API_URL", value),
        None => remove_environment("AGILEPLUS_API_URL"),
    }
    assert!(result.is_ok());
    assert_eq!(listener.join().expect("health listener thread"), "/health");
}

#[test]
fn public_platform_status_loads_api_only_persisted_target_from_agileplus_root() {
    let _guard = environment_lock().lock().expect("lock environment");
    let runtime_root = tempfile::tempdir().expect("temporary AgilePlus root");
    let ports_dir = runtime_root.path().join(".agileplus/runtime");
    std::fs::create_dir_all(&ports_dir).expect("create runtime directory");
    let (api_url, listener) = health_listener();
    std::fs::write(
        ports_dir.join("local-ports.env"),
        format!("AGILEPLUS_API_URL={api_url}\n"),
    )
    .expect("write persisted runtime");
    let prior_api_url = std::env::var("AGILEPLUS_API_URL").ok();
    let prior_root = std::env::var("AGILEPLUS_ROOT").ok();
    remove_environment("AGILEPLUS_API_URL");
    set_environment("AGILEPLUS_ROOT", runtime_root.path());

    let result = run_platform_status(PlatformStatusArgs {
        api_url: "http://127.0.0.1:3000".to_string(),
    });

    match prior_api_url {
        Some(value) => set_environment("AGILEPLUS_API_URL", value),
        None => remove_environment("AGILEPLUS_API_URL"),
    }
    match prior_root {
        Some(value) => set_environment("AGILEPLUS_ROOT", value),
        None => remove_environment("AGILEPLUS_ROOT"),
    }
    assert!(result.is_ok());
    assert_eq!(listener.join().expect("health listener thread"), "/health");
}

#[test]
fn health_status_reports_the_resolved_api_port() {
    let (api_url, listener) = health_listener();
    let health = health::fetch_platform_health(&format!("{api_url}/health"));

    assert_eq!(
        health.services[0].port,
        api_url
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
    );
    assert_eq!(listener.join().expect("health listener thread"), "/health");
}

#[test]
fn test_print_status_table_does_not_panic() {
    let health = health::synthetic_platform_health();
    // Should not panic — just print.
    health::print_status_table(&health.services);
    health::print_status_table_up(&health.services);
}

#[test]
fn test_platform_down_args_defaults() {
    let args = PlatformDownArgs {
        config: "process-compose.yml".to_string(),
        timeout: 30,
    };
    assert_eq!(args.timeout, 30);
}

#[test]
fn test_platform_logs_args() {
    let args = PlatformLogsArgs {
        config: "process-compose.yml".to_string(),
        service: Some("nats".to_string()),
        follow: true,
        lines: 50,
        since: Some("1h".to_string()),
    };
    assert_eq!(args.service.as_deref(), Some("nats"));
    assert!(args.follow);
    assert_eq!(args.lines, 50);
    assert_eq!(args.since.as_deref(), Some("1h"));
}

#[test]
fn test_find_process_compose_returns_path_in_test_cfg() {
    // In test cfg, which_process_compose always returns Some.
    let result = process_compose::find_process_compose();
    assert!(result.is_some());
}
