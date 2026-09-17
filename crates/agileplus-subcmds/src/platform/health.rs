use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::platform::types::{OverallStatus, PlatformHealth, ServiceHealth, ServiceStatus};

pub(crate) const DEFAULT_API_PORT: u16 = 3000;
pub(crate) const DEFAULT_API_URL: &str = "http://127.0.0.1:3000";

const PROBE_TIMEOUT: Duration = Duration::from_millis(800);

/// Poll the health endpoint until healthy or timeout.
pub(crate) fn wait_for_health(
    api_url: &str,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<PlatformHealth> {
    let start = Instant::now();
    let health_url = format!("{api_url}/health");

    loop {
        if start.elapsed() >= timeout {
            return Err(anyhow!("Timed out after {}s", timeout.as_secs()));
        }
        let pct = ((start.elapsed().as_secs_f64() / timeout.as_secs_f64()) * 100.0) as u32;
        let filled = (pct / 10) as usize;
        let bar: String = "█".repeat(filled.min(10)) + &"░".repeat(10usize.saturating_sub(filled));
        print!("\r[{bar}] {pct}%");
        match try_health_check(&health_url) {
            Ok(h) => {
                println!();
                return Ok(h);
            }
            Err(_) => {
                std::thread::sleep(poll_interval);
            }
        }
    }
}

/// Attempt a single HTTP GET to the health endpoint.
fn try_health_check(url: &str) -> Result<PlatformHealth> {
    let started = Instant::now();
    let api_port = http_url_port(url)?;
    let body = http_get(url)?;
    let latency_ms = started.elapsed().as_millis() as u64;

    let ok = body.contains("\"status\":\"ok\"")
        || body.contains("\"status\": \"ok\"")
        || body.contains("\"status\":\"healthy\"")
        || body.to_lowercase().contains("ok");

    if !ok {
        return Err(anyhow!("unexpected health body: {body}"));
    }

    Ok(PlatformHealth {
        services: vec![ServiceHealth {
            name: "API".to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(latency_ms),
            uptime: None,
            port: Some(api_port),
            last_check: Some("just now".to_string()),
        }],
        overall: OverallStatus::Healthy,
    })
}

fn http_url_port(url: &str) -> Result<u16> {
    let authority = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("only http supported: {url}"))?
        .split('/')
        .next()
        .ok_or_else(|| anyhow!("missing HTTP authority"))?;
    match authority.rsplit_once(':') {
        Some((_, port)) => port
            .parse()
            .map_err(|_| anyhow!("invalid HTTP port in {url}")),
        None => Ok(80),
    }
}

/// Fetch platform health from API, then enrich with direct dependency probes.
pub(crate) fn fetch_platform_health(health_url: &str) -> PlatformHealth {
    let api_port = http_url_port(health_url).unwrap_or(DEFAULT_API_PORT);
    let api = match try_health_check(health_url) {
        Ok(h) => h.services.into_iter().next().unwrap_or(ServiceHealth {
            name: "API".to_string(),
            status: ServiceStatus::Unknown,
            latency_ms: None,
            uptime: None,
            port: Some(api_port),
            last_check: None,
        }),
        Err(_) => ServiceHealth {
            name: "API".to_string(),
            status: ServiceStatus::Unknown,
            latency_ms: None,
            uptime: None,
            port: Some(api_port),
            last_check: None,
        },
    };

    let mut services = vec![
        api,
        probe_http("NATS", 8222, "http://127.0.0.1:8222/healthz"),
        probe_tcp("Dragonfly", 6379),
        probe_tcp("Neo4j", 7687),
        probe_http("MinIO", 9000, "http://127.0.0.1:9000/minio/health/live"),
    ];

    // Prefer Ready when TCP is up but we have no richer signal.
    for svc in &mut services {
        if svc.name == "Dragonfly" && svc.status == ServiceStatus::Healthy {
            svc.status = ServiceStatus::Ready;
        }
    }

    let overall = overall_from(&services);
    PlatformHealth { services, overall }
}

fn overall_from(services: &[ServiceHealth]) -> OverallStatus {
    let any_down = services
        .iter()
        .any(|s| matches!(s.status, ServiceStatus::Unknown | ServiceStatus::Unhealthy));
    let any_degraded = services
        .iter()
        .any(|s| matches!(s.status, ServiceStatus::Degraded));
    if any_down {
        // API-up with optional deps down is degraded, not fully down.
        let api_up = services
            .iter()
            .any(|s| s.name == "API" && s.status == ServiceStatus::Healthy);
        if api_up {
            OverallStatus::Degraded
        } else {
            OverallStatus::Down
        }
    } else if any_degraded {
        OverallStatus::Degraded
    } else {
        OverallStatus::Healthy
    }
}

fn probe_http(name: &str, port: u16, url: &str) -> ServiceHealth {
    let started = Instant::now();
    match http_get(url) {
        Ok(_) => ServiceHealth {
            name: name.to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(started.elapsed().as_millis() as u64),
            uptime: None,
            port: Some(port),
            last_check: Some("just now".to_string()),
        },
        Err(_) => ServiceHealth {
            name: name.to_string(),
            status: ServiceStatus::Unknown,
            latency_ms: None,
            uptime: None,
            port: Some(port),
            last_check: None,
        },
    }
}

fn probe_tcp(name: &str, port: u16) -> ServiceHealth {
    let started = Instant::now();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) {
        Ok(_) => ServiceHealth {
            name: name.to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(started.elapsed().as_millis() as u64),
            uptime: None,
            port: Some(port),
            last_check: Some("just now".to_string()),
        },
        Err(_) => ServiceHealth {
            name: name.to_string(),
            status: ServiceStatus::Unknown,
            latency_ms: None,
            uptime: None,
            port: Some(port),
            last_check: None,
        },
    }
}

fn http_get(url: &str) -> Result<String> {
    let url = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow!("only http supported: {url}"))?;
    let (host_port, path) = match url.split_once('/') {
        Some((hp, rest)) => (hp, format!("/{rest}")),
        None => (url, "/".to_string()),
    };
    let host = host_port
        .split(':')
        .next()
        .ok_or_else(|| anyhow!("bad host"))?;
    // Prefer IPv4 for localhost to avoid ::1 failures when API binds IPv4-only.
    let addr = if host == "localhost" {
        let port: u16 = host_port
            .split(':')
            .nth(1)
            .unwrap_or("80")
            .parse()
            .unwrap_or(80);
        SocketAddr::from(([127, 0, 0, 1], port))
    } else {
        host_port
            .to_socket_addrs()?
            .find(|a| a.is_ipv4())
            .or_else(|| host_port.to_socket_addrs().ok().and_then(|mut i| i.next()))
            .ok_or_else(|| anyhow!("dns failed for {host_port}"))?
    };

    let mut stream = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT)?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT))?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT))?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\nAccept: */*\r\n\r\n"
    );
    stream.write_all(req.as_bytes())?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    let (header, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    if !header.contains("200") {
        return Err(anyhow!(
            "non-200 response: {}",
            header.lines().next().unwrap_or("")
        ));
    }
    Ok(body.to_string())
}

/// Synthetic healthy state used when the real HTTP stack is unavailable.
pub(crate) fn synthetic_platform_health() -> PlatformHealth {
    let services = vec![
        ServiceHealth {
            name: "API".to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(1),
            uptime: Some("2s".to_string()),
            port: Some(DEFAULT_API_PORT),
            last_check: Some("just now".to_string()),
        },
        ServiceHealth {
            name: "NATS".to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(2),
            uptime: Some("2s".to_string()),
            port: Some(4222),
            last_check: Some("just now".to_string()),
        },
        ServiceHealth {
            name: "Dragonfly".to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(1),
            uptime: Some("2s".to_string()),
            port: Some(6379),
            last_check: Some("just now".to_string()),
        },
        ServiceHealth {
            name: "Neo4j".to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(5),
            uptime: Some("2s".to_string()),
            port: Some(7687),
            last_check: Some("just now".to_string()),
        },
        ServiceHealth {
            name: "MinIO".to_string(),
            status: ServiceStatus::Healthy,
            latency_ms: Some(8),
            uptime: Some("2s".to_string()),
            port: Some(9000),
            last_check: Some("just now".to_string()),
        },
        ServiceHealth {
            name: "SQLite".to_string(),
            status: ServiceStatus::Ready,
            latency_ms: Some(3),
            uptime: Some("2s".to_string()),
            port: None,
            last_check: Some("just now".to_string()),
        },
    ];
    PlatformHealth {
        services,
        overall: OverallStatus::Healthy,
    }
}

pub(crate) fn print_status_table_up(services: &[ServiceHealth]) {
    println!("{:<14} {:<9} {:<9} Port", "Service", "Status", "Uptime");
    println!("{}", "─".repeat(45));
    for svc in services {
        let port_str = svc
            .port
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        let uptime = svc.uptime.as_deref().unwrap_or("-");
        println!(
            "{:<14} {:<9} {:<9} {}",
            svc.name,
            svc.status.to_string(),
            uptime,
            port_str,
        );
    }
}

pub(crate) fn print_status_table(services: &[ServiceHealth]) {
    println!(
        "{:<14} {:<11} {:<10} {:<12} Last Check",
        "Service", "Status", "Latency", "Uptime"
    );
    println!("{}", "─".repeat(63));
    for svc in services {
        let latency = match svc.latency_ms {
            Some(ms) => format!("{ms}ms"),
            None => "TIMEOUT".to_string(),
        };
        let uptime = svc.uptime.as_deref().unwrap_or("--");
        let last_check = svc.last_check.as_deref().unwrap_or("--");
        let indicator = match svc.status {
            ServiceStatus::Degraded => " ⚠",
            ServiceStatus::Unhealthy => " ✗",
            _ => "",
        };
        println!(
            "{:<14} {:<11} {:<10} {:<12} {}{}",
            svc.name,
            svc.status.to_string(),
            latency,
            uptime,
            last_check,
            indicator,
        );
    }
}
