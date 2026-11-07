use buntrail_core::{Document, Error};

#[test]
fn rust_authority_unicode_crlf_delta_undo_and_composition() {
    let original = "a😀e\u{301}\r\n日本\r\n";
    let mut doc = Document::new(original.into());
    assert_eq!(doc.edit(0, 1, 2, 0, "x"), Err(Error::InvalidRange));
    let delta = doc.edit(0, 1, 1, 2, "🐇").unwrap();
    assert_eq!(delta.text, "🐇");
    assert_eq!(delta.removed_utf16, 2);
    assert!(doc.text().contains("🐇"));
    assert_eq!(doc.edit(0, 2, 0, 0, "bad"), Err(Error::Stale));
    doc.undo(1, 1).unwrap();
    assert_eq!(doc.text(), original);
    doc.redo(2, 1).unwrap();
    assert!(doc.text().contains("🐇"));
    doc.begin_composition(3, 1).unwrap();
    doc.edit(3, 1, 0, 1, "に").unwrap();
    assert_eq!(doc.edit(4, 2, 0, 1, "x"), Err(Error::Composition));
    assert_eq!(doc.confirmed_snapshot(), Err(Error::Composition));
    doc.cancel_composition(4, 1).unwrap();
    assert!(doc.text().starts_with("a🐇"));
    doc.begin_composition(5, 1).unwrap();
    doc.edit(5, 1, 0, 1, "に").unwrap();
    doc.edit(6, 1, 0, 1, "日本").unwrap();
    doc.commit_composition(7, 1).unwrap();
    doc.undo(7, 1).unwrap();
    assert!(doc.text().starts_with("a🐇"));
    doc.redo(9, 1).unwrap();
    assert!(doc.text().starts_with("日本🐇"));
    assert!(doc.confirmed_snapshot().unwrap().text.ends_with("\r\n"));
}

#[test]
fn delta_payload_does_not_copy_entire_document() {
    let mut doc = Document::new("x".repeat(1_048_576));
    let delta = doc.edit(0, 1, 10, 1, "y").unwrap();
    assert_eq!(delta.text.len(), 1);
    assert_eq!(doc.copied_delta_bytes(), 1);
    doc.undo(1, 1).unwrap();
    assert_eq!(doc.copied_delta_bytes(), 2);
}

#[test]
fn invalid_range_does_not_mutate_revision_or_text() {
    let mut doc = Document::new("😀\r\n".into());
    for (start, length) in [(1, 0), (0, 1), (usize::MAX, 2), (4, 1)] {
        assert!(doc.edit(0, 1, start, length, "bad").is_err());
        assert_eq!(doc.revision(), 0);
        assert_eq!(doc.text(), "😀\r\n");
    }
    assert_eq!(
        buntrail_core::digest(doc.text().as_bytes()),
        buntrail_core::digest("😀\r\n".as_bytes())
    );
}

#[test]
fn undo_restores_replaced_selection_anchor() {
    use buntrail_core::contracts::*;
    let mut d = Document::new("abc".into());
    d.apply_transaction(EditTransaction {
        expected_revision: BufferRevision(0),
        source_view: ViewID(1),
        replacements: vec![Replacement {
            range: BufferRange {
                revision: BufferRevision(0),
                start: 0,
                end: 1,
            },
            text: "word".into(),
        }],
        intent: EditIntent::Replace,
    })
    .unwrap();
    let undo = d
        .apply_transaction(EditTransaction {
            expected_revision: BufferRevision(1),
            source_view: ViewID(1),
            replacements: vec![],
            intent: EditIntent::Undo,
        })
        .unwrap();
    assert_eq!(
        undo.selection_anchors,
        vec![BufferRange {
            revision: BufferRevision(2),
            start: 0,
            end: 1
        }]
    );
}
