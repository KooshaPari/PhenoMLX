use anyhow::Result;
use colored::Colorize;
use std::path::Path;
use std::process::Command;

const PROJECT_ROOT: &str = "/Users/kooshapari/CodeProjects/Phenotype/repos/hwLedger";

struct AppBundle {
    name: &'static str,
    source_relative: &'static str,
}

const APPS: &[AppBundle] = &[
    AppBundle {
        name: "BenchMatrix.app",
        source_relative: "apps/bench-matrix/BenchMatrix.app",
    },
    AppBundle {
        name: "HWLedger.app",
        source_relative: "apps/hwledger-app/HWLedger.app",
    },
    AppBundle {
        name: "BenchCockpit.app",
        source_relative: "sidecars/bench-cockpit/BenchCockpit.app",
    },
];

pub async fn run() -> Result<()> {
    println!("{}", "=== hwLedger Install ===".bold().cyan());
    println!();

    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/kooshapari".into());
    let apps_dir_path = Path::new(&home).join("Applications");
    std::fs::create_dir_all(&apps_dir_path)?;

    for app in APPS {
        let src = Path::new(PROJECT_ROOT).join(app.source_relative);
        let dst = apps_dir_path.join(app.name);

        if !src.exists() {
            println!(
                "  {} {} — source not found at {}",
                "!".yellow().bold(),
                app.name,
                src.display()
            );
            continue;
        }

        if dst.exists() {
            println!(
                "  {} {} — removing existing",
                "~".dimmed(),
                app.name
            );
            std::fs::remove_dir_all(&dst)?;
        }

        print!("  Copying {}...", app.name);
        let status = Command::new("cp")
            .arg("-R")
            .arg(&src)
            .arg(&dst)
            .status()?;

        if status.success() {
            println!(" {}", "done".green());
        } else {
            println!(" {} (exit code: {})", "failed".red(), status.code().unwrap_or(-1));
        }
    }

    println!();
    println!("{}", "Install complete.".bold().green());
    println!("  Apps are in: {}", apps_dir_path.display());
    Ok(())
}
