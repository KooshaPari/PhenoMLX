//! `agileplus validate` command implementation.
//!
//! Checks governance compliance for a feature in Implementing state.
//! Transitions to Validated on success.
//! Traceability: FR-005, FR-018, FR-019 / WP13-T073, T074, T077

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;

use agileplus_domain::domain::audit::{AuditEntry, hash_entry};
use agileplus_domain::domain::event::Event;
use agileplus_domain::domain::state_machine::FeatureState;
use agileplus_domain::ports::{StoragePort, VcsPort};
use agileplus_events::{EventStore, compute_hash};

mod evidence;
mod report;

use self::evidence::{evaluate_evidence, evaluate_policies};
pub use report::{EvidenceCheck, PolicyEvalResult, ValidationReport};

/// Arguments for the `validate` subcommand.
#[derive(Debug, clap::Args)]
pub struct ValidateArgs {
    /// Feature slug to validate.
    #[arg(long)]
    pub feature: String,

    /// Output format for validation report (markdown or json).
    #[arg(long, default_value = "markdown")]
    pub format: String,

    /// Skip policy rule evaluation (evidence-only check).
    #[arg(long)]
    pub skip_policies: bool,

    /// Write report to file instead of stdout.
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Force validation even if not in Implementing state (logs governance exception).
    #[arg(long)]
    pub force: bool,
}

/// Run the `validate` command.
pub async fn run_validate<S, V>(args: ValidateArgs, storage: &S, vcs: &V) -> Result<()>
where
    S: StoragePort + EventStore,
    V: VcsPort,
{
    let start = std::time::Instant::now();
    let slug = &args.feature;

    // Look up feature
    let feature = storage
        .get_feature_by_slug(slug)
        .await
        .context("looking up feature")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Feature '{}' not found. Run `agileplus plan --feature {}` first.",
                slug,
                slug
            )
        })?;

    // State enforcement
    let mut governance_exceptions: Vec<String> = Vec::new();
    if feature.state != FeatureState::Implementing {
        if args.force {
            let exc = format!(
                "Force flag used: expected state 'Implementing', got '{}' for feature '{}'",
                feature.state, slug
            );
            eprintln!("Warning: {exc}");
            governance_exceptions.push(exc);
        } else {
            anyhow::bail!(
                "Feature '{}' is in state '{}'. Expected 'Implementing'. \
                Run `agileplus implement --feature {}` first, or use --force.",
                slug,
                feature.state,
                slug
            );
        }
    }

    // Load governance contract
    let contract = storage
        .get_latest_governance_contract(feature.id)
        .await
        .context("loading governance contract")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No governance contract found for feature '{}'. Run `agileplus plan --feature {}` first.",
                slug, slug
            )
        })?;

    // Evaluate evidence
    let (evidence_results, missing_evidence) =
        evaluate_evidence(storage, &contract, feature.id).await?;

    // Evaluate policies (unless skipped)
    let policy_results = if args.skip_policies {
        Vec::new()
    } else {
        evaluate_policies(storage, &contract, feature.id).await?
    };

    // Compute overall pass
    let evidence_pass =
        missing_evidence.is_empty() && evidence_results.iter().all(|e| e.found && e.threshold_met);
    let policy_pass = policy_results.iter().all(|p| p.passed);
    let overall_pass = evidence_pass && policy_pass;

    let report = ValidationReport {
        feature_slug: slug.clone(),
        timestamp: Utc::now(),
        overall_pass,
        evidence_results,
        policy_results,
        missing_evidence,
        governance_exceptions,
    };

    let summary = report.summary();
    println!("Validation summary: {summary}");

    // Format and output the report
    let report_content = match args.format.as_str() {
        "json" => report.to_json(),
        _ => report.to_markdown(),
    };

    if let Some(ref output_path) = args.output {
        std::fs::write(output_path, &report_content)
            .with_context(|| format!("writing report to {}", output_path.display()))?;
        println!("Validation report written to: {}", output_path.display());
    } else {
        print!("{report_content}");
    }

    if !overall_pass {
        anyhow::bail!(
            "Validation FAILED for feature '{}'. Fix the issues above and re-run validate.",
            slug
        );
    }

    // Transition to Validated
    storage
        .update_feature_state(feature.id, FeatureState::Validated)
        .await
        .context("transitioning feature to Validated")?;

    // Append audit entry
    let prev_hash = get_latest_hash(storage, feature.id).await;
    let mut audit = AuditEntry {
        id: 0,
        feature_id: feature.id,
        wp_id: None,
        timestamp: Utc::now(),
        actor: "user".into(),
        transition: "Implementing -> Validated".into(),
        evidence_refs: vec![],
        prev_hash,
        hash: [0u8; 32],
        event_id: None,
        archived_to: None,
    };
    audit.hash = hash_entry(&audit);
    storage
        .append_audit_entry(&audit)
        .await
        .context("appending audit entry")?;

    append_feature_transition_event(storage, feature.id, "Implementing", "Validated", "user")
        .await
        .context("appending state transition event")?;

    // Also write report as artifact
    let report_md = if args.format == "json" {
        report.to_markdown()
    } else {
        report_content.clone()
    };
    vcs.write_artifact(slug, "validation-report.md", &report_md)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(slug = %slug, error = %e, "failed to write validation-report.md artifact");
        });

    let elapsed_ms = start.elapsed().as_millis();
    tracing::info!(
        command = "validate",
        slug = %slug,
        summary = %summary,
        elapsed_ms = %elapsed_ms,
        "validate completed"
    );

    println!("Feature '{}' validated successfully.", slug);
    println!("  State: Implementing -> Validated");
    println!("  Report: kitty-specs/{slug}/validation-report.md");

    Ok(())
}

async fn get_latest_hash<S: StoragePort>(storage: &S, feature_id: i64) -> [u8; 32] {
    match storage.get_latest_audit_entry(feature_id).await {
        Ok(Some(entry)) => entry.hash,
        _ => [0u8; 32],
    }
}

async fn append_feature_transition_event<S: EventStore>(
    storage: &S,
    feature_id: i64,
    from: &str,
    to: &str,
    actor: &str,
) -> Result<()> {
    let prev_hash = storage
        .get_events("feature", feature_id)
        .await
        .map(|events| events.last().map(|event| event.hash).unwrap_or([0u8; 32]))
        .context("loading prior event chain")?;
    let sequence = storage
        .get_latest_sequence("feature", feature_id)
        .await
        .context("loading latest event sequence")?
        + 1;

    let payload = serde_json::json!({
        "from": from,
        "to": to,
    });

    let mut event = Event::new("feature", feature_id, "state_transitioned", payload, actor);
    event.sequence = sequence;
    event.prev_hash = prev_hash;
    event.hash = compute_hash(
        event.entity_id,
        &event.entity_type,
        &event.event_type,
        &event.payload,
        event.timestamp,
        &event.actor,
        &event.prev_hash,
    )
    .context("computing event hash")?;

    storage.append(&event).await.context("persisting event")?;

    tracing::info!(
        entity = "feature",
        feature_id,
        from,
        to,
        sequence,
        hash = ?event.hash,
        "persisted state transition event"
    );

    Ok(())
}

#[cfg(test)]
mod tests;
