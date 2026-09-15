use anyhow::Result;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const PROBLEMS: &[&str] = &[
    "What is 2 + 3?",
    "Name the capital of France.",
    "What color is the sky on a clear day?",
    "Translate 'hello' to Spanish.",
    "What planet is known as the Red Planet?",
    "How many continents are there?",
    "What is the boiling point of water in Celsius?",
    "Who wrote Romeo and Juliet?",
    "What gas do plants absorb from the atmosphere?",
    "What is the largest ocean on Earth?",
];

#[derive(Serialize, Deserialize)]
struct BenchmarkResult {
    model: String,
    variant: String,
    mlx_url: String,
    timestamp: String,
    problems: Vec<ProblemResult>,
    summary: Summary,
}

#[derive(Serialize, Deserialize)]
struct ProblemResult {
    index: usize,
    prompt: String,
    response: String,
    latency_ms: u64,
    tokens: usize,
}

#[derive(Serialize, Deserialize)]
struct Summary {
    total_problems: usize,
    avg_latency_ms: f64,
    total_tokens: usize,
    tokens_per_second: f64,
}

pub async fn run(model: &str, variant: &str, mlx_url: &str, output_dir: &str) -> Result<()> {
    println!("{}", "=== hwLedger Benchmark Suite ===".bold().cyan());
    println!();
    println!("  Model:   {}", model);
    println!("  Variant: {}", variant);
    println!("  MLX URL: {}", mlx_url);
    println!("  Output:  {}", output_dir);
    println!();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    let models_url = format!("{}/models", mlx_url);
    match client.get(&models_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            println!("  {}", "MLX server reachable".green());
        }
        _ => {
            println!("  {} MLX server not reachable at {}", "ERROR:".red().bold(), mlx_url);
            return Err(anyhow::anyhow!("MLX server not reachable at {}", mlx_url));
        }
    }
    println!();

    let mut results = Vec::new();
    let mut total_tokens = 0usize;
    let mut total_latency_ms = 0u64;

    for (i, prompt) in PROBLEMS.iter().enumerate() {
        println!(
            "  [{}/{}] {}",
            (i + 1).to_string().bold(),
            PROBLEMS.len().to_string().bold(),
            prompt.dimmed()
        );

        let start = std::time::Instant::now();
        let response = send_completion(&client, mlx_url, model, prompt).await?;
        let latency_ms = start.elapsed().as_millis() as u64;

        let tokens = response
            .get("usage")
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as usize;

        total_tokens += tokens;
        total_latency_ms += latency_ms;

        let text = response
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .unwrap_or("(no response)")
            .to_string();

        println!("    {} ({:.0}ms, {} tokens)", "done".green(), latency_ms, tokens);

        results.push(ProblemResult {
            index: i + 1,
            prompt: prompt.to_string(),
            response: text,
            latency_ms,
            tokens,
        });
    }

    let avg_latency = if results.is_empty() {
        0.0
    } else {
        total_latency_ms as f64 / results.len() as f64
    };
    let tps = if total_latency_ms > 0 {
        total_tokens as f64 / (total_latency_ms as f64 / 1000.0)
    } else {
        0.0
    };

    let summary = Summary {
        total_problems: results.len(),
        avg_latency_ms: avg_latency,
        total_tokens,
        tokens_per_second: tps,
    };

    let bench_result = BenchmarkResult {
        model: model.to_string(),
        variant: variant.to_string(),
        mlx_url: mlx_url.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        problems: results,
        summary,
    };

    std::fs::create_dir_all(output_dir)?;
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("bench_{}_{}_{}.json", variant, ts, model.replace('/', "_"));
    let path = Path::new(output_dir).join(&filename);
    std::fs::write(&path, serde_json::to_string_pretty(&bench_result)?)?;

    println!();
    println!("{}", "=== Summary ===".bold().green());
    println!("  Problems:    {}", bench_result.summary.total_problems);
    println!("  Avg latency: {:.0}ms", bench_result.summary.avg_latency_ms);
    println!("  Total tokens: {}", bench_result.summary.total_tokens);
    println!(
        "  Tokens/sec:  {:.1}",
        bench_result.summary.tokens_per_second
    );
    println!();
    println!("  Results written to: {}", path.display());
    Ok(())
}

async fn send_completion(
    client: &reqwest::Client,
    mlx_url: &str,
    model: &str,
    prompt: &str,
) -> Result<serde_json::Value> {
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 256,
        "temperature": 0.0,
    });

    let resp = client
        .post(format!("{}/chat/completions", mlx_url))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("API error {}: {}", status, text));
    }

    Ok(resp.json().await?)
}
