use buntrail_core::{
    Document,
    contracts::*,
    search::{find::*, replace},
};
fn options() -> FindOptions {
    FindOptions {
        expression: "(café)".into(),
        mode: SearchMode::Regex,
        case_sensitive: false,
        whole_word: true,
        scope: None,
    }
}
#[test]
fn test_replace_all_is_one_undo_group() {
    let mut d = Document::new("😀 CAFÉ café cafeteria\n".into());
    let tx = replace::transaction(
        &d.snapshot().text,
        BufferRevision(0),
        ViewID(1),
        &options(),
        "${1}🐇",
    )
    .unwrap();
    assert_eq!(tx.replacements.len(), 2);
    d.apply_transaction(tx).unwrap();
    assert_eq!(d.text(), "😀 CAFÉ🐇 café🐇 cafeteria\n");
    assert_eq!(d.undo_depth(), 1);
    d.undo(d.revision(), 1).unwrap();
    assert_eq!(d.text(), "😀 CAFÉ café cafeteria\n");
}
#[test]
fn selection_and_unicode_boundaries() {
    let d = Document::new("😀 café café".into());
    let mut o = options();
    o.scope = Some(BufferRange {
        revision: BufferRevision(0),
        start: 8,
        end: 12,
    });
    let m = find(&d.snapshot().text, BufferRevision(0), &o).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].start, 8);
    o.scope.as_mut().unwrap().start = 1;
    assert!(find(&d.snapshot().text, BufferRevision(0), &o).is_err());
    o = options();
    o.expression = "[".into();
    assert!(find(&d.snapshot().text, BufferRevision(0), &o).is_err());
}

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
struct MeasuredAllocator;
thread_local! { static MEASURE: Cell<bool> = const {Cell::new(false)}; static PEAK: Cell<usize> = const {Cell::new(0)}; static TOTAL: Cell<usize> = const {Cell::new(0)}; }
fn measured(size: usize) {
    if MEASURE.try_with(Cell::get).unwrap_or(false) {
        let _ = PEAK.try_with(|v| v.set(v.get().max(size)));
        let _ = TOTAL.try_with(|v| v.set(v.get().saturating_add(size)));
    }
}
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        measured(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        measured(size);
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;
fn allocation_measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    PEAK.set(0);
    TOTAL.set(0);
    MEASURE.set(true);
    let value = f();
    MEASURE.set(false);
    (value, PEAK.get(), TOTAL.get())
}
#[test]
fn capture_expansion_refuses_before_oversized_allocation() {
    let text = buntrail_core::Text::from("a".repeat(1024 * 1024));
    let mut options = options();
    options.expression = "^(a+)$".into();
    options.whole_word = false;
    let replacement = "$1".repeat(40);
    let (result, peak, _) =
        allocation_measure(|| plan(&text, BufferRevision(0), &options, &replacement));
    assert!(result.is_err());
    eprintln!("single_capture_peak_bytes={peak}");
    assert!(
        peak <= 4 * 1024 * 1024,
        "oversized capture allocated {peak} before refusal"
    );
}
#[test]
fn aggregate_capture_budget_refuses_before_expanding_later_match() {
    let text = buntrail_core::Text::from(format!(
        "{}\n{}\n",
        "a".repeat(1024 * 1024),
        "a".repeat(1024 * 1024)
    ));
    let mut options = options();
    options.expression = "^(a+)$".into();
    options.whole_word = false;
    let replacement = "$1".repeat(20);
    let (result, peak, total) =
        allocation_measure(|| plan(&text, BufferRevision(0), &options, &replacement));
    assert!(result.is_err());
    eprintln!("aggregate_capture_peak_bytes={peak} requested_total_bytes={total}");
    assert!(
        peak <= 24 * 1024 * 1024,
        "allocated {peak} for a 20 MiB first value or over-budget later value"
    );
    assert!(
        total < 32 * 1024 * 1024,
        "allocation requests {total} expanded a later match beyond the remaining budget"
    );
}
#[test]
fn bounded_replacement_keeps_regex_interpolation_grammar() {
    let text = buntrail_core::Text::from("ab");
    let mut options = options();
    options.expression = "(?<first>a)(b)?(?<missing>c)?".into();
    options.whole_word = false;
    let regex = pattern(&options).unwrap();
    let captures = regex.captures("ab").unwrap();
    for template in [
        "$0|$1|${2}|$first|${first}|$missing|$100|$1a",
        "$$|$$1|$$$1|${}|${1|$|${名前}|$99999999999999999999999999999999",
        "${first}🐇${2}",
    ] {
        let mut expected = String::new();
        captures.expand(template, &mut expected);
        assert_eq!(
            plan(&text, BufferRevision(0), &options, template)
                .unwrap()
                .replacements[0]
                .text,
            expected
        );
    }
}
