use buntrail_core::Document;
#[test]
fn retained_snapshot_does_not_copy_document_when_typing() {
    let mut d = Document::new("x".repeat(1_048_576));
    let old = d.snapshot();
    d.edit(0, 1, 10, 1, "😀").unwrap();
    let new = d.snapshot();
    assert_eq!(
        old.text.chunks().last().unwrap().as_ptr(),
        new.text.chunks().last().unwrap().as_ptr()
    );
    assert_eq!(old.text.len(), 1_048_576);
    let mut bytes = [0; 6];
    assert_eq!(d.snapshot().text.read(9, &mut bytes), 6);
    assert_eq!(&bytes, "x😀x".as_bytes());
}
#[test]
fn repeated_unicode_coordinate_edits_and_undo_keep_snapshot_chunks_shared() {
    let mut d = Document::new("abc😀\r\n".repeat(131072));
    let old = d.snapshot();
    for _ in 0..50 {
        let r = d.revision();
        d.edit(r, 1, 3, 2, "🐇").unwrap();
        let s = d.snapshot();
        assert_eq!(s.text.utf16_to_byte(5).unwrap(), 7);
        assert!(s.text.utf16_to_byte(4).is_err());
        d.undo(d.revision(), 1).unwrap();
    }
    assert_eq!(
        old.text.chunks().last().unwrap().as_ptr(),
        d.snapshot().text.chunks().last().unwrap().as_ptr()
    );
}
