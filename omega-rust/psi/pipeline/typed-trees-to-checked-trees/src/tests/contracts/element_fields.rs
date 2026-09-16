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
    check(&source, true);
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
    check(&source, true);
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
    // Element evidence lives at literal FixedIndex places; a runtime index is
    // not yet discharged by the transported facts.
    let source = format!(
        r#"{DEFINITIONS}
        machine consume(row: &Row) ensures row.bytes in Utf8 {{ }}
        machine caller(rows: &mut [Row; 2], index: u64[0..2]) {{
            let view: &[Row] = rows.as_slice();
            consume(&view[index]);
        }}
    "#
    );
    check(&source, false);
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
/// but the returned slice type loses the receiver's literal length, so
/// indexing it still hits the ranges gap this leg does not close.
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
