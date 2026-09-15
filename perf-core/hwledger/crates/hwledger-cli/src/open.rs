use anyhow::Result;
use colored::Colorize;
use std::process::Command;

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/Users/kooshapari".into())
}

pub async fn run() -> Result<()> {
    println!("{}", "Opening all hwLedger services...".bold().cyan());
    println!();

    let urls = [
        ("http://localhost:8090", "Bench-cockpit"),
        ("http://localhost:5173", "Bench-cockpit Vite HMR"),
        ("http://localhost:1420", "HWLedger app HMR"),
    ];

    for (url, name) in &urls {
        println!("  Opening {} ({})...", url, name);
        Command::new("open").arg(url).spawn()?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let apps = ["BenchMatrix.app", "HWLedger.app", "BenchCockpit.app"];

    println!();
    let apps_dir = format!("{}/Applications", home_dir());
    for app in &apps {
        let path = format!("{}/{}", apps_dir, app);
        println!("  Opening {}...", path);
        match Command::new("open").arg(&path).spawn() {
            Ok(_) => {}
            Err(e) => println!("    {} {}: {}", "!".red().bold(), path, e),
        }
    }

    println!();
    println!("{}", "All services opened.".bold().green());
    Ok(())
}
