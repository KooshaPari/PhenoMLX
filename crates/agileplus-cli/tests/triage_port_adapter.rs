// SPDX-License-Identifier: MIT OR Apache-2.0
// NOTE: triage module is currently STUB (agileplus_triage dep missing).
// Test disabled until triage module is re-enabled.
//
// use agileplus_cli::commands::triage::{TriageArgs, run};
// use agileplus_domain::{
//     domain::backlog::{BacklogItem, BacklogStatus, Intent},
//     ports::ContentStoragePort,
// };
// use agileplus_sqlite::SqliteTriageAdapter;
//
// #[tokio::test]
// async fn triage_command_uses_sqlite_adapter_and_records_dismissal() {
//     let triage = SqliteTriageAdapter::in_memory().unwrap();
//
//     let item = BacklogItem::from_triage(
//         "Crash on login".to_string(),
//         "OAuth callback fails".to_string(),
//         Intent::Bug,
//         "integration-test".to_string(),
//     );
//     let created_id = triage.storage().create_backlog_item(&item).await.unwrap();
//
//     let args = TriageArgs {
//         output: "json".to_string(),
//         outcome: "dismissed".to_string(),
//         peek: false,
//     };
//     let result = run(&args, &triage).await.unwrap();
//
//     assert_eq!(result.ticket.id, created_id.to_string());
//     assert_eq!(result.recorded_outcome, Some(agileplus_domain::ports::TriageOutcome::Dismissed));
//
//     let stored = triage
//         .storage()
//         .get_backlog_item(created_id)
//         .await
//         .unwrap()
//         .unwrap();
//     assert_eq!(stored.status, BacklogStatus::Dismissed);
// }
