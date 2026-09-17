use super::*;
use agileplus_domain::domain::feature::Feature;
use agileplus_domain::domain::governance::{
    Evidence, EvidenceRequirement, EvidenceType, GovernanceContract, GovernanceRule, PolicyCheck,
    PolicyDefinition, PolicyDomain, PolicyRule,
};
use agileplus_domain::domain::work_package::WorkPackage;
use agileplus_domain::ports::StoragePort;
use agileplus_sqlite::SqliteStorageAdapter;
use chrono::Utc;

#[allow(dead_code)] // test helper - used by multiple #[test] functions
fn make_contract(feature_id: i64) -> GovernanceContract {
    GovernanceContract {
        id: 1,
        feature_id,
        version: 1,
        rules: vec![GovernanceRule {
            transition: "Implementing -> Validated".to_string(),
            required_evidence: vec![EvidenceRequirement {
                fr_id: "FR-001".to_string(),
                evidence_type: EvidenceType::CiOutput,
                threshold: None,
            }],
            policy_refs: vec![],
        }],
        bound_at: Utc::now(),
    }
}

#[test]
fn report_to_markdown_pass() {
    let report = ValidationReport {
        feature_slug: "my-feat".to_string(),
        timestamp: Utc::now(),
        overall_pass: true,
        evidence_results: vec![EvidenceCheck {
            fr_id: "FR-001".to_string(),
            evidence_type: "CiOutput".to_string(),
            found: true,
            threshold_met: true,
            message: "OK".to_string(),
        }],
        policy_results: vec![],
        missing_evidence: vec![],
        governance_exceptions: vec![],
    };
    let md = report.to_markdown();
    assert!(md.contains("PASS"));
    assert!(md.contains("FR-001"));
}

#[test]
fn report_to_markdown_fail_missing_evidence() {
    let report = ValidationReport {
        feature_slug: "my-feat".to_string(),
        timestamp: Utc::now(),
        overall_pass: false,
        evidence_results: vec![EvidenceCheck {
            fr_id: "FR-001".to_string(),
            evidence_type: "CiOutput".to_string(),
            found: false,
            threshold_met: false,
            message: "No evidence found for FR `FR-001`".to_string(),
        }],
        policy_results: vec![],
        missing_evidence: vec![("FR-001".to_string(), "CiOutput".to_string())],
        governance_exceptions: vec![],
    };
    let md = report.to_markdown();
    assert!(md.contains("FAIL"));
    assert!(md.contains("Missing Evidence"));
}

#[test]
fn report_to_json_has_required_fields() {
    let report = ValidationReport {
        feature_slug: "feat".to_string(),
        timestamp: Utc::now(),
        overall_pass: true,
        evidence_results: vec![],
        policy_results: vec![],
        missing_evidence: vec![],
        governance_exceptions: vec![],
    };
    let json = report.to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["feature_slug"], "feat");
    assert_eq!(v["overall_pass"], true);
}

#[test]
fn evaluate_threshold_min_coverage_pass() {
    use agileplus_domain::domain::governance::Evidence;
    let ev = Evidence {
        id: 1,
        wp_id: 1,
        fr_id: "FR-001".to_string(),
        evidence_type: EvidenceType::TestResult,
        artifact_path: "ci.log".to_string(),
        metadata: Some(serde_json::json!({"coverage": 85.0})),
        created_at: Utc::now(),
    };
    let threshold = serde_json::json!({"min_coverage": 80.0});
    assert!(super::evidence::evaluate_threshold(&[&ev], &threshold));
}

#[test]
fn evaluate_threshold_min_coverage_fail() {
    use agileplus_domain::domain::governance::Evidence;
    let ev = Evidence {
        id: 1,
        wp_id: 1,
        fr_id: "FR-001".to_string(),
        evidence_type: EvidenceType::TestResult,
        artifact_path: "ci.log".to_string(),
        metadata: Some(serde_json::json!({"coverage": 60.0})),
        created_at: Utc::now(),
    };
    let threshold = serde_json::json!({"min_coverage": 80.0});
    assert!(!super::evidence::evaluate_threshold(&[&ev], &threshold));
}

#[test]
fn evaluate_threshold_max_critical_pass() {
    use agileplus_domain::domain::governance::Evidence;
    let ev = Evidence {
        id: 1,
        wp_id: 1,
        fr_id: "FR-SEC".to_string(),
        evidence_type: EvidenceType::SecurityScan,
        artifact_path: "scan.json".to_string(),
        metadata: Some(serde_json::json!({"critical_count": 0})),
        created_at: Utc::now(),
    };
    let threshold = serde_json::json!({"max_critical": 0});
    assert!(super::evidence::evaluate_threshold(&[&ev], &threshold));
}

#[test]
fn evaluate_threshold_max_critical_fail() {
    use agileplus_domain::domain::governance::Evidence;
    let ev = Evidence {
        id: 1,
        wp_id: 1,
        fr_id: "FR-SEC".to_string(),
        evidence_type: EvidenceType::SecurityScan,
        artifact_path: "scan.json".to_string(),
        metadata: Some(serde_json::json!({"critical_count": 3})),
        created_at: Utc::now(),
    };
    let threshold = serde_json::json!({"max_critical": 0});
    assert!(!super::evidence::evaluate_threshold(&[&ev], &threshold));
}

#[tokio::test]
async fn builtin_ci_policy_fails_without_matching_evidence() {
    let db = SqliteStorageAdapter::in_memory().unwrap();
    let feature_id = create_feature_with_wp(&db).await.0;
    let contract = contract_with_policy(
        feature_id,
        EvidenceType::CiOutput,
        "FR-CI",
        "policy:ci-required",
    );

    let results = super::evidence::evaluate_policies(&db, &contract, feature_id)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert!(results[0].message.contains("missing evidence"));
}

#[tokio::test]
async fn builtin_ci_policy_ignores_wrong_evidence_type() {
    let db = SqliteStorageAdapter::in_memory().unwrap();
    let (feature_id, wp_id) = create_feature_with_wp(&db).await;
    let contract = contract_with_policy(
        feature_id,
        EvidenceType::CiOutput,
        "FR-CI",
        "policy:ci-required",
    );
    create_evidence(&db, wp_id, "FR-CI", EvidenceType::ReviewApproval).await;

    let results = super::evidence::evaluate_policies(&db, &contract, feature_id)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(!results[0].passed);
    assert!(results[0].message.contains("missing evidence"));
}

#[tokio::test]
async fn builtin_ci_policy_passes_with_matching_evidence() {
    let db = SqliteStorageAdapter::in_memory().unwrap();
    let (feature_id, wp_id) = create_feature_with_wp(&db).await;
    let contract = contract_with_policy(
        feature_id,
        EvidenceType::CiOutput,
        "FR-CI",
        "policy:ci-required",
    );
    create_evidence(&db, wp_id, "FR-CI", EvidenceType::CiOutput).await;

    let results = super::evidence::evaluate_policies(&db, &contract, feature_id)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].passed);
    assert!(results[0].message.contains("satisfied"));
}

#[tokio::test]
async fn active_policy_matches_generated_ci_ref() {
    let db = SqliteStorageAdapter::in_memory().unwrap();
    let (feature_id, wp_id) = create_feature_with_wp(&db).await;
    let policy_id = create_policy_rule(
        &db,
        PolicyDomain::Quality,
        PolicyCheck::EvidencePresent {
            evidence_type: EvidenceType::CiOutput,
        },
    )
    .await;
    let contract = contract_with_policy(
        feature_id,
        EvidenceType::CiOutput,
        "FR-CI",
        "policy:ci-required",
    );
    create_evidence(&db, wp_id, "FR-CI", EvidenceType::CiOutput).await;

    let results = super::evidence::evaluate_policies(&db, &contract, feature_id)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].policy_id, policy_id);
    assert!(results[0].passed);
}

#[tokio::test]
async fn empty_policy_refs_do_not_evaluate_active_policies() {
    let db = SqliteStorageAdapter::in_memory().unwrap();
    let feature_id = create_feature_with_wp(&db).await.0;
    create_policy_rule(&db, PolicyDomain::Compliance, PolicyCheck::ManualApproval).await;
    let contract = make_contract(feature_id);

    let results = super::evidence::evaluate_policies(&db, &contract, feature_id)
        .await
        .unwrap();

    assert!(results.is_empty());
}

async fn create_feature_with_wp(db: &SqliteStorageAdapter) -> (i64, i64) {
    let feature_id = StoragePort::create_feature(
        db,
        &Feature::new(
            "validate-policy-test",
            "Validate Policy Test",
            [0u8; 32],
            None,
        ),
    )
    .await
    .unwrap();
    let wp_id =
        StoragePort::create_work_package(db, &WorkPackage::new(feature_id, "WP", 1, "criteria"))
            .await
            .unwrap();
    (feature_id, wp_id)
}

async fn create_evidence(
    db: &SqliteStorageAdapter,
    wp_id: i64,
    fr_id: &str,
    evidence_type: EvidenceType,
) {
    let evidence = Evidence {
        id: 0,
        wp_id,
        fr_id: fr_id.to_string(),
        evidence_type,
        artifact_path: "artifact.txt".to_string(),
        metadata: None,
        created_at: Utc::now(),
    };
    StoragePort::create_evidence(db, &evidence).await.unwrap();
}

async fn create_policy_rule(
    db: &SqliteStorageAdapter,
    domain: PolicyDomain,
    check: PolicyCheck,
) -> i64 {
    let now = Utc::now();
    let rule = PolicyRule {
        id: 0,
        domain,
        rule: PolicyDefinition {
            description: "test policy".to_string(),
            check,
        },
        active: true,
        created_at: now,
        updated_at: now,
    };
    StoragePort::create_policy_rule(db, &rule).await.unwrap()
}

fn contract_with_policy(
    feature_id: i64,
    evidence_type: EvidenceType,
    fr_id: &str,
    policy_ref: &str,
) -> GovernanceContract {
    GovernanceContract {
        id: 1,
        feature_id,
        version: 1,
        rules: vec![GovernanceRule {
            transition: "Implementing -> Validated".to_string(),
            required_evidence: vec![EvidenceRequirement {
                fr_id: fr_id.to_string(),
                evidence_type,
                threshold: None,
            }],
            policy_refs: vec![policy_ref.to_string()],
        }],
        bound_at: Utc::now(),
    }
}
