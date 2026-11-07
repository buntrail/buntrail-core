use buntrail_core::{contracts::*, terminal::layout::*};
fn tab(n: u8) -> TerminalTabID {
    TerminalTabID([n; 16])
}
fn operation(n: u8) -> OperationID {
    OperationID([n; 16])
}
fn runtime() -> RuntimeGeneration {
    RuntimeGeneration {
        supervisor_boot: [7; 16],
        container_start: None,
    }
}
fn session(n: u8) -> TerminalSessionID {
    TerminalSessionID([n; 16])
}
#[test]
fn tabs_splits_focus_and_close_preserve_unrelated_sessions() {
    let mut layout = TerminalLayoutState::default();
    layout
        .new_tab(tab(1), session(1), operation(1), runtime())
        .unwrap();
    layout
        .split(
            session(1),
            session(2),
            SplitOrientation::Right,
            operation(2),
            runtime(),
        )
        .unwrap();
    layout
        .new_tab(tab(3), session(3), operation(3), runtime())
        .unwrap();
    assert_eq!(layout.tabs.len(), 2);
    assert_eq!(layout.sessions(), vec![session(1), session(2), session(3)]);
    layout.focus(session(2)).unwrap();
    assert_eq!(layout.selected, Some(tab(1)));
    assert_eq!(layout.tabs[0].focused, session(2));
    layout.close(session(2)).unwrap();
    assert_eq!(layout.tabs[0].root, TerminalNode::Pane(session(1)));
    assert_eq!(layout.sessions(), vec![session(1), session(3)]);
    assert!(
        layout
            .split(
                session(1),
                session(3),
                SplitOrientation::Below,
                operation(2),
                runtime()
            )
            .is_err()
    );
    assert!(layout.resize(session(1), f64::NAN).is_err());
}
#[test]
fn independent_nested_splits_keep_ratios_and_reject_unknown_panes() {
    let mut layout = TerminalLayoutState::default();
    layout
        .new_tab(tab(1), session(1), operation(1), runtime())
        .unwrap();
    layout
        .split(
            session(1),
            session(2),
            SplitOrientation::Below,
            operation(2),
            runtime(),
        )
        .unwrap();
    layout
        .split(
            session(2),
            session(3),
            SplitOrientation::Right,
            operation(3),
            runtime(),
        )
        .unwrap();
    layout.resize(session(2), 0.7).unwrap();
    let before = layout.clone();
    assert!(layout.focus(session(9)).is_err());
    assert_eq!(layout, before);
    layout.close(session(1)).unwrap();
    assert_eq!(layout.sessions(), vec![session(2), session(3)]);
    layout.validate().unwrap();
}

#[test]
fn tab_identity_survives_closing_original_pane_and_runtime_tracks_remaining_session() {
    let mut layout = TerminalLayoutState::default();
    layout
        .new_tab(tab(20), session(1), operation(1), runtime())
        .unwrap();
    layout
        .split(
            session(1),
            session(2),
            SplitOrientation::Right,
            operation(2),
            runtime(),
        )
        .unwrap();
    layout.close(session(1)).unwrap();
    assert_eq!(layout.selected, Some(tab(20)));
    assert_eq!(layout.tabs[0].id, tab(20));
    assert_eq!(layout.runtimes, vec![(session(2), operation(2), runtime())]);
    layout.validate().unwrap();
}

#[test]
fn split_ratio_addresses_a_specific_nested_divider() {
    let mut layout = TerminalLayoutState::default();
    layout
        .new_tab(tab(1), session(1), operation(1), runtime())
        .unwrap();
    layout
        .split(
            session(1),
            session(2),
            SplitOrientation::Right,
            operation(2),
            runtime(),
        )
        .unwrap();
    layout
        .split(
            session(1),
            session(3),
            SplitOrientation::Below,
            operation(3),
            runtime(),
        )
        .unwrap();
    layout
        .split(
            session(2),
            session(4),
            SplitOrientation::Below,
            operation(4),
            runtime(),
        )
        .unwrap();
    layout.resize_at(tab(1), 0, 0, 0.7).unwrap();
    layout.resize_at(tab(1), 0, 1, 0.3).unwrap();
    let TerminalNode::Split { ratio, first, .. } = &layout.tabs[0].root else {
        panic!()
    };
    assert_eq!(*ratio, 0.7);
    let TerminalNode::Split { ratio, .. } = first.as_ref() else {
        panic!()
    };
    assert_eq!(*ratio, 0.3);
    assert!(layout.resize_at(tab(1), 0, 2, 0.5).is_err());
}

#[test]
fn identity_capacity_rejects_new_work_without_evicting_retired_ids() {
    let mut layout = TerminalLayoutState::default();
    for n in 0u32..10_000 {
        let mut session_bytes = [0; 16];
        session_bytes[..4].copy_from_slice(&n.to_be_bytes());
        let mut operation_bytes = [0; 16];
        operation_bytes[..4].copy_from_slice(&n.to_be_bytes());
        layout.retired.push((
            TerminalSessionID(session_bytes),
            OperationID(operation_bytes),
            runtime(),
        ));
    }
    let before = layout.clone();
    let error = layout
        .new_tab(tab(1), session(1), operation(1), runtime())
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Busy);
    assert_eq!(layout, before);
}
