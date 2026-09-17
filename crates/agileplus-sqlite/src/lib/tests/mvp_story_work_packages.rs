use super::*;
use agileplus_domain::domain::{
    epic::Epic,
    project::Project,
    story::Story,
    work_package::{DependencyType, WorkPackage, WpDependency, WpState},
};

async fn make_story(db: &SqliteStorageAdapter) -> i64 {
    let project_id = StoragePort::create_project(
        db,
        &Project::new("MVP Project", "mvp-project").expect("project"),
    )
    .await
    .expect("create project");
    let epic_id = StoragePort::create_epic(
        db,
        &Epic::new(project_id, "Render Foundation").expect("epic"),
    )
    .await
    .expect("create epic");
    StoragePort::create_story(
        db,
        &Story::new(epic_id, project_id, "Shader fallback", Some(5)).expect("story"),
    )
    .await
    .expect("create story")
}

fn scoped_wp(title: &str, seq: i32, files: &[&str]) -> WorkPackage {
    let mut wp = WorkPackage::new(0, title, seq, "acceptance");
    wp.file_scope = files.iter().map(|file| file.to_string()).collect();
    wp
}

#[tokio::test]
async fn story_work_package_create_lists_by_story() {
    let db = make_adapter();
    let story_id = make_story(&db).await;

    let wp_id = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("VertexLit chain", 1, &["Core.cs", "VoxelRender.cs"]),
    )
    .await
    .unwrap();

    let wps = StoragePort::list_wps_by_story(&db, story_id).await.unwrap();
    assert_eq!(wps.len(), 1);
    assert_eq!(wps[0].id, wp_id);
    assert_eq!(wps[0].title, "VertexLit chain");
    assert_eq!(wps[0].file_scope, vec!["Core.cs", "VoxelRender.cs"]);
}

#[tokio::test]
async fn story_work_package_dependencies_are_persisted() {
    let db = make_adapter();
    let story_id = make_story(&db).await;
    let wp1 = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("VertexLit chain", 1, &["Core.cs"]),
    )
    .await
    .unwrap();
    let wp2 = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("wp2", 2, &["Core.cs"]),
    )
    .await
    .unwrap();

    StoragePort::add_wp_dependency(
        &db,
        &WpDependency {
            wp_id: wp2,
            depends_on: wp1,
            dep_type: DependencyType::FileOverlap,
        },
    )
    .await
    .unwrap();

    let deps = StoragePort::get_wp_dependencies(&db, wp2).await.unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].depends_on, wp1);
    assert_eq!(deps[0].dep_type, DependencyType::FileOverlap);
}

#[tokio::test]
async fn work_package_transition_persists_state() {
    let db = make_adapter();
    let story_id = make_story(&db).await;
    let wp_id = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("Transition target", 1, &["Core.cs"]),
    )
    .await
    .unwrap();

    StoragePort::update_wp_state(&db, wp_id, WpState::Doing)
        .await
        .unwrap();

    let wp = StoragePort::get_work_package(&db, wp_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wp.state, WpState::Doing);
}

#[tokio::test]
async fn next_ready_excludes_unsatisfied_dependency_and_file_overlap() {
    let db = make_adapter();
    let story_id = make_story(&db).await;

    let in_flight = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("in flight", 1, &["Core.cs"]),
    )
    .await
    .unwrap();
    let overlap_blocked = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("overlap blocked", 2, &["Core.cs"]),
    )
    .await
    .unwrap();
    let dep_blocker = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("dep blocker", 3, &["VoxelRender.cs"]),
    )
    .await
    .unwrap();
    let dep_blocked = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("dep blocked", 4, &["Lighting.cs"]),
    )
    .await
    .unwrap();
    let ready = StoragePort::create_work_package_for_story(
        &db,
        story_id,
        &scoped_wp("ready", 5, &["Sky.cs"]),
    )
    .await
    .unwrap();

    StoragePort::add_wp_dependency(
        &db,
        &WpDependency {
            wp_id: dep_blocked,
            depends_on: dep_blocker,
            dep_type: DependencyType::Explicit,
        },
    )
    .await
    .unwrap();
    StoragePort::update_wp_state(&db, in_flight, WpState::Doing)
        .await
        .unwrap();

    let next_ready = StoragePort::get_next_ready_wps(&db, None).await.unwrap();
    let ids = next_ready.iter().map(|wp| wp.id).collect::<Vec<_>>();

    assert!(ids.contains(&dep_blocker));
    assert!(ids.contains(&ready));
    assert!(!ids.contains(&in_flight));
    assert!(!ids.contains(&overlap_blocked));
    assert!(!ids.contains(&dep_blocked));
}
