use anyhow::Result;
use colored::Colorize;
use std::process::Command;
use tokio::signal;

const PROJECT_ROOT: &str = "/Users/kooshapari/CodeProjects/Phenotype/repos/hwLedger";

pub async fn run() -> Result<()> {
    println!("{}", "=== hwLedger Dev Servers ===".bold().yellow());
    println!();

    let go_server_dir = format!("{}/sidecars/bench-cockpit/server", PROJECT_ROOT);
    let bench_cockpit_dir = format!("{}/sidecars/bench-cockpit", PROJECT_ROOT);
    let hwledger_app_dir = format!("{}/apps/hwledger-app", PROJECT_ROOT);

    println!("[1/4] bench-cockpit Go server (:8090)...");
    let mut go_child = Command::new("go")
        .arg("run")
        .arg(".")
        .current_dir(&go_server_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    println!("  PID: {}", go_child.id());

    println!("[2/4] bench-cockpit Vite dev (:5173)...");
    let mut vite_child = Command::new("bun")
        .arg("run")
        .arg("dev")
        .current_dir(&bench_cockpit_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    println!("  PID: {}", vite_child.id());

    println!("[3/4] hwledger-app Vite dev (:1420)...");
    let mut hwchild = Command::new("bun")
        .arg("run")
        .arg("dev")
        .current_dir(&hwledger_app_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    println!("  PID: {}", hwchild.id());

    println!("[4/4] MLX server (:8766)...");
    let mlx_running = reqwest::get("http://localhost:8766/v1/models")
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let mut mlx_child = if mlx_running {
        println!("  {}", "Already running".green());
        None
    } else {
        println!("  Starting...");
        let child = Command::new("/opt/homebrew/bin/mlx_lm.server")
            .args([
                "--model",
                "mlx-community/Qwen3.5-0.8B-4bit",
                "--host",
                "0.0.0.0",
                "--port",
                "8766",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        println!("  PID: {}", child.id());
        Some(child)
    };

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    println!();
    println!("{}", "=== All servers running ===".bold().green());
    println!();
    println!("  Open in browser:");
    println!("    http://localhost:8090  — Bench-cockpit (Go + React)");
    println!("    http://localhost:5173  — Bench-cockpit Vite HMR");
    println!("    http://localhost:1420  — HWLedger app HMR");
    println!();
    println!("  Native apps:");
    println!("    open ~/Applications/BenchMatrix.app");
    println!("    open ~/Applications/HWLedger.app");
    println!("    open ~/Applications/BenchCockpit.app");
    println!();
    println!("  MLX server: http://localhost:8766");
    println!("  Langfuse:   http://localhost:3000");
    println!();
    println!("{}", "Press Ctrl+C to stop all servers".bold().yellow());

    signal::ctrl_c().await?;

    println!("\nShutting down...");

    go_child.kill()?;
    vite_child.kill()?;
    hwchild.kill()?;
    if let Some(ref mut m) = mlx_child {
        m.kill()?;
    }

    println!("All servers stopped.");
    Ok(())
}
