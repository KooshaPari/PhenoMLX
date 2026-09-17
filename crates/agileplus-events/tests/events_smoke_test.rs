//! Integration smoke tests for agileplus_events::DomainEvent and
//! agileplus_events::EventEnvelope.

use agileplus_events::{
    AggregateId, DomainEvent, EpicCreated, EpicStatusChanged, EventEnvelope, FeatureCreated,
    FeatureShipped, FeatureStateAdvanced, ProjectArchived, ProjectCreated, ProjectRenamed,
    StoryAssigned, StoryCreated, StoryStatusChanged, UserAdded, UserRoleChanged, UserStatusChanged,
    WorkPackageCreated, WorkPackageStateChanged,
};
use agileplus_domain::domain::epic::EpicStatus;
use agileplus_domain::domain::state_machine::FeatureState;
use agileplus_domain::domain::story::StoryStatus;
use agileplus_domain::domain::user::{UserRole, UserStatus};
use agileplus_domain::domain::work_package::WpState;
use uuid::Uuid;

fn assert_envelope_round_trip(original: &EventEnvelope) -> EventEnvelope {
    let json = serde_json::to_string(original).expect("serialize envelope");
    let decoded: EventEnvelope = serde_json::from_str(&json).expect("deserialize envelope");
    assert_eq!(decoded.id, original.id, "envelope id must survive round-trip");
    assert_eq!(decoded.aggregate_id, original.aggregate_id);
    assert_eq!(decoded.aggregate_type, original.aggregate_type);
    assert_eq!(decoded.occurred_at, original.occurred_at);
    assert_eq!(decoded.causation_id, original.causation_id);
    assert_eq!(decoded.correlation_id, original.correlation_id);
    decoded
}

#[test]
fn project_created_construction() {
    let event = DomainEvent::ProjectCreated(ProjectCreated {
        project_id: AggregateId(1),
        slug: "my-project".into(),
        name: "My Project".into(),
    });
    assert_eq!(event.event_type(), "project.created");
    assert_eq!(event.aggregate_type(), "Project");
}

#[test]
fn epic_created_construction() {
    let event = DomainEvent::EpicCreated(EpicCreated {
        epic_id: AggregateId(2),
        project_id: AggregateId(1),
        title: "Onboarding".into(),
    });
    assert_eq!(event.event_type(), "epic.created");
    assert_eq!(event.aggregate_type(), "Epic");
}

#[test]
fn feature_shipped_construction() {
    let event = DomainEvent::FeatureShipped(FeatureShipped {
        feature_id: AggregateId(99),
        slug: "shipped-feat".into(),
    });
    assert_eq!(event.event_type(), "feature.shipped");
    assert_eq!(event.aggregate_type(), "Feature");
}

#[test]
fn envelope_wraps_project_created() {
    let project_id = AggregateId(5);
    let payload = DomainEvent::ProjectCreated(ProjectCreated {
        project_id,
        slug: "test-proj".into(),
        name: "Test Project".into(),
    });

    let envelope = EventEnvelope::new(project_id, payload);

    assert_eq!(envelope.aggregate_id, project_id);
    assert_eq!(envelope.aggregate_type, "Project");
    assert!(!envelope.id.is_nil(), "UUIDs should be generated");
    assert!(envelope.causation_id.is_none());
    assert!(envelope.correlation_id.is_none());
}

#[test]
fn envelope_with_causation_and_correlation() {
    let payload = DomainEvent::ProjectCreated(ProjectCreated {
        project_id: AggregateId(12),
        slug: "corr-test".into(),
        name: "Correlation Test".into(),
    });

    let cause = Uuid::new_v4();
    let corr = Uuid::new_v4();
    let envelope = EventEnvelope::new(AggregateId(12), payload)
        .with_causation(cause)
        .with_correlation(corr);

    assert_eq!(envelope.causation_id, Some(cause));
    assert_eq!(envelope.correlation_id, Some(corr));
}

#[test]
fn envelope_ids_are_unique_per_construction() {
    let payload = DomainEvent::ProjectCreated(ProjectCreated {
        project_id: AggregateId(1),
        slug: "x".into(),
        name: "X".into(),
    });
    let a = EventEnvelope::new(AggregateId(1), payload.clone());
    let b = EventEnvelope::new(AggregateId(1), payload);
    assert_ne!(a.id, b.id, "each envelope must get a fresh UUID");
}

#[test]
fn round_trip_project_created() {
    let project_id = AggregateId(7);
    let payload = DomainEvent::ProjectCreated(ProjectCreated {
        project_id,
        slug: "round-trip".into(),
        name: "Round Trip".into(),
    });
    let envelope = EventEnvelope::new(project_id, payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::ProjectCreated(p) => {
            assert_eq!(p.project_id, project_id);
            assert_eq!(p.slug, "round-trip");
            assert_eq!(p.name, "Round Trip");
        }
        other => panic!("expected ProjectCreated, got {other:?}"),
    }
}

#[test]
fn round_trip_project_renamed() {
    let payload = DomainEvent::ProjectRenamed(ProjectRenamed {
        project_id: AggregateId(1),
        old_name: "Old".into(),
        new_name: "New".into(),
    });
    let envelope = EventEnvelope::new(AggregateId(1), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::ProjectRenamed(p) => {
            assert_eq!(p.old_name, "Old");
            assert_eq!(p.new_name, "New");
        }
        other => panic!("expected ProjectRenamed, got {other:?}"),
    }
}

#[test]
fn round_trip_project_archived() {
    let payload = DomainEvent::ProjectArchived(ProjectArchived {
        project_id: AggregateId(1),
    });
    let envelope = EventEnvelope::new(AggregateId(1), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    assert!(matches!(decoded.payload, DomainEvent::ProjectArchived(_)));
}

#[test]
fn round_trip_epic_created() {
    let payload = DomainEvent::EpicCreated(EpicCreated {
        epic_id: AggregateId(2),
        project_id: AggregateId(1),
        title: "Onboarding".into(),
    });
    let envelope = EventEnvelope::new(AggregateId(2), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::EpicCreated(e) => assert_eq!(e.title, "Onboarding"),
        other => panic!("expected EpicCreated, got {other:?}"),
    }
}

#[test]
fn round_trip_epic_status_changed() {
    let payload = DomainEvent::EpicStatusChanged(EpicStatusChanged {
        epic_id: AggregateId(2),
        project_id: AggregateId(1),
        from: EpicStatus::Backlog,
        to: EpicStatus::Active,
    });
    let envelope = EventEnvelope::new(AggregateId(2), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::EpicStatusChanged(e) => {
            assert_eq!(e.from, EpicStatus::Backlog);
            assert_eq!(e.to, EpicStatus::Active);
        }
        other => panic!("expected EpicStatusChanged, got {other:?}"),
    }
}

#[test]
fn round_trip_story_created() {
    let payload = DomainEvent::StoryCreated(StoryCreated {
        story_id: AggregateId(42),
        epic_id: AggregateId(2),
        project_id: AggregateId(1),
        title: "Login".into(),
        points: Some(3),
    });
    let envelope = EventEnvelope::new(AggregateId(42), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::StoryCreated(s) => {
            assert_eq!(s.title, "Login");
            assert_eq!(s.points, Some(3));
        }
        other => panic!("expected StoryCreated, got {other:?}"),
    }
}

#[test]
fn round_trip_story_status_changed() {
    let payload = DomainEvent::StoryStatusChanged(StoryStatusChanged {
        story_id: AggregateId(42),
        epic_id: AggregateId(2),
        from: StoryStatus::Todo,
        to: StoryStatus::InProgress,
    });
    let envelope = EventEnvelope::new(AggregateId(42), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::StoryStatusChanged(s) => {
            assert_eq!(s.from, StoryStatus::Todo);
            assert_eq!(s.to, StoryStatus::InProgress);
        }
        other => panic!("expected StoryStatusChanged, got {other:?}"),
    }
}

#[test]
fn round_trip_story_assigned() {
    let payload = DomainEvent::StoryAssigned(StoryAssigned {
        story_id: AggregateId(42),
        assignee_id: Some(AggregateId(5)),
    });
    let envelope = EventEnvelope::new(AggregateId(42), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::StoryAssigned(s) => assert_eq!(s.assignee_id, Some(AggregateId(5))),
        other => panic!("expected StoryAssigned, got {other:?}"),
    }
}

#[test]
fn round_trip_story_unassigned() {
    let payload = DomainEvent::StoryAssigned(StoryAssigned {
        story_id: AggregateId(42),
        assignee_id: None,
    });
    let envelope = EventEnvelope::new(AggregateId(42), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::StoryAssigned(s) => assert_eq!(s.assignee_id, None),
        other => panic!("expected StoryAssigned, got {other:?}"),
    }
}

#[test]
fn round_trip_user_added() {
    let payload = DomainEvent::UserAdded(UserAdded {
        user_id: AggregateId(99),
        display_name: "Alice".into(),
        email: "alice@example.com".into(),
        role: UserRole::Member,
    });
    let envelope = EventEnvelope::new(AggregateId(99), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::UserAdded(u) => {
            assert_eq!(u.display_name, "Alice");
            assert_eq!(u.email, "alice@example.com");
            assert_eq!(u.role, UserRole::Member);
        }
        other => panic!("expected UserAdded, got {other:?}"),
    }
}

#[test]
fn round_trip_user_role_changed() {
    let payload = DomainEvent::UserRoleChanged(UserRoleChanged {
        user_id: AggregateId(5),
        old_role: UserRole::Member,
        new_role: UserRole::Admin,
    });
    let envelope = EventEnvelope::new(AggregateId(5), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::UserRoleChanged(u) => {
            assert_eq!(u.old_role, UserRole::Member);
            assert_eq!(u.new_role, UserRole::Admin);
        }
        other => panic!("expected UserRoleChanged, got {other:?}"),
    }
}

#[test]
fn round_trip_user_status_changed() {
    let payload = DomainEvent::UserStatusChanged(UserStatusChanged {
        user_id: AggregateId(5),
        from: UserStatus::Active,
        to: UserStatus::Suspended,
    });
    let envelope = EventEnvelope::new(AggregateId(5), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::UserStatusChanged(u) => {
            assert_eq!(u.from, UserStatus::Active);
            assert_eq!(u.to, UserStatus::Suspended);
        }
        other => panic!("expected UserStatusChanged, got {other:?}"),
    }
}

#[test]
fn round_trip_feature_created() {
    let payload = DomainEvent::FeatureCreated(FeatureCreated {
        feature_id: AggregateId(100),
        slug: "f-100".into(),
        friendly_name: "Friendly".into(),
        project_id: Some(AggregateId(1)),
    });
    let envelope = EventEnvelope::new(AggregateId(100), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::FeatureCreated(f) => {
            assert_eq!(f.slug, "f-100");
            assert_eq!(f.project_id, Some(AggregateId(1)));
        }
        other => panic!("expected FeatureCreated, got {other:?}"),
    }
}

#[test]
fn round_trip_feature_state_advanced() {
    let payload = DomainEvent::FeatureStateAdvanced(FeatureStateAdvanced {
        feature_id: AggregateId(100),
        from: FeatureState::Created,
        to: FeatureState::Specified,
    });
    let envelope = EventEnvelope::new(AggregateId(100), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::FeatureStateAdvanced(f) => {
            assert_eq!(f.from, FeatureState::Created);
            assert_eq!(f.to, FeatureState::Specified);
        }
        other => panic!("expected FeatureStateAdvanced, got {other:?}"),
    }
}

#[test]
fn round_trip_feature_shipped() {
    let payload = DomainEvent::FeatureShipped(FeatureShipped {
        feature_id: AggregateId(100),
        slug: "f-100".into(),
    });
    let envelope = EventEnvelope::new(AggregateId(100), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::FeatureShipped(f) => assert_eq!(f.slug, "f-100"),
        other => panic!("expected FeatureShipped, got {other:?}"),
    }
}

#[test]
fn round_trip_work_package_created() {
    let payload = DomainEvent::WorkPackageCreated(WorkPackageCreated {
        wp_id: AggregateId(20),
        feature_id: AggregateId(100),
        title: "Implement login".into(),
        sequence: 1,
    });
    let envelope = EventEnvelope::new(AggregateId(20), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::WorkPackageCreated(w) => {
            assert_eq!(w.title, "Implement login");
            assert_eq!(w.sequence, 1);
            assert_eq!(w.feature_id, AggregateId(100));
        }
        other => panic!("expected WorkPackageCreated, got {other:?}"),
    }
}

#[test]
fn round_trip_work_package_state_changed() {
    let payload = DomainEvent::WorkPackageStateChanged(WorkPackageStateChanged {
        wp_id: AggregateId(20),
        feature_id: AggregateId(100),
        from: WpState::Planned,
        to: WpState::Doing,
    });
    let envelope = EventEnvelope::new(AggregateId(20), payload);
    let decoded = assert_envelope_round_trip(&envelope);
    match decoded.payload {
        DomainEvent::WorkPackageStateChanged(w) => {
            assert_eq!(w.from, WpState::Planned);
            assert_eq!(w.to, WpState::Doing);
        }
        other => panic!("expected WorkPackageStateChanged, got {other:?}"),
    }
}

#[test]
fn round_trip_all_event_variants_payload_only() {
    let events: Vec<DomainEvent> = vec![
        DomainEvent::ProjectCreated(ProjectCreated {
            project_id: AggregateId(1),
            slug: "p1".into(),
            name: "Project 1".into(),
        }),
        DomainEvent::ProjectRenamed(ProjectRenamed {
            project_id: AggregateId(1),
            old_name: "Old".into(),
            new_name: "New".into(),
        }),
        DomainEvent::ProjectArchived(ProjectArchived {
            project_id: AggregateId(1),
        }),
        DomainEvent::EpicCreated(EpicCreated {
            epic_id: AggregateId(2),
            project_id: AggregateId(1),
            title: "Epic".into(),
        }),
        DomainEvent::EpicStatusChanged(EpicStatusChanged {
            epic_id: AggregateId(2),
            project_id: AggregateId(1),
            from: EpicStatus::Backlog,
            to: EpicStatus::Done,
        }),
        DomainEvent::StoryCreated(StoryCreated {
            story_id: AggregateId(3),
            epic_id: AggregateId(2),
            project_id: AggregateId(1),
            title: "Story".into(),
            points: Some(5),
        }),
        DomainEvent::StoryStatusChanged(StoryStatusChanged {
            story_id: AggregateId(3),
            epic_id: AggregateId(2),
            from: StoryStatus::Todo,
            to: StoryStatus::Done,
        }),
        DomainEvent::StoryAssigned(StoryAssigned {
            story_id: AggregateId(3),
            assignee_id: Some(AggregateId(7)),
        }),
        DomainEvent::UserAdded(UserAdded {
            user_id: AggregateId(4),
            display_name: "Bob".into(),
            email: "bob@example.com".into(),
            role: UserRole::Admin,
        }),
        DomainEvent::UserRoleChanged(UserRoleChanged {
            user_id: AggregateId(4),
            old_role: UserRole::Member,
            new_role: UserRole::Admin,
        }),
        DomainEvent::UserStatusChanged(UserStatusChanged {
            user_id: AggregateId(4),
            from: UserStatus::Active,
            to: UserStatus::Inactive,
        }),
        DomainEvent::FeatureCreated(FeatureCreated {
            feature_id: AggregateId(5),
            slug: "f5".into(),
            friendly_name: "Friendly".into(),
            project_id: None,
        }),
        DomainEvent::FeatureStateAdvanced(FeatureStateAdvanced {
            feature_id: AggregateId(5),
            from: FeatureState::Created,
            to: FeatureState::Shipped,
        }),
        DomainEvent::FeatureShipped(FeatureShipped {
            feature_id: AggregateId(5),
            slug: "f5".into(),
        }),
        DomainEvent::WorkPackageCreated(WorkPackageCreated {
            wp_id: AggregateId(6),
            feature_id: AggregateId(5),
            title: "WP".into(),
            sequence: 1,
        }),
        DomainEvent::WorkPackageStateChanged(WorkPackageStateChanged {
            wp_id: AggregateId(6),
            feature_id: AggregateId(5),
            from: WpState::Doing,
            to: WpState::Done,
        }),
    ];

    assert_eq!(events.len(), 16, "guard: every variant must be exercised");

    for event in events {
        let json = serde_json::to_string(&event).expect("serialize event");
        let decoded: DomainEvent = serde_json::from_str(&json).expect("deserialize event");
        assert_eq!(decoded.event_type(), event.event_type());
        assert_eq!(decoded.aggregate_type(), event.aggregate_type());
    }
}

#[test]
fn envelope_causation_correlation_survives_round_trip() {
    let payload = DomainEvent::ProjectCreated(ProjectCreated {
        project_id: AggregateId(12),
        slug: "corr-test".into(),
        name: "Correlation Test".into(),
    });
    let cause = Uuid::new_v4();
    let corr = Uuid::new_v4();
    let envelope = EventEnvelope::new(AggregateId(12), payload)
        .with_causation(cause)
        .with_correlation(corr);

    let json = serde_json::to_string(&envelope).expect("serialize");
    let decoded: EventEnvelope = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.causation_id, Some(cause));
    assert_eq!(decoded.correlation_id, Some(corr));
}
