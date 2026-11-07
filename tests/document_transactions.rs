use buntrail_core::{contracts::*, documents::*};
use std::sync::Arc;
fn loaded(id: u8) -> LoadedDocument {
    let target = TargetIdentity {
        security_binding: vec![1],
        effective_user: vec![2],
        filesystem_namespace: vec![3],
        container: None,
    };
    LoadedDocument {
        document: DocumentID([id; 16]),
        file: FileRef {
            target: target.clone(),
            path: TargetPath::posix(b"/file".to_vec()).unwrap(),
            resolved_identity: Some(vec![9]),
        },
        version: FileVersion {
            target,
            opaque_token: vec![1],
            content_digest: [0; 32],
            file_identity: vec![9],
        },
        text: Arc::new("a😀\r\n".into()),
        encoding: Encoding::Utf8 { bom: false },
        line_endings: LineEndings::CrLf,
        read_only: None,
        journal_epoch: 0,
        journal_lease: None,
    }
}
fn edit(revision: u64, intent: EditIntent, replacements: Vec<Replacement>) -> EditTransaction {
    EditTransaction {
        expected_revision: BufferRevision(revision),
        source_view: ViewID(1),
        replacements,
        intent,
    }
}
fn replacement(revision: u64, start: u64, end: u64, text: &str) -> Replacement {
    Replacement {
        range: BufferRange {
            revision: BufferRevision(revision),
            start,
            end,
        },
        text: text.into(),
    }
}
#[test]
fn two_views_share_one_document() {
    let mut c = DocumentCoordinator::default();
    let id = c.open(loaded(1)).unwrap();
    assert_eq!(c.open(loaded(2)).unwrap(), id);
    c.get_mut(id)
        .unwrap()
        .apply_transaction(edit(
            0,
            EditIntent::Replace,
            vec![replacement(0, 0, 1, "b")],
        ))
        .unwrap();
    assert_eq!(c.get(id).unwrap().snapshot().text.as_str(), "b😀\r\n");
}
#[test]
fn composition_is_one_undo_group() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.apply_transaction(edit(0, EditIntent::BeginComposition, vec![]))
        .unwrap();
    d.apply_transaction(edit(
        0,
        EditIntent::UpdateComposition,
        vec![replacement(0, 0, 1, "に")],
    ))
    .unwrap();
    d.apply_transaction(edit(
        1,
        EditIntent::UpdateComposition,
        vec![replacement(1, 0, 1, "日本")],
    ))
    .unwrap();
    d.apply_transaction(edit(2, EditIntent::CommitComposition, vec![]))
        .unwrap();
    d.apply_transaction(edit(2, EditIntent::Undo, vec![]))
        .unwrap();
    assert_eq!(d.snapshot().text.as_str(), "a😀\r\n");
}
#[test]
fn cancel_composition_restores_preimage() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.apply_transaction(edit(0, EditIntent::BeginComposition, vec![]))
        .unwrap();
    d.apply_transaction(edit(
        0,
        EditIntent::UpdateComposition,
        vec![replacement(0, 0, 1, "に")],
    ))
    .unwrap();
    assert!(d.recovery_snapshot().provisional_composition);
    d.apply_transaction(edit(1, EditIntent::CancelComposition, vec![]))
        .unwrap();
    assert_eq!(d.snapshot().text.as_str(), "a😀\r\n");
}
#[test]
fn save_requires_composition_resolution() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.apply_transaction(edit(0, EditIntent::BeginComposition, vec![]))
        .unwrap();
    assert_eq!(
        d.prepare_save(OperationID([3; 16])).unwrap_err().code,
        ErrorCode::Composition
    );
}
#[test]
fn edit_during_save_stays_dirty() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "b")],
    ))
    .unwrap();
    let request = d.prepare_save(OperationID([3; 16])).unwrap();
    d.apply_transaction(edit(
        1,
        EditIntent::Replace,
        vec![replacement(1, 0, 1, "c")],
    ))
    .unwrap();
    d.complete_save(OperationReceipt {
        id: request.operation,
        state: OperationState::Confirmed(MutationResult::Saved(SaveResult {
            saved_revision: BufferRevision(1),
            version: loaded(1).version,
        })),
    })
    .unwrap();
    assert!(d.is_dirty());
    assert_eq!(d.snapshot().revision, BufferRevision(2));
}
#[test]
fn unicode_ranges_reject_split_surrogates() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    assert!(
        d.apply_transaction(edit(
            0,
            EditIntent::Replace,
            vec![replacement(0, 2, 2, "bad")]
        ))
        .is_err()
    );
    assert_eq!(d.snapshot().text.as_str(), "a😀\r\n");
}
#[test]
fn undo_restores_selection_anchors() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    let before = BufferRange {
        revision: BufferRevision(0),
        start: 0,
        end: 1,
    };
    d.set_selection(ViewID(1), before).unwrap();
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "word")],
    ))
    .unwrap();
    let result = d
        .apply_transaction(edit(1, EditIntent::Undo, vec![]))
        .unwrap();
    assert_eq!(
        result.selection_anchors,
        vec![BufferRange {
            revision: BufferRevision(2),
            start: 0,
            end: 1
        }]
    );
}
#[test]
fn ordinary_typing_boundaries_and_replace_all_are_undo_groups() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.begin_typing(ViewID(1));
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 0, "x")],
    ))
    .unwrap();
    d.apply_transaction(edit(
        1,
        EditIntent::Replace,
        vec![replacement(1, 1, 1, "y")],
    ))
    .unwrap();
    d.end_typing();
    d.apply_transaction(edit(2, EditIntent::Undo, vec![]))
        .unwrap();
    assert_eq!(d.snapshot().text.as_str(), "a😀\r\n");
}
#[test]
fn restored_draft_is_dirty_and_pending_save_cannot_be_replayed() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "draft")],
    ))
    .unwrap();
    let p = d.prepare_save(OperationID([5; 16])).unwrap();
    let mut c = DocumentCoordinator::default();
    let id = c.restore(p.recovery).unwrap();
    let restored = c.get_mut(id).unwrap();
    assert!(restored.is_dirty());
    assert_eq!(restored.snapshot().text.as_str(), "draft😀\r\n");
    assert_eq!(
        restored
            .prepare_save(OperationID([6; 16]))
            .unwrap_err()
            .code,
        ErrorCode::Busy
    );
}
#[test]
fn empty_composition_does_not_shift_undo_selection_history() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.set_selection(
        ViewID(1),
        BufferRange {
            revision: BufferRevision(0),
            start: 0,
            end: 1,
        },
    )
    .unwrap();
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "x")],
    ))
    .unwrap();
    d.set_selection(
        ViewID(1),
        BufferRange {
            revision: BufferRevision(1),
            start: 3,
            end: 3,
        },
    )
    .unwrap();
    d.apply_transaction(edit(1, EditIntent::BeginComposition, vec![]))
        .unwrap();
    d.apply_transaction(edit(1, EditIntent::CommitComposition, vec![]))
        .unwrap();
    let undo = d
        .apply_transaction(edit(1, EditIntent::Undo, vec![]))
        .unwrap();
    assert_eq!(undo.selection_anchors[0].start, 0);
    assert_eq!(undo.selection_anchors[0].end, 1);
}
#[test]
fn another_views_edit_ends_typing_group() {
    let mut d = DocumentSession::from_loaded(loaded(1));
    d.begin_typing(ViewID(1));
    let mut other = edit(0, EditIntent::Replace, vec![replacement(0, 0, 1, "other")]);
    other.source_view = ViewID(2);
    d.apply_transaction(other).unwrap();
    d.apply_transaction(edit(
        1,
        EditIntent::Replace,
        vec![replacement(1, 0, 5, "mine")],
    ))
    .unwrap();
    d.apply_transaction(edit(2, EditIntent::Undo, vec![]))
        .unwrap();
    assert_eq!(d.snapshot().text.as_str(), "other😀\r\n");
}
#[test]
fn rebind_keeps_clean_save_state_consistent_with_the_new_path_version() {
    let mut d = DocumentSession::from_loaded(loaded(30));
    let mut file = d.file().clone();
    file.path = TargetPath::posix(b"/renamed".to_vec()).unwrap();
    let mut version = d.version().clone();
    version.opaque_token.push(2);
    d.rebind(file, version.clone()).unwrap();
    assert!(matches!(d.save_state(),DocumentSaveState::Clean(current) if current==&version));
}
#[test]
fn rebind_does_not_accept_changed_disk_contents_as_the_base_of_dirty_edits() {
    let mut d = DocumentSession::from_loaded(loaded(31));
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "dirty")],
    ))
    .unwrap();
    let original = d.version().clone();
    let mut file = d.file().clone();
    file.path = TargetPath::posix(b"/renamed".to_vec()).unwrap();
    let mut changed = original.clone();
    changed.content_digest = [2; 32];
    changed.opaque_token.push(3);
    d.rebind(file.clone(), changed).unwrap();
    assert_eq!(d.file(), &file);
    assert_eq!(
        d.version(),
        &original,
        "changed disk bytes must still conflict with the submitted dirty base"
    );
    assert_eq!(d.snapshot().text.as_str(), "dirty😀\r\n");
    assert!(d.is_dirty());
}

#[test]
fn preparing_discard_does_not_declare_live_unsaved_bytes_saved() {
    let mut d = DocumentSession::from_loaded(loaded(90));
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "unsaved")],
    ))
    .unwrap();
    let before = d.snapshot();
    let marker = d.discard().unwrap();
    assert!(marker.clean);
    assert_eq!(marker.generation, d.recovery_snapshot().generation + 1);
    assert_eq!(d.snapshot().text, before.text);
    assert!(d.is_dirty());
    assert!(!d.recovery_snapshot().clean);
}

#[test]
fn undo_to_saved_checkpoint_clears_dirty_and_redo_marks_dirty() {
    let mut d = DocumentSession::from_loaded(loaded(91));
    assert!(!d.is_dirty());
    assert!(matches!(d.save_state(), DocumentSaveState::Clean(_)));
    d.set_selection(
        ViewID(1),
        BufferRange {
            revision: BufferRevision(0),
            start: 1,
            end: 1,
        },
    )
    .unwrap();
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 1, 1, "Ω")],
    ))
    .unwrap();
    assert_eq!(d.snapshot().revision, BufferRevision(1));
    assert_eq!(d.snapshot().text.as_str(), "aΩ😀\r\n");
    assert!(d.is_dirty());

    let undo = d
        .apply_transaction(edit(1, EditIntent::Undo, vec![]))
        .unwrap();
    assert_eq!(d.snapshot().revision, BufferRevision(2));
    assert_eq!(d.snapshot().text.as_str(), "a😀\r\n");
    assert_eq!(
        undo.selection_anchors,
        vec![BufferRange {
            revision: BufferRevision(2),
            start: 1,
            end: 1,
        }]
    );
    let dirty_after_undo = d.is_dirty();
    let save_state_after_undo = d.save_state().clone();
    let recovery_after_undo = d.recovery_snapshot();
    println!(
        "saved checkpoint restored at revision 2: dirty={dirty_after_undo}, save_state={save_state_after_undo:?}, recovery_clean={}",
        recovery_after_undo.clean
    );

    let redo = d
        .apply_transaction(edit(2, EditIntent::Redo, vec![]))
        .unwrap();
    assert_eq!(d.snapshot().revision, BufferRevision(3));
    assert_eq!(d.snapshot().text.as_str(), "aΩ😀\r\n");
    assert_eq!(
        redo.selection_anchors,
        vec![BufferRange {
            revision: BufferRevision(3),
            start: 2,
            end: 2,
        }]
    );
    assert!(d.is_dirty());
    assert!(matches!(d.save_state(), DocumentSaveState::Dirty));
    assert!(!d.recovery_snapshot().clean);
    assert!(
        !dirty_after_undo,
        "Undo restored the confirmed saved checkpoint at a newer mutation revision"
    );
    assert!(matches!(save_state_after_undo, DocumentSaveState::Clean(_)));
    assert!(recovery_after_undo.clean);
}

fn navigate(d: &mut DocumentSession, intent: EditIntent) {
    let revision = d.snapshot().revision.0;
    d.apply_transaction(edit(revision, intent, vec![])).unwrap();
}
fn replace_head(d: &mut DocumentSession, text: &str) {
    let revision = d.snapshot().revision.0;
    d.apply_transaction(edit(
        revision,
        EditIntent::Replace,
        vec![replacement(revision, 0, 1, text)],
    ))
    .unwrap();
}
fn confirm(d: &mut DocumentSession, save: &PreparedSave) {
    let mut version = save.recovery.version.clone();
    version.opaque_token.push(2);
    d.complete_save(OperationReceipt {
        id: save.operation,
        state: OperationState::Confirmed(MutationResult::Saved(SaveResult {
            saved_revision: save.recovery.snapshot.revision,
            version,
        })),
    })
    .unwrap();
}
fn assert_checkpoint(d: &DocumentSession, text: &str, dirty: bool) {
    assert_eq!(d.snapshot().text.as_str(), text);
    assert_eq!(d.is_dirty(), dirty);
    assert_eq!(d.recovery_snapshot().clean, !dirty);
    if dirty {
        assert!(matches!(d.save_state(), DocumentSaveState::Dirty));
    } else {
        assert!(matches!(d.save_state(), DocumentSaveState::Clean(v) if v == d.version()));
    }
}

#[test]
fn savepoint_saved_mid_history_is_found_by_undo_and_redo() {
    let mut d = DocumentSession::from_loaded(loaded(92));
    replace_head(&mut d, "b");
    replace_head(&mut d, "c");
    navigate(&mut d, EditIntent::Undo);
    let save = d.prepare_save(OperationID([1; 16])).unwrap();
    confirm(&mut d, &save);
    assert_checkpoint(&d, "b😀\r\n", false);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "a😀\r\n", true);
    navigate(&mut d, EditIntent::Redo);
    assert_checkpoint(&d, "b😀\r\n", false);
    navigate(&mut d, EditIntent::Redo);
    assert_checkpoint(&d, "c😀\r\n", true);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "b😀\r\n", false);
    assert_eq!(d.snapshot().revision, BufferRevision(7));
}

#[test]
fn savepoint_abandoned_branch_at_same_depth_is_not_saved() {
    let mut d = DocumentSession::from_loaded(loaded(93));
    replace_head(&mut d, "b");
    let save = d.prepare_save(OperationID([1; 16])).unwrap();
    confirm(&mut d, &save);
    navigate(&mut d, EditIntent::Undo);
    replace_head(&mut d, "c");
    assert_checkpoint(&d, "c😀\r\n", true);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "a😀\r\n", true);
    navigate(&mut d, EditIntent::Redo);
    assert_checkpoint(&d, "c😀\r\n", true);
}

#[test]
fn savepoint_save_ends_typing_group_without_moving_saved_midpoint() {
    let mut d = DocumentSession::from_loaded(loaded(94));
    d.begin_typing(ViewID(1));
    replace_head(&mut d, "b");
    replace_head(&mut d, "c");
    let save = d.prepare_save(OperationID([1; 16])).unwrap();
    d.begin_typing(ViewID(1));
    replace_head(&mut d, "d");
    replace_head(&mut d, "e");
    confirm(&mut d, &save);
    assert_checkpoint(&d, "e😀\r\n", true);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "c😀\r\n", false);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "a😀\r\n", true);
    navigate(&mut d, EditIntent::Redo);
    assert_checkpoint(&d, "c😀\r\n", false);
    navigate(&mut d, EditIntent::Redo);
    assert_checkpoint(&d, "e😀\r\n", true);
}

#[test]
fn savepoint_continued_typing_after_save_can_undo_to_saved_text() {
    let mut d = DocumentSession::from_loaded(loaded(95));
    d.begin_typing(ViewID(1));
    replace_head(&mut d, "b");
    let save = d.prepare_save(OperationID([1; 16])).unwrap();
    replace_head(&mut d, "c");
    confirm(&mut d, &save);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "b😀\r\n", false);
}

#[test]
fn savepoint_composition_commit_cancel_and_empty_groups_keep_checkpoint() {
    for finish in [EditIntent::CommitComposition, EditIntent::CancelComposition] {
        let mut d = DocumentSession::from_loaded(loaded(96));
        navigate(&mut d, EditIntent::BeginComposition);
        assert!(!d.recovery_snapshot().clean);
        d.apply_transaction(edit(
            0,
            EditIntent::UpdateComposition,
            vec![replacement(0, 0, 1, "に")],
        ))
        .unwrap();
        assert!(d.is_dirty());
        assert!(d.prepare_save(OperationID([1; 16])).is_err());
        navigate(&mut d, finish.clone());
        if finish == EditIntent::CommitComposition {
            assert_checkpoint(&d, "に😀\r\n", true);
            navigate(&mut d, EditIntent::Undo);
            assert_checkpoint(&d, "a😀\r\n", false);
            navigate(&mut d, EditIntent::Redo);
            assert_checkpoint(&d, "に😀\r\n", true);
            navigate(&mut d, EditIntent::Undo);
        }
        assert_checkpoint(&d, "a😀\r\n", false);
        let revision = d.snapshot().revision;
        navigate(&mut d, EditIntent::BeginComposition);
        navigate(&mut d, finish);
        assert_checkpoint(&d, "a😀\r\n", false);
        assert_eq!(d.snapshot().revision, revision);
    }
}

#[test]
fn savepoint_inflight_and_unknown_survive_navigation_until_confirmation() {
    for unknown in [false, true] {
        let mut d = DocumentSession::from_loaded(loaded(97));
        let save = d.prepare_save(OperationID([1; 16])).unwrap();
        replace_head(&mut d, "b");
        if unknown {
            d.complete_save(OperationReceipt {
                id: save.operation,
                state: OperationState::Unknown(ReconciliationEvidence {
                    target: d.file().target.clone(),
                    observed: ObservedState::Unobserved,
                    explanation: "lost acknowledgement".into(),
                }),
            })
            .unwrap();
        }
        let pending_state = d.save_state().clone();
        navigate(&mut d, EditIntent::Undo);
        assert_eq!(d.snapshot().text.as_str(), "a😀\r\n");
        assert!(!d.is_dirty());
        assert_eq!(d.save_state(), &pending_state);
        assert!(!d.recovery_snapshot().clean);
        assert_eq!(
            d.recovery_snapshot().pending_operation,
            Some(save.operation)
        );
        assert_eq!(
            d.prepare_save(OperationID([2; 16])).unwrap_err().code,
            ErrorCode::Busy
        );
        navigate(&mut d, EditIntent::Redo);
        assert!(d.is_dirty());
        assert_eq!(d.save_state(), &pending_state);
        navigate(&mut d, EditIntent::Undo);
        confirm(&mut d, &save);
        assert_checkpoint(&d, "a😀\r\n", false);
    }
}

#[test]
fn savepoint_acknowledgement_confirms_submitted_checkpoint_not_current_edit() {
    for return_before_ack in [false, true] {
        let mut d = DocumentSession::from_loaded(loaded(98));
        replace_head(&mut d, "b");
        let save = d.prepare_save(OperationID([1; 16])).unwrap();
        replace_head(&mut d, "c");
        if return_before_ack {
            navigate(&mut d, EditIntent::Undo);
        }
        confirm(&mut d, &save);
        if return_before_ack {
            assert_checkpoint(&d, "b😀\r\n", false);
        } else {
            assert_checkpoint(&d, "c😀\r\n", true);
            navigate(&mut d, EditIntent::Undo);
            assert_checkpoint(&d, "b😀\r\n", false);
        }
        navigate(&mut d, EditIntent::Redo);
        assert_checkpoint(&d, "c😀\r\n", true);
    }
}

#[test]
fn savepoint_restore_never_confirms_unconfirmed_or_provisional_equal_text() {
    for provisional in [false, true] {
        let d = DocumentSession::from_loaded(loaded(99));
        let mut recovery = d.recovery_snapshot();
        recovery.clean = false;
        recovery.provisional_composition = provisional;
        let mut coordinator = DocumentCoordinator::default();
        let id = coordinator.restore(recovery).unwrap();
        let restored = coordinator.get_mut(id).unwrap();
        assert!(restored.is_dirty());
        if provisional {
            assert!(restored.prepare_save(OperationID([1; 16])).is_err());
            navigate(restored, EditIntent::CommitComposition);
        }
        replace_head(restored, "b");
        navigate(restored, EditIntent::Undo);
        assert_checkpoint(restored, "a😀\r\n", true);
        let save = restored.prepare_save(OperationID([2; 16])).unwrap();
        confirm(restored, &save);
        assert_checkpoint(restored, "a😀\r\n", false);
    }
}

#[test]
fn savepoint_restored_pending_confirmation_requires_captured_revision() {
    for (changed_since_save, provisional) in [(false, false), (true, false), (false, true)] {
        let mut d = DocumentSession::from_loaded(loaded(100));
        replace_head(&mut d, "b");
        let save = d.prepare_save(OperationID([1; 16])).unwrap();
        if changed_since_save {
            replace_head(&mut d, "c");
            navigate(&mut d, EditIntent::Undo);
        }
        let mut recovery = d.recovery_snapshot();
        recovery.provisional_composition = provisional;
        let mut coordinator = DocumentCoordinator::default();
        let id = coordinator.restore(recovery).unwrap();
        let restored = coordinator.get_mut(id).unwrap();
        assert!(matches!(
            restored.save_state(),
            DocumentSaveState::Unknown(_)
        ));
        assert!(restored.is_dirty());
        confirm(restored, &save);
        if provisional {
            assert!(restored.is_dirty());
            assert!(!restored.recovery_snapshot().clean);
            navigate(restored, EditIntent::CommitComposition);
        }
        assert_checkpoint(restored, "b😀\r\n", changed_since_save || provisional);
    }
}

#[test]
fn savepoint_failed_conflict_is_not_resolved_by_undo() {
    let mut d = DocumentSession::from_loaded(loaded(101));
    replace_head(&mut d, "b");
    let save = d.prepare_save(OperationID([1; 16])).unwrap();
    d.complete_save(OperationReceipt {
        id: save.operation,
        state: OperationState::Failed(OperationFailure {
            error: CoreError::new(ErrorCode::StaleRevision, "file version conflict"),
            target: d.file().target.clone(),
            observed: ObservedState::Unobserved,
            effect: FailureEffect::ProvenNoEffect,
        }),
    })
    .unwrap();
    let failure = d.save_state().clone();
    navigate(&mut d, EditIntent::Undo);
    assert!(!d.is_dirty());
    assert_eq!(d.save_state(), &failure);
    navigate(&mut d, EditIntent::Redo);
    assert!(d.is_dirty());
    assert!(matches!(d.save_state(), DocumentSaveState::Dirty));
}

#[test]
fn savepoint_revalidate_reload_keep_edits_and_rebind_preserve_authority() {
    let mut d = DocumentSession::from_loaded(loaded(102));
    replace_head(&mut d, "b");
    navigate(&mut d, EditIntent::Undo);
    let mut external = loaded(102);
    external.text = Arc::new("x😀\r\n".into());
    external.version.opaque_token.push(2);
    external.version.content_digest = [2; 32];
    assert!(d.revalidate(external).unwrap());
    assert_checkpoint(&d, "x😀\r\n", false);
    assert_eq!(d.snapshot().revision, BufferRevision(3));
    replace_head(&mut d, "y");
    let mut external = loaded(102);
    external.text = Arc::new("z😀\r\n".into());
    external.version.opaque_token.push(3);
    external.version.content_digest = [3; 32];
    assert!(!d.revalidate(external.clone()).unwrap());
    d.resolve_external(
        external.clone(),
        BufferRevision(4),
        &external.version,
        ExternalResolution::KeepEdits,
    )
    .unwrap();
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "x😀\r\n", true);
    d.external = Some(external.clone());
    d.resolve_external(
        external.clone(),
        BufferRevision(5),
        &external.version,
        ExternalResolution::Reload,
    )
    .unwrap();
    assert_checkpoint(&d, "z😀\r\n", false);
    replace_head(&mut d, "w");
    let mut file = d.file().clone();
    file.path = TargetPath::posix(b"/renamed".to_vec()).unwrap();
    let mut version = d.version().clone();
    version.opaque_token.push(4);
    d.rebind(file, version).unwrap();
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "z😀\r\n", false);
    assert_eq!(d.snapshot().revision, BufferRevision(8));
}

#[test]
fn savepoint_multi_replacement_is_atomic_and_one_checkpoint() {
    let mut d = DocumentSession::from_loaded(loaded(103));
    assert!(
        d.apply_transaction(edit(
            0,
            EditIntent::Replace,
            vec![replacement(0, 0, 1, "x"), replacement(0, 2, 3, "bad")]
        ))
        .is_err()
    );
    assert_checkpoint(&d, "a😀\r\n", false);
    assert_eq!(d.snapshot().revision, BufferRevision(0));
    d.apply_transaction(edit(
        0,
        EditIntent::Replace,
        vec![replacement(0, 0, 1, "x"), replacement(0, 3, 3, "Ω")],
    ))
    .unwrap();
    assert_checkpoint(&d, "x😀Ω\r\n", true);
    navigate(&mut d, EditIntent::Undo);
    assert_checkpoint(&d, "a😀\r\n", false);
    navigate(&mut d, EditIntent::Redo);
    assert_checkpoint(&d, "x😀Ω\r\n", true);
    assert_eq!(d.snapshot().revision, BufferRevision(6));
}

#[test]
fn savepoint_mismatched_save_receipt_cannot_confirm_any_checkpoint() {
    for mismatch in 0..3 {
        let mut d = DocumentSession::from_loaded(loaded(104));
        replace_head(&mut d, "b");
        let save = d.prepare_save(OperationID([1; 16])).unwrap();
        let mut operation = save.operation;
        let mut version = d.version().clone();
        let mut revision = BufferRevision(1);
        match mismatch {
            0 => operation = OperationID([2; 16]),
            1 => revision = BufferRevision(0),
            _ => version.target.security_binding.push(4),
        }
        let before = d.recovery_snapshot();
        let state = d.save_state().clone();
        assert_eq!(
            d.complete_save(OperationReceipt {
                id: operation,
                state: OperationState::Confirmed(MutationResult::Saved(SaveResult {
                    saved_revision: revision,
                    version,
                })),
            })
            .unwrap_err()
            .code,
            ErrorCode::InvalidArgument
        );
        assert_eq!(d.recovery_snapshot(), before);
        assert_eq!(d.save_state(), &state);
        assert!(d.is_dirty());
        confirm(&mut d, &save);
        assert_checkpoint(&d, "b😀\r\n", false);
    }
}
