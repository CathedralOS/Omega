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

/// The caller's return re-proves every readable `&mut` referent's declared
/// field facts, so a corrupted element rejects there with its exact place
/// while every call in the body stays accepted: the sibling's coverage was
/// never disturbed.
fn check_sibling_coverage_at_calls_and_corruption_at_return(source: &str, place: &str) {
    let Err(diagnostics) =
        lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
    else {
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

/// A loop-carried element byte store keeps the carrier's declared predicate
/// live across the back-edge: the `cell -> col -> cell` cycle rejoins
/// `self.line`'s delivery every pass, and the stored byte's bound arrives
/// through the nested guarded loops (the same `i`/`k` bound chain that sets
/// `b`), so an early pass can deliver a weak set whose collapse the premise
/// channel alone could never repair. Each edge's delivery potential -- this
/// pass's own byte-class evidence -- is what the store's carrier premise
/// assumes, so the fixpoint still reaches the declared set rather than
/// starving on the collapsed join. This is the multiplication table's
/// `clear`/`cell_store` shape, where `self.line[self.o0] = self.tc` under
/// `emit_row`'s reader previously rejected `self.line requires [u8; N]::Utf8`.
#[test]
fn loop_carried_element_byte_stores_keep_the_carriers_declared_domain() {
    let source = r#"domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Main { line: [u8; 4] in Utf8; i: i32 in Wrapping; k: i32 in Wrapping; p: i32 in Wrapping; b: u8; }
        machine consume(text: &[u8; 4]) requires text in Utf8 { }
        machine digit(value: i32 in Wrapping) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }
        machine Main::run(&mut self) {
            self.line = "ok  ";
            self.i = 1;
            transition { _ -> row() }
            state row(&mut self) {
                transition self.i >= 1 && self.i <= 2 { true -> pinit() _ -> done() }
            }
            state pinit(&mut self) { self.k = 0; transition { _ -> col() } }
            state col(&mut self) {
                transition self.k >= 0 && self.k < 4 { true -> cell() _ -> bump() }
            }
            state cell(&mut self) {
                self.p = self.i * self.k;
                self.b = digit(self.p);
                self.line[self.k] = self.b;
                self.k = self.k + 1;
                transition { _ -> col() }
            }
            state bump(&mut self) { self.i = self.i + 1; transition { _ -> row() } }
            state done(&mut self) { consume(&self.line); }
        }
    "#
    .to_string();
    check(&source, true);
}

/// The potential premise is not a permission slip: a store whose byte leaves
/// the declared class refutes the candidate through its own edge -- its
/// delivery potential lacks the predicate, so the ceiling drops it, the mint
/// gate stays shut, and the join honestly retires the carrier's coverage.
#[test]
fn loop_carried_out_of_class_byte_store_retires_the_carriers_domain() {
    let source = r#"domain [u8; 4]::Utf8 requires valid_utf8(self);
        data Main { line: [u8; 4] in Utf8; k: i32 in Wrapping; }
        machine consume(text: &[u8; 4]) requires text in Utf8 { }
        machine Main::run(&mut self) {
            self.line = "ok  ";
            self.k = 0;
            transition { _ -> clear() }
            state clear(&mut self) {
                transition self.k >= 0 && self.k < 4 { true -> cell() _ -> done() }
            }
            state cell(&mut self) {
                self.line[self.k] = 255;
                self.k = self.k + 1;
                transition { _ -> clear() }
            }
            state done(&mut self) { consume(&self.line); }
        }
    "#
    .to_string();
    check_rejection(&source, false, "requires");
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
