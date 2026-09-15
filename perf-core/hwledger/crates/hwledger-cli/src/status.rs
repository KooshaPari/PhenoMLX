use anyhow::Result;
use colored::Colorize;
use std::time::Duration;

struct Service {
    name: &'static str,
    url: String,
}

fn build_services() -> Vec<Service> {
    vec![
        Service { name: "bench-cockpit", url: "http://localhost:8090".into() },
        Service { name: "bench-cockpit-vite", url: "http://localhost:5173".into() },
        Service { name: "hwledger-app", url: "http://localhost:1420".into() },
        Service { name: "mlx-server", url: "http://localhost:8766/v1/models".into() },
        Service { name: "langfuse", url: "http://localhost:3000".into() },
    ]
}

pub async fn run() -> Result<()> {
    println!("{}", "=== hwLedger Service Status ===".bold().cyan());
    println!();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;

    let services = build_services();
    let name_width = services.iter().map(|s| s.name.len()).max().unwrap_or(20);

    println!(
        "  {:<name_width$}  {:<10}  {}",
        "SERVICE".bold(),
        "STATUS".bold(),
        "URL".bold()
    );
    println!(
        "  {:<name_width$}  {:<10}  {}",
        "-".repeat(name_width),
        "-".repeat(10),
        "-".repeat(30)
    );

    for svc in &services {
        let status = match client.get(&svc.url).send().await {
            Ok(resp) if resp.status().is_success() => "UP".green(),
            Ok(resp) => format!("HTTP {}", resp.status()).yellow(),
            Err(_) => "DOWN".red(),
        };

        println!(
            "  {:<name_width$}  {:<10}  {}",
            svc.name,
            status,
            svc.url
        );
    }

    println!();
    Ok(())
}
