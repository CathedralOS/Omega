//! Declared-field domain coverage for fixed-array collection elements. Every
//! element carries its declared field predicates at the exact `FixedIndex`
//! place; coverage flows through indexing, element borrows, whole-array calls,
//! copies, and transitions. A corrupted element or stale alias retires only
//! that element's facts, so the rejecting call names the retired coordinate.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

const DEFINITIONS: &str = r#"
domain [u8; 4]::Utf8 requires valid_utf8(self);
data Text { bytes: [u8; 4] in Utf8; }
data Packet { payload: Text; tag: u64; }
data Row { bytes: [u8; 4] in Utf8; tag: u64; }
"#;

fn check(source: &str, accepted: bool) {
    check_rejection(
        source,
        accepted,
        "cannot prove default-domain field requirement",
    );
}

fn check_rejection(source: &str, accepted: bool, fragment: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(
            accepted,
            "an unproved collection element crossed the boundary:\n{source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(fragment)),
                "expected a diagnostic containing {fragment:?}: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn corrupting_the_source_after_the_copy_keeps_the_copy_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data CopyRow [copy] {{ bytes: [u8; 4] in Utf8; tag: u64; }}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &CopyRow) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let mut rows: [CopyRow; 2] = [CopyRow {{bytes: "okay", tag: 0}}, CopyRow {{bytes: "okay", tag: 1}}];
            let r: CopyRow = rows[0];
            corrupt(&mut rows[0].bytes);
            consume(&r);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn reassigned_view_carries_the_new_receivers_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let mut a: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let mut b: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            corrupt(&mut a[0].bytes);
            let mut view: &[Row] = a.as_slice();
            view = b.as_slice();
            consume(&view[0]);
        }}
    "#
    );
    check(&source, true);
}

/// The floor on `rows[0]`'s field says nothing about `rows[1]` — the
/// sibling's nested write still has no extent to prove against.
#[test]
fn view_write_into_slice_field_rejects_an_unproven_sibling() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        machine caller(rows: &mut [SliceRow; 2]) requires rows[0].bytes.len > 0; {{
            let view: &mut [SliceRow] = rows.as_mut_slice();
            view[1].bytes[0] = 255;
        }}
    "#
    );
    check_rejection(&source, false, "within unknown slice length");
}

const COPY_ROW: &str = "data CopyRow [copy] { bytes: [u8; 4] in Utf8; tag: u64; }";

/// A returned owned collection keeps the same call-expression result facts a
/// direct result read already publishes: the declared element predicates of
/// `pair[i]` survive onto the destination binding.
#[test]
fn returned_array_into_local_carries_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        {COPY_ROW}
        machine both(rows: &[CopyRow; 2]) -> [CopyRow; 2] {{ [rows[0], rows[1]] }}
        machine consume(row: &CopyRow) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &[CopyRow; 2]) {{
            let pair: [CopyRow; 2] = both(rows);
            consume(&pair[1]);
        }}
    "#
    );
    check(&source, true);
}

/// The referent's extent is the binding's ceiling too: a two-element referent
/// does not lend an index it does not have.
#[test]
fn returned_mutable_slice_indexes_only_within_the_referents_extent() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine borrow_rows(level: &mut Level) -> &mut [Row] {{
            let slots: &mut [Row] = level.rooms.as_mut_slice();
            transition {{ _ -> slots }}
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let view: &mut [Row] = borrow_rows(level);
            consume(&view[2]);
        }}
    "#
    );
    check_rejection(&source, false, "cannot prove index `2` is within length 2");
}

/// The same narrowing through `as_slice()`/`as_mut_slice()` views: the
/// elementwise row re-anchors element-for-element below the view local, so
/// `view[index]` is covered exactly like `rows[index]`.
#[test]
fn runtime_index_reads_through_slice_views_carry_coverage() {
    for (view, call) in [("&[Row]", "as_slice"), ("&mut [Row]", "as_mut_slice")] {
        let source = format!(
            r#"{DEFINITIONS}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &mut [Row; 2], index: u64[0..2]) {{
                let view: {view} = rows.{call}();
                consume(&view[index]);
            }}
        "#
        );
        check(&source, true);
    }
}

/// A corrupted element retires the whole-extent row along with its own, so a
/// runtime selector that could name the corrupted element rejects: coverage
/// can never outlive the evidence it narrows from.
#[test]
fn corrupted_element_retires_whole_extent_coverage_for_runtime_reads() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2], index: u64[0..2]) {{
            let view: &mut [Row] = rows.as_mut_slice();
            let alias: &mut [u8; 4] = &mut view[1].bytes;
            alias[0] = 255;
            consume(&view[index]);
        }}
    "#
    );
    check(&source, false);
}
