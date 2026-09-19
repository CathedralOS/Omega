//! Declared-field domain coverage for fixed-array collection elements. Every
//! element carries its declared field predicates at the exact `FixedIndex`
//! place; coverage flows through indexing, element borrows, whole-array calls,
//! copies, and transitions. A corrupted element or stale alias retires only
//! that element's facts, so the rejecting call names the retired coordinate.
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

/// The caller's return re-proves every readable `&mut` referent's declared
/// field facts, so a corrupted element rejects there with its exact place
/// while every call in the body stays accepted: the sibling's coverage was
/// never disturbed.
fn check_sibling_coverage_at_calls_and_corruption_at_return(source: &str, place: &str) {
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(source)) else {
        panic!("a corrupted referent must not be handed back at the return:\n{source}");
    };
    let field_requirements = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove default-domain field requirement")
        })
        .collect::<Vec<_>>();
    assert!(
        !field_requirements.is_empty()
            && field_requirements.iter().all(|diagnostic| {
                diagnostic.message.contains("for return from caller")
                    && diagnostic.message.contains(place)
            }),
        "expected only the return-time rejection of {place:?}: {diagnostics:#?}\n{source}"
    );
}

fn check_rejection(source: &str, accepted: bool, fragment: &str) {
    match lower_typed_trees(parse_typed_trees(source)) {
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
fn fixed_array_element_borrow_calls_carry_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{ consume(&rows[0]); consume(&rows[1]); }}
    "#
    );
    check(&source, true);
}

#[test]
fn fixed_array_whole_collection_calls_require_every_element() {
    for (callee, caller, accepted) in [
        ("rows: &mut [Row; 2]", "consume_all(rows)", true),
        ("rows: &[Row; 2]", "consume_all_shared(rows)", true),
        ("rows: [Row; 2]", "consume_all_owned(rows)", true),
    ] {
        let source = format!(
            r#"{DEFINITIONS}
            machine consume_all(rows: &mut [Row; 2]) {{ }}
            machine consume_all_shared(rows: &[Row; 2]) {{ }}
            machine consume_all_owned(rows: [Row; 2]) {{ }}
            machine caller({callee}) {{ {caller}; }}
        "#
        );
        check(&source, accepted);
    }
}

#[test]
fn corrupted_fixed_array_element_rejects_the_whole_collection_call() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume_all(rows: &mut [Row; 2]) {{ }}
        machine caller(rows: &mut [Row; 2]) {{ corrupt(&mut rows[1].bytes); consume_all(rows); }}
    "#
    );
    check(&source, false);
}

#[test]
fn corrupting_one_element_preserves_its_siblings_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{ corrupt(&mut rows[0].bytes); consume(&rows[1]); }}
    "#
    );
    check_sibling_coverage_at_calls_and_corruption_at_return(&source, "rows[0].bytes");
}

#[test]
fn corrupted_element_rejects_its_own_indexed_borrow_call() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{ corrupt(&mut rows[0].bytes); consume(&rows[0]); }}
    "#
    );
    check(&source, false);
}

#[test]
fn stale_element_alias_retires_the_elements_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            let alias: &mut [u8; 4] = &mut rows[0].bytes;
            alias[0] = 255;
            consume(&rows[0]);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn rewritten_element_field_reestablishes_its_own_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{ rows[0].bytes = "okay"; consume(&rows[0]); }}
    "#
    );
    check(&source, true);
}

#[test]
fn whole_element_write_reestablishes_the_elements_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume_all(rows: &mut [Row; 2]) {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            rows[0] = Row {{bytes: "okay", tag: 0}};
            consume_all(rows);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn runtime_index_write_retires_every_elements_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume_all(rows: &mut [Row; 2]) {{ }}
        machine caller(rows: &mut [Row; 2], index: u64[0..2]) {{
            rows[index] = Row {{bytes: "okay", tag: 0}};
            consume_all(rows);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn element_coverage_flows_through_transitions() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine walk(rows: &mut [Row; 2]) {{
            transition {{ _ -> next(rows) }}
            state next(rows: &mut [Row; 2]) {{ consume(&rows[0]); }}
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn corrupted_element_rejects_the_transition() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine walk(rows: &mut [Row; 2]) {{
            corrupt(&mut rows[1].bytes);
            transition {{ _ -> next(rows) }}
            state next(rows: &mut [Row; 2]) {{ }}
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn mutable_call_ensures_transport_restores_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine fill(row: &mut Row) ensures row.bytes in Utf8 {{ row.bytes = "okay"; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            fill(&mut rows[0]);
            consume(&rows[0]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn nested_array_field_elements_carry_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Envelop {{ rows: [Row; 2]; stamp: u64; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(packet: &mut Envelop) {{ consume(&packet.rows[0]); }}
    "#
    );
    check(&source, true);
}

#[test]
fn nested_array_field_calls_require_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Envelop {{ rows: [Row; 2]; stamp: u64; }}
        machine consume(packet: &mut Envelop) {{ }}
        machine caller(packet: &mut Envelop) {{ consume(packet); }}
    "#
    );
    check(&source, true);
}

#[test]
fn corrupted_nested_array_field_element_rejects_the_call() {
    let source = format!(
        r#"{DEFINITIONS}
        data Envelop {{ rows: [Row; 2]; stamp: u64; }}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(packet: &mut Envelop) {{ }}
        machine caller(packet: &mut Envelop) {{ corrupt(&mut packet.rows[1].bytes); consume(packet); }}
    "#
    );
    check(&source, false);
}

#[test]
fn machine_field_array_elements_carry_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ rows: [Row; 2]; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine Main::run(&mut self) {{ consume(&self.rows[0]); consume(&self.rows[1]); }}
    "#
    );
    check(&source, true);
}

#[test]
fn corrupted_machine_field_element_rejects_the_indexed_call() {
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ rows: [Row; 2]; }}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine Main::run(&mut self) {{
            corrupt(&mut self.rows[0].bytes);
            consume(&self.rows[0]);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn mutable_self_calls_require_array_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ rows: [Row; 2]; }}
        machine Main::run(&mut self) {{ self.inspect(); }}
        machine Main::inspect(&mut self) {{ }}
    "#
    );
    check(&source, true);
}

#[test]
fn corrupted_machine_field_element_rejects_the_self_call() {
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ rows: [Row; 2]; }}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine Main::run(&mut self) {{
            corrupt(&mut self.rows[0].bytes);
            self.inspect();
        }}
        machine Main::inspect(&mut self) {{ }}
    "#
    );
    check(&source, false);
}

#[test]
fn locally_constructed_arrays_carry_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            consume(rows[0]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn copied_element_carries_its_constructed_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let r: Row = rows[0];
            consume(&r);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn copied_collection_carries_every_elements_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let copy: [Row; 2] = rows;
            consume(&copy[0]);
            consume(&copy[1]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn element_copy_through_assignment_carries_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let mut r: Row = rows[0];
            r = rows[1];
            consume(&r);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn corrupted_source_element_retires_the_copied_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let mut rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            corrupt(&mut rows[0].bytes);
            let r: Row = rows[0];
            consume(&r);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn copied_sibling_survives_a_corrupt_call_to_its_source_element() {
    // The copied value is independent storage: invalidating `rows[0]` after
    // the copy retires the source's evidence, never the copy's.
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let mut rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let r: Row = rows[1];
            corrupt(&mut rows[0].bytes);
            consume(&r);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn element_copied_after_a_sibling_write_still_proves() {
    // A write to a different element leaves `rows[1]`'s evidence live in its
    // own transfer context, so the copy still carries it.
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let mut rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            corrupt(&mut rows[0].bytes);
            rows[1].bytes = "okay";
            let r: Row = rows[1];
            consume(&r);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn copied_element_write_retires_its_own_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let mut r: Row = rows[0];
            corrupt(&mut r.bytes);
            consume(&r);
        }}
    "#
    );
    check(&source, false);
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
fn slice_view_of_field_carries_element_field_coverage() {
    // `level.rooms.as_slice()` lends the field's element storage to `rooms`:
    // `rooms[i]` is `level.rooms[i]`, so the declared element predicates
    // re-anchor below the view local.
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &Level) {{
            let rooms: &[Row] = level.rooms.as_slice();
            consume(&rooms[0]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn mutable_slice_view_carries_element_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            let view: &mut [Row] = rows.as_mut_slice();
            consume(&view[0]);
            consume(&view[1]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn slice_view_of_local_array_carries_constructed_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let view: &[Row] = rows.as_slice();
            consume(&view[1]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn slice_view_of_machine_field_carries_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ rows: [Row; 2]; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine Main::run(&mut self) {{
            let view: &[Row] = self.rows.as_slice();
            consume(&view[0]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn mutable_slice_view_of_machine_field_carries_element_coverage() {
    // The dungeon's shape: `self.rooms.as_mut_slice()` lends the attached
    // field's elements; a later indexed read still proves the field contract.
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ rows: [Row; 2]; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine Main::run(&mut self) {{
            let view: &mut [Row] = self.rows.as_mut_slice();
            consume(&view[0]);
            consume(&view[1]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn slice_view_element_coverage_flows_through_transitions() {
    let source = format!(
        r#"{DEFINITIONS}
        machine walk(rows: &mut [Row; 2]) {{
            let view: &[Row] = rows.as_slice();
            transition {{ _ -> next(&view[0]) }}
            state next(row: &Row) {{ }}
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn corrupted_element_rejects_its_slice_view() {
    // The corrupt call retires `rows[0]`'s evidence before the view binds, so
    // the transported view has nothing for that element.
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            corrupt(&mut rows[0].bytes);
            let view: &[Row] = rows.as_slice();
            consume(&view[0]);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn corrupted_sibling_view_preserves_the_other_elements_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            corrupt(&mut rows[0].bytes);
            let view: &[Row] = rows.as_slice();
            consume(&view[1]);
        }}
    "#
    );
    check_sibling_coverage_at_calls_and_corruption_at_return(&source, "rows[0].bytes");
}

#[test]
fn write_through_mutable_view_retires_the_views_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            let view: &mut [Row] = rows.as_mut_slice();
            view[0].bytes[0] = 255;
            consume(&view[0]);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn write_through_mutable_view_retires_the_source_coverage() {
    // `view[0]` IS `rows[0]`: the alias-closing write invalidation retires the
    // receiver-named fact too, so a later read of the source still rejects.
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2]) {{
            let view: &mut [Row] = rows.as_mut_slice();
            view[0].bytes[0] = 255;
            consume(&rows[0]);
        }}
    "#
    );
    check(&source, false);
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

#[test]
fn reassigned_view_drops_the_first_receivers_coverage() {
    // Rebinding `view` retires the facts rooted at it; `b`'s corrupted first
    // element supplies no evidence, so the call must reject rather than keep
    // `a`'s transported coverage.
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let mut a: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let mut b: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            let mut view: &[Row] = a.as_slice();
            corrupt(&mut b[0].bytes);
            view = b.as_slice();
            consume(&view[0]);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn slice_view_runtime_index_still_needs_element_coverage() {
    // A runtime index IS discharged by the transported whole-extent row -- but
    // only while that coverage is live. Corrupting one element retires the
    // source's `rows[0..usize::MAX].bytes` fact, so the view mints no whole
    // extent and the read still rejects.
    let source = format!(
        r#"{DEFINITIONS}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2], index: u64[0..2]) {{
            corrupt(&mut rows[0].bytes);
            let view: &[Row] = rows.as_slice();
            consume(&view[index]);
        }}
    "#
    );
    check(&source, false);
}

/// A view names the source's elements under the same indices, so a dynamic
/// floor proven on the receiver's element field (`rows[0].bytes.len > 0`)
/// re-anchors below the view (`view[0].bytes`) at the binding.
#[test]
fn view_write_into_slice_field_uses_the_receiver_elements_length_floor() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        machine caller(rows: &mut [SliceRow; 2]) requires rows[0].bytes.len > 0; {{
            let view: &mut [SliceRow] = rows.as_mut_slice();
            view[0].bytes[0] = 255;
        }}
    "#
    );
    check(&source, true);
}

/// The same re-anchoring applies when the view's receiver is a machine field
/// (`level.rooms`) rather than a bare parameter.
#[test]
fn view_write_into_slice_field_of_a_field_receiver() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        data Level {{ rooms: [SliceRow; 2]; }}
        machine caller(level: &mut Level) requires level.rooms[0].bytes.len > 0; {{
            let view: &mut [SliceRow] = level.rooms.as_mut_slice();
            view[0].bytes[0] = 255;
        }}
    "#
    );
    check(&source, true);
}

/// A requires clause that itself indexes the element's slice field records the
/// index proof; that proof re-anchors below the view too.
#[test]
fn view_write_into_slice_field_uses_the_elements_index_proof() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        machine caller(rows: &mut [SliceRow; 2], i: u64) requires rows[0].bytes[i] == 0; {{
            let view: &mut [SliceRow] = rows.as_mut_slice();
            view[0].bytes[i] = 255;
        }}
    "#
    );
    check(&source, true);
}

/// An owned copy carries the same bound-time element values, so the source's
/// nested slice-field floor holds below the copy as well.
#[test]
fn copied_collection_keeps_the_elements_slice_field_floor() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        machine caller(rows: [SliceRow; 2]) requires rows[0].bytes.len > 0; {{
            let mut copy: [SliceRow; 2] = rows;
            copy[0].bytes[0] = 255;
        }}
    "#
    );
    check(&source, true);
}

/// Without any length evidence for the element's slice field, the nested
/// write keeps its rejection — the view cannot invent a floor.
#[test]
fn view_write_into_slice_field_without_length_evidence_still_rejects() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        machine caller(rows: &mut [SliceRow; 2]) {{
            let view: &mut [SliceRow] = rows.as_mut_slice();
            view[0].bytes[0] = 255;
        }}
    "#
    );
    check_rejection(&source, false, "within unknown slice length");
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

/// Rebinding the view retires the facts rooted at it: the nested floor `a`'s
/// element supplied must not keep proving `view[0].bytes` once `view` names
/// `b`'s elements instead.
#[test]
fn reassigned_view_drops_the_first_receivers_nested_field_floor() {
    let source = format!(
        r#"{DEFINITIONS}
        data SliceRow {{ bytes: [u8]; tag: u64; }}
        machine caller(a: &mut [SliceRow; 2], b: &mut [SliceRow; 2])
            requires a[0].bytes.len > 0;
        {{
            let mut view: &mut [SliceRow] = a.as_mut_slice();
            view = b.as_mut_slice();
            view[0].bytes[0] = 255;
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

#[test]
fn returned_array_through_assignment_carries_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        {COPY_ROW}
        machine both(rows: &[CopyRow; 2]) -> [CopyRow; 2] {{ [rows[0], rows[1]] }}
        machine consume(row: &CopyRow) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &[CopyRow; 2]) {{
            let mut pair: [CopyRow; 2] = [CopyRow {{bytes: "okay", tag: 0}}, CopyRow {{bytes: "okay", tag: 1}}];
            pair = both(rows);
            consume(&pair[0]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn returned_element_into_local_carries_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        {COPY_ROW}
        machine pick(rows: &[CopyRow; 2]) -> CopyRow {{ rows[0] }}
        machine consume(row: &CopyRow) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &[CopyRow; 2]) {{
            let chosen: CopyRow = pick(rows);
            consume(&chosen);
        }}
    "#
    );
    check(&source, true);
}

/// A `&mut [Row]` return carries its element evidence onto the view binding,
/// and the binding's write origin supplies the referent's extent: `view` is
/// `level.rooms` lent element-for-element, so its slice length is 2 even
/// though the returned `&mut [Row]` type erases it.
#[test]
fn returned_mutable_slice_still_needs_its_length() {
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
            consume(&view[0]);
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

/// Rebinding the returned slice re-derives the extent from the NEW referent:
/// `view` naming a three-element array keeps the index a two-element one
/// could not prove, while the stale first extent must not.
#[test]
fn reassigned_returned_slice_takes_the_new_referents_extent() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        data Wing {{ cells: [Row; 3]; }}
        machine borrow_rows(level: &mut Level) -> &mut [Row] {{
            let slots: &mut [Row] = level.rooms.as_mut_slice();
            transition {{ _ -> slots }}
        }}
        machine borrow_cells(wing: &mut Wing) -> &mut [Row] {{
            let slots: &mut [Row] = wing.cells.as_mut_slice();
            transition {{ _ -> slots }}
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level, wing: &mut Wing) {{
            let mut view: &mut [Row] = borrow_rows(level);
            view = borrow_cells(wing);
            consume(&view[2]);
        }}
    "#
    );
    check(&source, true);
}

/// The same referent recovery covers byte slices: a returned `&mut [u8]`
/// bound to a plain `[u8; N]` field keeps the receiver's literal extent.
#[test]
fn returned_mutable_byte_slice_carries_the_referents_extent() {
    let source = format!(
        r#"{DEFINITIONS}
        data Buffer {{ data: [u8; 8]; }}
        machine borrow_data(buffer: &mut Buffer) -> &mut [u8] {{
            let bytes: &mut [u8] = buffer.data.as_mut_slice();
            transition {{ _ -> bytes }}
        }}
        machine caller(buffer: &mut Buffer) {{
            let view: &mut [u8] = borrow_data(buffer);
            view[7] = 1;
        }}
    "#
    );
    check(&source, true);
}

/// A referent that is itself an unknown-length slice lends no extent: the
/// lane transports the referent's length evidence, it does not invent one.
#[test]
fn returned_mutable_slice_without_a_resolved_extent_still_rejects() {
    let source = format!(
        r#"{DEFINITIONS}
        machine forward(rows: &mut [Row]) -> &mut [Row] {{
            transition {{ _ -> rows }}
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(input: &mut [Row]) {{
            let view: &mut [Row] = forward(input);
            consume(&view[0]);
        }}
    "#
    );
    check_rejection(&source, false, "within unknown slice length");
}

#[test]
fn directly_borrowed_element_into_local_carries_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let room: &mut Row = &mut level.rooms[0];
            consume(room);
        }}
    "#
    );
    check(&source, true);
}

/// A reference result has no storage of its own: the binding's write origin
/// recovers the referent `level.rooms[0]`, and live evidence below it
/// re-anchors below `room` through the same transport a direct borrow uses.
#[test]
fn returned_element_reference_expression_tail_carries_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            &mut level.rooms[0]
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let room: &mut Row = room_mut(level);
            consume(room);
        }}
    "#
    );
    check(&source, true);
}

/// The authored return tail desugars to a lone ordinary value transition;
/// the same referent recovery admits it only when no guarded sibling arm can
/// offer a second result.
#[test]
fn returned_element_reference_into_local_carries_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            let slots: &mut [Row] = level.rooms.as_mut_slice();
            transition {{ _ -> &mut slots[0] }}
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let room: &mut Row = room_mut(level);
            consume(room);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn returned_array_reference_into_local_carries_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine borrow_rooms(level: &mut Level) -> &mut [Row; 2] {{
            transition {{ _ -> &mut level.rooms }}
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let view: &mut [Row; 2] = borrow_rooms(level);
            consume(&view[0]);
            consume(&view[1]);
        }}
    "#
    );
    check(&source, true);
}

/// The returned reference is a loan, not a snapshot: corrupting the selected
/// element before the call leaves no live fact for the binding to inherit.
#[test]
fn returned_reference_to_a_corrupted_element_rejects() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            &mut level.rooms[0]
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            corrupt(&mut level.rooms[0].bytes);
            let room: &mut Row = room_mut(level);
            consume(room);
        }}
    "#
    );
    check(&source, false);
}

/// A write through the returned reference is a write to the referent: the
/// alias-closing invalidation retires the source-named evidence too.
#[test]
fn write_through_returned_reference_retires_source_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            &mut level.rooms[0]
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let room: &mut Row = room_mut(level);
            room.bytes[0] = 255;
            consume(&level.rooms[0]);
        }}
    "#
    );
    check(&source, false);
}

/// The same closure in reverse: a write to the referent retires the binding's
/// transported evidence rather than leaving a stale `room.bytes` fact.
#[test]
fn write_to_source_retires_the_returned_references_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            &mut level.rooms[0]
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level) {{
            let room: &mut Row = room_mut(level);
            level.rooms[0].bytes[0] = 255;
            consume(room);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn rebound_returned_reference_reanchors_to_the_new_source() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            &mut level.rooms[0]
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level, other: &mut Level) {{
            let mut room: &mut Row = room_mut(level);
            room = room_mut(other);
            consume(room);
        }}
    "#
    );
    check(&source, true);
}

/// Rebinding retires the evidence `room` inherited from `level`: a corrupted
/// element in the new referent supplies nothing, so the call must reject
/// rather than keep the first loan's coverage.
#[test]
fn rebound_returned_reference_to_a_corrupted_source_rejects() {
    let source = format!(
        r#"{DEFINITIONS}
        data Level {{ rooms: [Row; 2]; }}
        machine corrupt(bytes: &mut [u8; 4]) {{ bytes[0] = 255; }}
        machine room_mut(level: &mut Level) -> &mut Row {{
            &mut level.rooms[0]
        }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(level: &mut Level, other: &mut Level) {{
            corrupt(&mut other.rooms[0].bytes);
            let mut room: &mut Row = room_mut(level);
            room = room_mut(other);
            consume(room);
        }}
    "#
    );
    check(&source, false);
}

#[test]
fn returned_array_into_machine_field_carries_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        {COPY_ROW}
        data Main {{ pair: [CopyRow; 2]; }}
        machine both(rows: &[CopyRow; 2]) -> [CopyRow; 2] {{ [rows[0], rows[1]] }}
        machine consume(row: &CopyRow) ensures row.bytes in Utf8 {{ }}
        machine Main::run(&mut self, rows: &[CopyRow; 2]) {{
            self.pair = both(rows);
            consume(&self.pair[0]);
        }}
    "#
    );
    check(&source, true);
}

#[test]
fn element_copy_into_machine_field_carries_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        data Main {{ slot: Row; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine Main::run(&mut self) {{
            let rows: [Row; 2] = [Row {{bytes: "okay", tag: 0}}, Row {{bytes: "okay", tag: 1}}];
            self.slot = rows[0];
            consume(&self.slot);
        }}
    "#
    );
    check(&source, true);
}

/// A mutable collection view lends each element's declared-field coverage to
/// the binding, and one statement transports all of it. A write at one literal
/// index retires that coordinate alone: the transported facts sit in separate
/// per-place contexts, so a sibling element's coverage stays live for the
/// next call (the dungeon's `clear_level` shape: a slice view of the level's
/// rooms handed element by element to a mutating helper).
#[test]
fn view_literal_index_write_keeps_sibling_element_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine clear_row(row: &mut Row) {{ row.bytes = ""; row.tag = 0; }}
        machine caller(rows: &mut [Row; 2]) {{
            let view: &mut [Row] = rows.as_mut_slice();
            view[0].tag = 3;
            clear_row(&mut view[1]);
        }}
    "#
    );
    check(&source, true);
}

/// The per-place split does not widen the evidence: corrupting one element
/// through the view still retires exactly that element, so its own call
/// rejects while the untouched sibling's call is accepted and only the
/// caller's return reports the corrupted element.
#[test]
fn view_element_corruption_retires_only_that_element() {
    for (consumed, call_rejected) in [("view[1]", true), ("view[0]", false)] {
        let source = format!(
            r#"{DEFINITIONS}
            machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
            machine caller(rows: &mut [Row; 2]) {{
                let view: &mut [Row] = rows.as_mut_slice();
                let alias: &mut [u8; 4] = &mut view[1].bytes;
                alias[0] = 255;
                consume(&{consumed});
            }}
        "#
        );
        if call_rejected {
            check(&source, false);
        } else {
            check_sibling_coverage_at_calls_and_corruption_at_return(&source, "rows[1].bytes");
        }
    }
}

/// A runtime `rows[index]` read narrows coverage from the whole-extent row:
/// every element's declared fields are seeded once at `rows[0..usize::MAX]`,
/// so whichever element the selector names is covered. The declared index
/// range discharges the bounds half; the field coverage is what this proves.
/// Before whole-extent seeding this read had no element fact to borrow.
#[test]
fn runtime_index_read_carries_whole_extent_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &[Row; 2], index: u64[0..2]) {{
            consume(&rows[index]);
        }}
    "#
    );
    check(&source, true);
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

/// A `&mut rows[index]` return hands the caller whichever element storage the
/// selector named; the whole-extent row discharges the returned referent's
/// declared fields at the exit, and the `rows[0]` fallback narrows the same
/// way. This is the `find_room_mut` shape.
#[test]
fn mutable_reference_return_of_runtime_indexed_element_carries_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine pick(rows: &mut [Row; 2], index: u64[0..2], found: bool) -> &mut Row {{
            transition found {{
                true -> &mut rows[index]
                false -> &mut rows[0]
            }}
        }}
    "#
    );
    check(&source, true);
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

/// `let row: Row;` binds zeroed storage; the empty byte sequence satisfies
/// `Utf8`, so the declared field is live evidence from the binding and the
/// `&mut` out-parameter call discharges its entry rows. This is the
/// `find_room(level, cell, &mut out_room)` out-parameter shape.
#[test]
fn uninitialized_nominal_local_carries_zii_field_coverage() {
    let source = format!(
        r#"{DEFINITIONS}
        machine fill(out: &mut Row) {{ out.bytes = "okay"; out.tag = 0; }}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller() {{
            let row: Row;
            fill(&mut row);
            consume(&row);
        }}
    "#
    );
    check(&source, true);
}

/// The ZII seed is only the zero-value gate: a field whose domain the empty
/// byte sequence violates stays unproved until a write establishes it, so
/// `consume(&row)` on a fresh `NonEmpty` field still rejects.
#[test]
fn uninitialized_local_field_with_ungated_domain_still_requires_a_write() {
    let source = r#"domain [u8; 4]::NonEmpty requires non_empty(self);
        data Gated { bytes: [u8; 4] in NonEmpty; }
        machine consume(row: &Gated) ensures row.bytes in NonEmpty { }
        machine caller() {
            let row: Gated;
            consume(&row);
        }
    "#
    .to_string();
    check(&source, false);
}
