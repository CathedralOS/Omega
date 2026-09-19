use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

#[test]
fn boundary_witness_survives_disjoint_internal_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_other();
            self.small = self.n;
        }

        machine Main::touch_other(&mut self) {
            self.other = 1;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a disjoint internal frame should preserve the boundary range witness");
}

#[test]
fn boundary_witness_survives_disjoint_recast_local_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_other_through_recast();
            self.small = self.n;
        }

        machine Main::touch_other_through_recast(&mut self) {
            let view: &mut f32 = &mut self.other as &mut f32;
            view = 1.0;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("an exact mutable-recast frame should preserve a disjoint boundary range witness");
}

#[test]
fn boundary_witness_dies_under_overlapping_recast_local_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_n_through_recast();
            self.small = self.n;
        }

        machine Main::touch_n_through_recast(&mut self) {
            let view: &mut f32 = &mut self.n as &mut f32;
            view = 9.0;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping mutable-recast frame must invalidate the range witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_local_alias_call_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_other_through_alias();
            self.small = self.n;
        }

        machine Main::touch_other_through_alias(&mut self) {
            let root: &mut u32 = &mut self.other;
            let alias: &mut u32 = &mut root;
            transition { _ -> finish(alias) }
            state finish(&mut self, value: &mut u32) {
                value = 1;
            }
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "an exact named-transition alias frame should preserve the disjoint boundary range witness",
    );
}

#[test]
fn boundary_witness_dies_when_internal_call_frame_writes_place() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_n();
            self.small = self.n;
        }

        machine Main::touch_n(&mut self) {
            self.n = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping internal frame must invalidate the range witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_dies_when_local_alias_call_frame_writes_place() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_n_through_alias();
            self.small = self.n;
        }

        machine Main::touch_n_through_alias(&mut self) {
            let root: &mut u32 = &mut self.n;
            let alias: &mut u32 = &mut root;
            transition { _ -> finish(alias) }
            state finish(&mut self, value: &mut u32) {
                value = 9;
            }
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "an overlapping named-transition alias frame must invalidate the range witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_projected_alias_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            size: u32;
            other: u32;
        }

        data Main {
            fw: Firmware;
            cell: Cell;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.cell.size);
            self.touch_other_through_projection();
            self.small = self.cell.size;
        }

        machine Main::touch_other_through_projection(&mut self) {
            let cell_alias: &mut Cell = &mut self.cell;
            let other: &mut u32 = &mut cell_alias.other;
            other = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("an exact projected-alias frame should preserve a witness on a disjoint sibling");
}

#[test]
fn boundary_witness_dies_under_overlapping_projected_alias_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            size: u32;
            other: u32;
        }

        data Main {
            fw: Firmware;
            cell: Cell;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.cell.size);
            self.touch_size_through_projection();
            self.small = self.cell.size;
        }

        machine Main::touch_size_through_projection(&mut self) {
            let cell_alias: &mut Cell = &mut self.cell;
            let size: &mut u32 = &mut cell_alias.size;
            size = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping projected-alias frame must invalidate the range witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_member_indexed_alias_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Group {
            cells: [u32; 2];
            other: u32;
        }

        data Main {
            fw: Firmware;
            group: Group;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.group.other);
            self.touch_cells_through_member_index();
            self.small = self.group.other;
        }

        machine Main::touch_cells_through_member_index(&mut self) {
            let group_alias: &mut Group = &mut self.group;
            let cell_alias: &mut u32 = &mut group_alias.cells[0];
            cell_alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "the indexed projection should retain its intermediate collection and preserve a sibling witness",
    );
}

#[test]
fn boundary_witness_dies_under_member_indexed_alias_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Group {
            cells: [u32; 2];
            other: u32;
        }

        data Main {
            fw: Firmware;
            group: Group;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.group.cells[0]);
            self.touch_cells_through_member_index();
            self.small = self.group.cells[0];
        }

        machine Main::touch_cells_through_member_index(&mut self) {
            let group_alias: &mut Group = &mut self.group;
            let cell_alias: &mut u32 = &mut group_alias.cells[1];
            cell_alias = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "the retained intermediate collection must invalidate an overlapping indexed witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_direct_member_after_index_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            value: u32;
        }

        data Main {
            fw: Firmware;
            cells: [Cell; 2];
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.other);
            self.touch_direct_member_after_index();
            self.small = self.other;
        }

        machine Main::touch_direct_member_after_index(&mut self) {
            let value: &mut u32 = &mut self.cells[0].value;
            value = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a direct member-after-index frame should preserve a witness outside its collection",
    );
}

#[test]
fn boundary_witness_dies_under_direct_member_after_index_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Cell {
            value: u32;
        }

        data Main {
            fw: Firmware;
            cells: [Cell; 2];
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.cells[0].value);
            self.touch_direct_member_after_index();
            self.small = self.cells[0].value;
        }

        machine Main::touch_direct_member_after_index(&mut self) {
            let value: &mut u32 = &mut self.cells[1].value;
            value = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "the direct member-after-index frame must invalidate an overlapping collection witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_caller_isolated_local_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_local_collection();
            self.small = self.n;
        }

        machine Main::touch_local_collection(&mut self) {
            let values: [u32; 2] = [0, 1];
            let alias: &mut u32 = &mut values[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "writes through a reference-free local collection must not invalidate caller facts",
    );
}

#[test]
fn boundary_witness_survives_transparently_forwarded_local_collection() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            small: u32 [0..=8];
        }

        machine return_values(values: &mut [u32; 2]) -> &mut [u32; 2] {
            values
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_computed_local_collection();
            self.small = self.n;
        }

        machine Main::touch_computed_local_collection(&mut self) {
            let local: [u32; 2] = [0, 1];
            let values: &mut [u32; 2] = return_values(&mut local);
            let alias: &mut u32 = &mut values[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a transparent helper preserves the caller-isolated origin of a local collection");
}

#[test]
fn boundary_witness_survives_transparent_call_result_alias_chain() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine identity_values(values: &mut [u32; 2]) -> &mut [u32; 2] {
            values
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_values_through_identity_calls();
            self.small = self.n;
        }

        machine Main::touch_values_through_identity_calls(&mut self) {
            let first: &mut [u32; 2] = identity_values(&mut self.values);
            let second: &mut [u32; 2] = identity_values(first);
            let alias: &mut u32 = &mut second[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a direct identity-result chain should preserve a witness outside its argument origin",
    );
}

#[test]
fn boundary_witness_survives_transparent_result_with_pure_call_scratch() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine make_scratch() -> u32 {
            0
        }

        machine values_after_scratch(values: &mut [u32; 2]) -> &mut [u32; 2] {
            let scratch: u32 = make_scratch();
            values
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_values_after_scratch();
            self.small = self.n;
        }

        machine Main::touch_values_after_scratch(&mut self) {
            let selected: &mut [u32; 2] = values_after_scratch(&mut self.values);
            selected[0] = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a complete empty call frame for isolated scratch must preserve the returned origin",
    );
}

#[test]
fn boundary_witness_dies_when_transparent_result_scratch_call_writes_it() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine overwrite(value: &mut u32) -> u32 {
            value = 9;
            0
        }

        machine values_after_write(
            values: &mut [u32; 2],
            witness: &mut u32
        ) -> &mut [u32; 2] {
            let scratch: u32 = overwrite(witness);
            values
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_values_and_n();
            self.small = self.n;
        }

        machine Main::touch_values_and_n(&mut self) {
            let selected: &mut [u32; 2] =
                values_after_write(&mut self.values, &mut self.n);
            selected[0] = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a nonempty scratch-call frame must invalidate its written witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_projected_call_result_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            n: u32;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine first_value(values: &mut [u32; 2]) -> &mut u32 {
            &mut values[0]
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.n);
            self.touch_projected_call_result();
            self.small = self.n;
        }

        machine Main::touch_projected_call_result(&mut self) {
            let value: &mut u32 = first_value(&mut self.values);
            value = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a direct projected call result should preserve a witness outside its argument origin",
    );
}

#[test]
fn boundary_witness_dies_under_projected_call_result_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            values: [u32; 2];
            small: u32 [0..=8];
        }

        machine first_value(values: &mut [u32; 2]) -> &mut u32 {
            &mut values[0]
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.values[1]);
            self.touch_projected_call_result();
            self.small = self.values[1];
        }

        machine Main::touch_projected_call_result(&mut self) {
            let value: &mut u32 = first_value(&mut self.values);
            value = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source)).expect_err(
        "a projected indexed call result must invalidate an overlapping collection witness",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_witness_survives_disjoint_indexed_alias_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            cells: [u32; 2];
            other: u32;
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.other);
            self.touch_cells_through_indexed_alias();
            self.small = self.other;
        }

        machine Main::touch_cells_through_indexed_alias(&mut self) {
            let alias: &mut u32 = &mut self.cells[0];
            alias = 9;
        }
    "#;

    lower_typed_trees(parse_typed_trees(source)).expect(
        "a collection-coarse indexed alias frame should preserve a witness on disjoint storage",
    );
}

#[test]
fn boundary_witness_dies_under_indexed_alias_collection_frame() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }

        data Main {
            fw: Firmware;
            cells: [u32; 2];
            small: u32 [0..=8];
        }

        machine Main::main(&mut self) reaches Firmware {
            self.fw.get_size(&mut self.cells[0]);
            self.touch_cells_through_indexed_alias();
            self.small = self.cells[0];
        }

        machine Main::touch_cells_through_indexed_alias(&mut self) {
            let alias: &mut u32 = &mut self.cells[1];
            alias = 9;
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the whole collection frame must invalidate an indexed boundary witness");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn incoming_guard_conjuncts_jointly_bound_operands_after_disjoint_write() {
    let source = r#"
        data Main {
            scratch: i32;
            position: i32 [0..=8];
            direction: i32;
            result: i32 [0..=9];
        }

        machine Main::main(&mut self) {
            transition self.scratch == 0 && self.direction >= 0 && self.direction <= 1 {
                true -> update()
                false -> done()
            }
            state update(&mut self) {
                self.scratch = 1;
                self.result = self.position + self.direction;
            }
            state done(&mut self) {}
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("surviving lower and upper conjuncts must jointly bound the operand");
}

#[test]
fn incoming_guard_conjuncts_do_not_revive_invalidated_or_unestablished_facts() {
    for (guard, prior_write) in [
        ("self.value < 16 && self.signed > -5", "self.signed = -5;"),
        (
            "self.value < 16 && self.signed > -5",
            "self.reset_signed();",
        ),
        ("self.value < 16 || self.signed > -5", "self.value = 16;"),
        ("!(self.value < 16 && self.signed > -5)", "self.value = 16;"),
    ] {
        let source = format!(
            r#"
            data Main {{
                value: i32 [0..=16];
                signed: i8 [-5..=5];
            }}

            machine Main::main(&mut self) {{
                transition {guard} {{
                    true -> update()
                    false -> done()
                }}
                state update(&mut self) {{
                    {prior_write}
                    self.signed = self.signed - 1;
                }}
                state done(&mut self) {{}}
            }}

            machine Main::reset_signed(&mut self) {{ self.signed = -5; }}
            "#
        );
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("an invalidated or unestablished conjunct cannot justify the subtraction");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove assignment value `self.signed - 1`")),
            "expected bounded assignment rejection for {guard} after {prior_write}, got {diagnostics:#?}"
        );
    }
}

#[test]
fn incoming_guard_survives_pure_value_call_before_bounded_assignment() {
    let source = r#"
        machine widen(value: i32) -> i64 {
            value as i64
        }

        data Main {
            i: i32 [0..=2];
            scratch: i64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition { _ -> step() }

            state step(&mut self) {
                transition self.i < 2 { true -> add() _ -> done() }
            }

            state add(&mut self) {
                self.scratch = widen(self.i);
                self.i = self.i + 1;
                transition { _ -> step() }
            }

            state done(&mut self) {}
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a pure value-call frame should preserve the incoming counter guard");
}

#[test]
fn incoming_guard_dies_when_value_call_writes_guarded_place() {
    let source = r#"
        data Main {
            i: i32 [0..=2];
            scratch: i64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition { _ -> step() }

            state step(&mut self) {
                transition self.i < 2 { true -> add() _ -> done() }
            }

            state add(&mut self) {
                self.scratch = self.touch_i();
                self.i = self.i + 1;
                transition { _ -> step() }
            }

            state touch_i(&mut self) -> i64 {
                self.i = 2;
                0
            }

            state done(&mut self) {}
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping value-call frame must invalidate the incoming guard");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected the bounded-assignment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn incoming_guard_survives_the_consuming_assignment_destination_write() {
    let source = r#"
        data Main { value: i32 [0..=9]; }

        machine Main::main(&mut self) {
            transition self.value < 9 {
                true -> update()
                false -> done()
            }
            state update(&mut self) {
                self.value = self.value + 1;
            }
            state done(&mut self) {}
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("the destination is written after the bounded value has been consumed");
}

#[test]
fn incoming_guard_dies_under_the_consuming_assignment_value_call() {
    let source = r#"
        data Main {
            value: i32 [0..=9];
            result: i32 [0..=9];
        }

        machine Main::main(&mut self) {
            transition self.value < 9 {
                true -> update()
                false -> done()
            }
            state update(&mut self) {
                self.result = self.touch_value() + self.value;
            }
            state done(&mut self) {}
        }

        machine Main::touch_value(&mut self) -> i32 [1..=1] {
            self.value = 9;
            1
        }
    "#;

    let pure_call = source.replace("self.value = 9;", "");
    lower_typed_trees(parse_typed_trees(&pure_call))
        .expect("the same bounded call result preserves the guard when its frame is pure");
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the consuming value call invalidates the incoming guard before addition");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove assignment value")),
        "expected bounded-assignment rejection, got {diagnostics:#?}"
    );
}

#[test]
fn bounded_byte_domain_membership_projects_to_matching_slice_domain() {
    let source = r#"
        boundary trait Sink {
            machine write(text: [u8] in Utf8);
        }

        domain [u8]::Utf8
        requires
            valid_utf8(self);

        domain [u8; 4]::Utf8
        requires
            valid_utf8(self);

        data Main {
            sink: Sink;
            text: [u8; 4] in Utf8;
        }

        machine Main::main(&mut self) reaches Sink {
            self.text = "Gate";
            self.sink.write(self.text);
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a bounded Utf8 carrier should carry Utf8 through its slice projection");
}

#[test]
fn bounded_byte_domain_projection_proves_the_requested_predicate_independently() {
    let source = r#"
        boundary trait Sink {
            machine write(text: [u8] in Text);
        }

        domain [u8]::Text
        requires
            no_nul(self);

        domain [u8; 4]::Text
        requires
            valid_utf8(self);

        data Main {
            sink: Sink;
            text: [u8; 4] in Text;
        }

        machine Main::main(&mut self) reaches Sink {
            self.text = "G\x00te";
            self.sink.write(self.text);
        }
    "#;

    lower_typed_trees(parse_typed_trees(&source.replace(r"G\x00te", "Gate")))
        .expect("known bytes may independently establish both domain predicates");
    let Err(diagnostics) = lower_typed_trees(parse_typed_trees(source)) else {
        panic!("a carrier projection must not conflate different domain theories");
    };
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call write")
                && diagnostic.message.contains("self.text in [u8]::Text")
        }),
        "expected the mismatched slice-domain requirement to remain unproven, got {diagnostics:#?}"
    );
}
