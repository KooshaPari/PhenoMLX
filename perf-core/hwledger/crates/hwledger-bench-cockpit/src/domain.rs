use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Meta {
    pub model: String,
    #[serde(default)]
    pub mlx_url: String,
    #[serde(default)]
    pub n_suites: usize,
    #[serde(default)]
    pub n_tasks_per_suite: usize,
    #[serde(default)]
    pub variants: Vec<String>,
    #[serde(default)]
    pub n_cells: usize,
    #[serde(default)]
    pub difficulty_mix: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct VariantSummary {
    #[serde(default)]
    pub n_cells: usize,
    #[serde(default)]
    pub pass_at_1: f64,
    #[serde(default)]
    pub gen_ok: f64,
    #[serde(default)]
    pub verified_pass_at_1: f64,
    #[serde(default)]
    pub mean_wall_clock_s: f64,
    #[serde(default)]
    pub mean_partial_credit: f64,
    #[serde(default)]
    pub mean_format_compliance: f64,
    #[serde(default)]
    pub mean_intent_preservation: f64,
    #[serde(default)]
    pub n_hallucinations: usize,
    #[serde(default)]
    pub ok_count: usize,
    #[serde(default)]
    pub mean_tokens_per_second: f64,
    #[serde(default)]
    pub mean_tokens_read: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Summary {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub by_variant: HashMap<String, VariantSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Cell {
    #[serde(default)]
    pub suite: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub wall_clock_s: f64,
    #[serde(default)]
    pub tokens_per_second: f64,
    #[serde(default)]
    pub first_token_latency_ms: f64,
    #[serde(default)]
    pub peak_rss_mb: f64,
    #[serde(default)]
    pub peak_gpu_mem_mb: f64,
    #[serde(default)]
    pub energy_proxy_joules: f64,
    #[serde(default)]
    pub pass_at_1: f64,
    #[serde(default)]
    pub gen_ok: f64,
    #[serde(default)]
    pub verified_pass_at_1: f64,
    #[serde(default)]
    pub partial_credit: f64,
    #[serde(default)]
    pub judge_score: f64,
    #[serde(default)]
    pub intent_preservation_rate: f64,
    #[serde(default)]
    pub hallucination_count: usize,
    #[serde(default)]
    pub tool_call_success_rate: f64,
    #[serde(default)]
    pub retry_count: usize,
    #[serde(default)]
    pub format_compliance_rate: f64,
    #[serde(default)]
    pub reply: String,
    #[serde(default)]
    pub reply_full: Option<String>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub semantic: HashMap<String, f64>,
    #[serde(default)]
    pub failure_analysis: serde_json::Value,
    #[serde(default)]
    pub progress_trace: Vec<serde_json::Value>,
    #[serde(default)]
    pub chat_trace: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub task_title: Option<String>,
    #[serde(default)]
    pub task_description: Option<String>,
    #[serde(default)]
    pub acceptance: Option<String>,
    #[serde(default)]
    pub rubric: Option<String>,
    #[serde(default)]
    pub assignment: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub completed_at: String,
    #[serde(default)]
    pub model_name: String,
    #[serde(default)]
    pub model_version: String,
    #[serde(default)]
    pub temperature: f64,
    #[serde(default)]
    pub max_tokens: usize,
    #[serde(default)]
    pub top_p: f64,
    #[serde(default)]
    pub seed: usize,
    #[serde(default)]
    pub system_prompt_hash: String,
    #[serde(default)]
    pub task_type: String,
    #[serde(default)]
    pub expected_answer: String,
    #[serde(default)]
    pub scoring_method: String,
    #[serde(default)]
    pub total_tokens_in: usize,
    #[serde(default)]
    pub total_tokens_out: usize,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub error_message: String,
    #[serde(default)]
    pub error_code: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub rlvr_composite: f64,
    #[serde(default)]
    pub rlvr_l0: f64,
    #[serde(default)]
    pub rlvr_l1: f64,
    #[serde(default)]
    pub rlvr_l2: f64,
    #[serde(default)]
    pub rlvr_l3: f64,
    #[serde(default)]
    pub rlvr_reward: f64,
    #[serde(default)]
    pub rlvr_reward_breakdown: Option<HashMap<String, f64>>,
    #[serde(default)]
    pub rlvr_passed: bool,
    #[serde(default)]
    pub rlvr_verifiable: bool,
    #[serde(default)]
    pub rlvr_tournament_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ResultsData {
    pub summary: Summary,
    #[serde(default)]
    pub cells: Vec<Cell>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintWarning {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(default)]
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SuiteCoverageRow {
    pub suite: String,
    pub present: bool,
    #[serde(default)]
    pub variants: HashMap<String, usize>,
    #[serde(default)]
    pub n_cells: usize,
    #[serde(default)]
    pub has_stock: bool,
    #[serde(default)]
    pub has_ours: bool,
    #[serde(default)]
    pub experiment_arms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub json_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_paths: Option<Vec<String>>,
    pub server_ts: String,
    pub data: Option<ResultsData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<LintWarning>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lint_run_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite_coverage: Option<Vec<SuiteCoverageRow>>,
}

pub const KNOWN_SUITE_CATALOG: &[&str] = &[
    "aider-polyglot",
    "aime",
    "arc-agi-2",
    "bfcl",
    "browsercomp",
    "deep-swe",
    "gpqa-diamond",
    "hle",
    "ifeval",
    "kernelbench",
    "livecodebench",
    "mmlu-pro",
    "mt-bench",
    "osworld",
    "perplexity",
    "pinchbench",
    "startup-bench",
    "swe-bench",
    "swe-bench-pro",
    "swe-bench-verified",
    "terminal-bench",
    "vending-bench",
    "ycbench",
];

pub fn summarize_by_variant(cells: &[Cell]) -> HashMap<String, VariantSummary> {
    #[derive(Default)]
    struct Acc {
        n: usize,
        ok: usize,
        pass: f64,
        gen_ok: f64,
        verified_sum: f64,
        verified_n: usize,
        wall: f64,
        partial: f64,
        format: f64,
        intent: f64,
        hall: usize,
        tps: f64,
    }

    let mut map: HashMap<String, Acc> = HashMap::new();
    for c in cells {
        let a = map.entry(c.variant.clone()).or_default();
        a.n += 1;
        if c.ok {
            a.ok += 1;
        }
        a.pass += c.pass_at_1;
        let gen_ok = if c.gen_ok != 0.0 {
            c.gen_ok
        } else {
            c.pass_at_1
        };
        a.gen_ok += gen_ok;
        if c.verified_pass_at_1 > 0.0 {
            a.verified_sum += c.verified_pass_at_1;
            a.verified_n += 1;
        }
        a.wall += c.wall_clock_s;
        a.partial += c.partial_credit;
        a.format += c.format_compliance_rate;
        a.intent += c.intent_preservation_rate;
        a.hall += c.hallucination_count;
        a.tps += c.tokens_per_second;
    }

    map.into_iter()
        .filter(|(_, a)| a.n > 0)
        .map(|(v, a)| {
            let n = a.n as f64;
            let verified_mean = if a.verified_n > 0 {
                a.verified_sum / a.verified_n as f64
            } else {
                0.0
            };
            let tps = a.tps / n;
            (
                v,
                VariantSummary {
                    n_cells: a.n,
                    pass_at_1: a.pass / n,
                    gen_ok: a.gen_ok / n,
                    verified_pass_at_1: verified_mean,
                    mean_wall_clock_s: a.wall / n,
                    mean_partial_credit: a.partial / n,
                    mean_format_compliance: a.format / n,
                    mean_intent_preservation: a.intent / n,
                    n_hallucinations: a.hall,
                    ok_count: a.ok,
                    mean_tokens_per_second: tps,
                    mean_tokens_read: tps,
                },
            )
        })
        .collect()
}
#[allow(dead_code)]
pub fn first_non_empty<'a>(vals: &[&'a str]) -> &'a str {
    for v in vals {
        if !v.is_empty() {
            return v;
        }
    }
    ""
}
