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
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => assert!(
            accepted,
            "an unproved collection element crossed the boundary:\n{source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("cannot prove default-domain field requirement")
                }),
                "the element coverage obligation must reject, not a side channel: {diagnostics:#?}\n{source}"
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
