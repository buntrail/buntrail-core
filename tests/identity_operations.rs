use buntrail_core::{contracts::*, operations::state_machine::OperationStateMachine};
fn target(id: u8) -> TargetIdentity {
    TargetIdentity {
        security_binding: b"verified-host-key".to_vec(),
        effective_user: b"1000".to_vec(),
        filesystem_namespace: b"root".to_vec(),
        container: Some(ContainerIdentity {
            engine_endpoint: b"unix:/run/podman.sock".to_vec(),
            immutable_id: vec![id],
        }),
    }
}
#[test]
#[allow(non_snake_case)]
fn testEnvironmentIdentitySeparation() {
    let locator = EnvironmentLocator {
        route: RouteKind::LocalContainer,
        ssh_profile: None,
        engine: Some(ContainerEngine::Podman),
        engine_context: Some("default".into()),
        container_selector: Some("editor".into()),
        effective_user: Some("1000".into()),
    };
    let original = (
        locator.clone(),
        target(1),
        RuntimeGeneration {
            supervisor_boot: [1; 16],
            container_start: Some(vec![1]),
        },
    );
    let same_name_new_container = (locator.clone(), target(2), original.2.clone());
    let same_container_restart = (
        locator,
        target(1),
        RuntimeGeneration {
            supervisor_boot: [2; 16],
            container_start: Some(vec![2]),
        },
    );
    assert_eq!(same_name_new_container.0, original.0);
    assert_ne!(same_name_new_container.1, original.1);
    assert_eq!(same_container_restart.1, original.1);
    assert_ne!(same_container_restart.2, original.2);
    let lossy_a = TargetPath::posix(vec![b'/', 0x80]).unwrap();
    let lossy_b = TargetPath::posix(vec![b'/', 0x81]).unwrap();
    assert_eq!(lossy_a.display(), lossy_b.display());
    assert_ne!(lossy_a, lossy_b);
    assert!(TargetPath::posix(b"relative".to_vec()).is_err());
    assert!(TargetPath::posix(b"/bad\0path".to_vec()).is_err());
}
#[test]
#[allow(non_snake_case)]
fn testUnknownIsNotRetryable() {
    let mut machine = OperationStateMachine::new();
    assert_eq!(
        machine.transition(OperationEvent::Started).unwrap(),
        OperationState::Running
    );
    let evidence = ReconciliationEvidence {
        target: target(1),
        observed: ObservedState::Unobserved,
        explanation: "lost mutation acknowledgement".into(),
    };
    let state = machine
        .transition(OperationEvent::LostAcknowledgement(evidence))
        .unwrap();
    assert!(matches!(state, OperationState::Unknown(_)));
    assert!(!state.allows_automatic_replay());
    assert!(machine.transition(OperationEvent::Started).is_err());
    let result = MutationResult::TerminalOpened {
        session: TerminalSessionID([7; 16]),
        runtime: RuntimeGeneration {
            supervisor_boot: [9; 16],
            container_start: None,
        },
    };
    assert!(
        machine
            .transition(OperationEvent::Confirmed(result.clone()))
            .is_err()
    );
    assert_eq!(
        machine
            .transition(OperationEvent::Reconciled(result.clone()))
            .unwrap(),
        OperationState::Confirmed(result)
    );
    assert!(machine.transition(OperationEvent::Started).is_err());
}
#[test]
fn typed_ranges_reject_split_scalars_surrogates_and_stale_revisions() {
    let snapshot = DocumentSnapshot {
        document: DocumentID([1; 16]),
        revision: BufferRevision(7),
        text: std::sync::Arc::new("a😀e\u{301}\r\n".into()),
        encoding: Encoding::Utf8 { bom: false },
        line_endings: LineEndings::CrLf,
        language: None,
    };
    let range = BufferRange {
        revision: BufferRevision(7),
        start: 1,
        end: 3,
    };
    let bytes = range.to_bytes(&snapshot).unwrap();
    assert_eq!((bytes.start, bytes.end), (1, 5));
    assert_eq!(bytes.to_utf16(&snapshot).unwrap(), range);
    assert!(
        BufferRange {
            end: 2,
            ..range.clone()
        }
        .to_bytes(&snapshot)
        .is_err()
    );
    assert!(
        TextByteRange { end: 2, ..bytes }
            .to_utf16(&snapshot)
            .is_err()
    );
    assert!(
        BufferRange {
            revision: BufferRevision(6),
            ..range
        }
        .to_bytes(&snapshot)
        .is_err()
    );
}
