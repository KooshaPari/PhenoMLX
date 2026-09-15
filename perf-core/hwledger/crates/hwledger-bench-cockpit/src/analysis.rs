use std::collections::HashMap;

use crate::domain::{Cell, LintWarning, SuiteCoverageRow, KNOWN_SUITE_CATALOG};

pub fn build_suite_coverage(cells: &[Cell]) -> Vec<SuiteCoverageRow> {
    let mut by_suite: HashMap<&str, HashMap<&str, usize>> = HashMap::new();
    for c in cells {
        by_suite
            .entry(c.suite.as_str())
            .or_default()
            .entry(c.variant.as_str())
            .and_modify(|n| *n += 1)
            .or_insert(1);
    }

    let mut order: Vec<&str> = KNOWN_SUITE_CATALOG.iter().copied().collect();
    for &s in by_suite.keys() {
        if !order.contains(&s) {
            order.push(s);
        }
    }

    order
        .into_iter()
        .map(|suite| {
            let variants = by_suite.get(suite).cloned().unwrap_or_default();
            let n: usize = variants.values().sum();
            let arms: Vec<String> = variants
                .iter()
                .filter(|&(v, _)| *v != "stock" && *v != "ours")
                .map(|(v, _)| v.to_string())
                .collect();
            SuiteCoverageRow {
                suite: suite.to_string(),
                present: n > 0,
                n_cells: n,
                has_stock: variants.get("stock").copied().unwrap_or(0) > 0,
                has_ours: variants.get("ours").copied().unwrap_or(0) > 0,
                variants: variants
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
                experiment_arms: arms,
            }
        })
        .collect()
}

pub fn lint_cells(cells: &[Cell]) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    // Rule 1: degenerate pass@1 == 1.0 with timing/content anomalies
    let trivial: Vec<String> = cells
        .iter()
        .filter(|c| c.pass_at_1 >= 0.999)
        .filter(|c| {
            let no_io = c.prompt.is_empty() && c.reply.is_empty();
            let no_tokens = c.total_tokens_in + c.total_tokens_out == 0;
            let fast = c.wall_clock_s < 0.05;
            fast || (no_io && no_tokens && c.wall_clock_s < 1.0)
        })
        .map(|c| format!("{}/{}/{}", c.suite, c.task_id, c.variant))
        .collect();
    if !trivial.is_empty() {
        warnings.push(LintWarning {
            code: "degenerate_cell".into(),
            severity: "error".into(),
            message: format!(
                "{} cell(s) scored 100% with degenerate signals — likely vacuous passes",
                trivial.len()
            ),
            cells: trivial,
        });
    }

    // Rule 2: all stock+ours pass@1 == 1.0 → unscored placeholder
    let mut by_key: HashMap<(String, String), Vec<&Cell>> = HashMap::new();
    for c in cells {
        by_key
            .entry((c.suite.clone(), c.task_id.clone()))
            .or_default()
            .push(c);
    }
    let all_pass: Vec<String> = by_key
        .iter()
        .filter(|(_, group)| {
            let peers: Vec<&&Cell> = group
                .iter()
                .filter(|c| c.variant == "stock" || c.variant == "ours")
                .collect();
            peers.len() >= 2 && peers.iter().all(|c| c.pass_at_1 >= 0.999)
        })
        .map(|((s, t), _)| format!("{}/{}", s, t))
        .collect();
    if !all_pass.is_empty() {
        warnings.push(LintWarning {
            code: "all_variants_pass".into(),
            severity: "warning".into(),
            message: format!(
                "{} task(s) scored 100% across stock+ours — likely unscored placeholder fixture",
                all_pass.len()
            ),
            cells: all_pass,
        });
    }

    // Rule 3: missing judge score
    let no_judge: Vec<String> = cells
        .iter()
        .filter(|c| c.pass_at_1 >= 0.999 && c.judge_score == 0.0)
        .filter(|c| {
            !(c.metadata.get("judge").map(|s| s.as_str()) == Some("deterministic")
                || c.metadata.get("judge").map(|s| s.as_str()) == Some("exact"))
        })
        .map(|c| format!("{}/{}/{}", c.suite, c.task_id, c.variant))
        .collect();
    if !no_judge.is_empty() {
        warnings.push(LintWarning {
            code: "missing_judge_score".into(),
            severity: "warning".into(),
            message: format!(
                "{} cell(s) have pass@1==1.0 with judge_score==0 — likely not actually scored",
                no_judge.len()
            ),
            cells: no_judge,
        });
    }

    // Rule 4: vacuous pass (empty expected_answer + scoring_method)
    let vacuous: Vec<String> = cells
        .iter()
        .filter(|c| c.pass_at_1 >= 0.999)
        .filter(|c| c.expected_answer.is_empty() && c.scoring_method.is_empty())
        .map(|c| format!("{}/{}/{}", c.suite, c.task_id, c.variant))
        .collect();
    if !vacuous.is_empty() {
        warnings.push(LintWarning {
            code: "vacuous_pass".into(),
            severity: "error".into(),
            message: format!(
                "{} cell(s) scored 100% with empty expected_answer and scoring_method — vacuous pass",
                vacuous.len()
            ),
            cells: vacuous,
        });
    }

    // Rule 5: synthetic evidence
    let synthetic: Vec<String> = cells
        .iter()
        .filter(|c| c.metadata.get("synthetic").map(|s| s.as_str()) == Some("true"))
        .take(20)
        .map(|c| format!("{}/{}/{}", c.suite, c.task_id, c.variant))
        .collect();
    let syn_count = cells
        .iter()
        .filter(|c| c.metadata.get("synthetic").map(|s| s.as_str()) == Some("true"))
        .count();
    if syn_count > 0 {
        let sev = if syn_count == cells.len() {
            "error"
        } else {
            "warning"
        };
        let msg = if syn_count == cells.len() {
            format!(
                "ALL {} cells are synthetic=true — treat dashboard scores as reported/synthetic smoke",
                syn_count
            )
        } else {
            format!(
                "{}/{} cell(s) marked synthetic=true — pass@1 is not live-suite promotion proof",
                syn_count,
                cells.len()
            )
        };
        warnings.push(LintWarning {
            code: "synthetic_100pct".into(),
            severity: sev.into(),
            message: msg,
            cells: synthetic,
        });
    }

    warnings
}
