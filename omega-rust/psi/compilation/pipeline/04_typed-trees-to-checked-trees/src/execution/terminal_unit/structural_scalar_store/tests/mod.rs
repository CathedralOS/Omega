use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders};

use super::{build_structural_scalar_field_store_sequence, frame};
use crate::execution::terminal_unit::calls::structural_signature;
use crate::tests::front_end::{checked_program, typed_program};
use checked_trees::{CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan};

mod arithmetic_policies;
mod array_literal_fields;
mod borrowed_arrays;
mod borrowed_records;
mod byte_view_writes;
mod call_results;
mod case_field_stores;
mod closed_generic_records;
mod computations;
mod computed_store_values;
mod domain_field_stores;
mod guard_bounded_stores;
mod indexed_stores;
mod local_float_fields;
mod record_literal_fields;
mod reference_field_stores;
mod value_root_stores;
mod where_field_stores;

#[test]
fn array_stores_before_terminal_machine_call_keep_a_complete_write_frame() {
    let checked = checked_program(
        r#"
        boundary trait Output { machine observe(value: i32) reaches Output; }
        data Main { arr: [i32; 3]; }
        machine Main::main(&mut self) {
            self.arr[0] = 10;
            self.arr[1] = 20;
            self.arr[2] = 30;
            transition { _ -> self.pick(self.arr, 1) }
        }
        machine Main::pick(&mut self, arr: [i32; 3], k: u64) reaches Output
        requires k <= 2 {
            let value: i32 = arr[k];
            transition value == 20 {
                true -> yes()
                false -> no()
            }
            state yes(&mut self) { Output::observe(1); }
            state no(&mut self) { Output::observe(2); }
        }
    "#,
    );
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let recorded = &checked
        .facts
        .mutation
        .for_machine(machine.symbol)
        .unwrap()
        .state_write_frames[0]
        .frame;
    assert!(recorded.complete_paths().is_some(), "{recorded:?}");
    assert!(
        frame::matches(program, machine, state, recorded, None),
        "{recorded:?}"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .any(|plan| plan.machine == machine.symbol),
        "array stores and terminal invocation must form one executable Unit plan: {:?}",
        checked.facts.flow.terminal_unit_effects.omissions
    );
}

#[test]
fn array_byte_fields_do_not_admit_stored_borrows_or_nominal_drop() {
    // An array layout describes storage only; each operation keeps its own
    // ownership decision. Every `[Cell; 2]` layout composes, partial-affine
    // disposal refuses stored borrows, and automatic disposal refuses a
    // nominal `drop` reachable through the elements. The last row is the
    // owned control without `drop`.
    for (extra, drop, owned_contents, nominal_drop) in [
        ("view: &[u8];", "", false, false),
        ("reference: &i32;", "", false, false),
        ("", "machine Cell::drop(&mut self) {}", true, true),
        ("", "", true, false),
    ] {
        let source = format!(
            r#"
            domain [u8;3]::Utf8 requires valid_utf8(self);
            data Cell {{ out: [u8;3] in Utf8; {extra} }}
            data Record {{ cells: [Cell;2]; }}
            {drop}
            machine Record::observe(&self) {{}}
        "#
        );
        let program = typed_program(&source);
        let record = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Record")
            .unwrap();
        let [typed_trees::data::DataMember::Field(cells)] = program.data_members(record) else {
            panic!("one array field");
        };
        let mut shapes = ShapeCollector::new(&program);
        assert!(
            shapes.add_attached_data(record, &[]).is_some(),
            "array layout composes: {extra} {drop}"
        );
        assert_eq!(
            shapes
                .add_partial_affine_type(cells.type_reference, &[])
                .is_some(),
            owned_contents,
            "a layout cannot authorize partial disposal of stored borrows: {extra} {drop}"
        );
        assert_eq!(
            crate::execution::terminal_unit::types::type_graph_requires_nominal_drop(
                &program,
                cells.type_reference
            ),
            nominal_drop,
            "element cleanup reaches the nominal drop: {extra} {drop}"
        );
        if !extra.is_empty() {
            continue;
        }
        // Discarding the whole record must run each element's cleanup: with
        // a nominal `drop` there is no Unit plan, only a recorded omission.
        let checked = checked_program(&format!("{source}\nmachine discard(record: Record) {{}}"));
        let discard = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "discard")
            .unwrap()
            .symbol;
        let effects = &checked.facts.flow.terminal_unit_effects;
        assert_eq!(
            effects.machines.iter().any(|plan| plan.machine == discard),
            !nominal_drop,
            "{drop}: {:?}",
            effects.omissions
        );
        assert_eq!(
            effects
                .omissions
                .iter()
                .any(|omission| omission.machine == discard),
            nominal_drop,
            "{drop}: {:?}",
            effects.omissions
        );
    }
}

#[test]
fn byte_field_sequence_rejects_missing_extra_and_opaque_write_frames() {
    let source = r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; flag: bool; }
        machine Record::replace(&mut self) { self.out = "XXX"; self.flag = true; }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Record::replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, parameters) =
        structural_signature(program, &mut shapes, machine, state, &[], true).unwrap();
    let stores = build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &parameters,
        &[],
        0,
        None,
    )
    .unwrap();
    assert!(matches!(
        stores.as_slice(),
        [
            CheckedUnitEffectOperationPlan::ByteSequenceStore(_),
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
        ]
    ));
    for replacement in [
        facts::NormalizedWriteFrame::complete(vec!["self.out".into()]),
        facts::NormalizedWriteFrame::complete(vec!["self.flag".into()]),
        facts::NormalizedWriteFrame::complete(vec![
            "self.flag".into(),
            "self.out".into(),
            "self.absent".into(),
        ]),
        facts::NormalizedWriteFrame::opaque(),
    ] {
        let mut changed = checked.facts.clone();
        changed
            .mutation
            .machines
            .iter_mut()
            .find(|fact| fact.machine == machine.symbol)
            .unwrap()
            .state_write_frames[0]
            .frame = replacement;
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &changed,
                machine,
                state,
                &parameters,
                &[],
                0,
                None,
            )
            .is_none()
        );
    }
}

#[test]
fn ordered_stores_replay_successor_writes_and_reject_modified_frames() {
    let source = r#"
        data Flags { first: bool; second: bool; }
        machine Flags::run(&mut self) {
            self.first = false;
            transition { _ -> update() }
            state update(&mut self) {
                self.second = true;
                transition self.first { true -> run() _ -> done() }
            }
            state done(&mut self) {}
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Flags::run")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, parameters) =
        structural_signature(program, &mut shapes, machine, state, &[], true).unwrap();
    let frame = &checked
        .facts
        .mutation
        .for_machine(machine.symbol)
        .unwrap()
        .state_write_frames[0]
        .frame;
    assert_eq!(
        frame.complete_paths().unwrap(),
        ["self.first", "self.second"]
    );
    let stores = build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &parameters,
        &[],
        0,
        None,
    )
    .expect("entry store retains the complete successor frame");
    assert_eq!(
        stores.len(),
        1,
        "only this state's local assignment is emitted"
    );
    for replacement in [
        facts::NormalizedWriteFrame::complete(vec!["self.first".into()]),
        facts::NormalizedWriteFrame::complete(vec![
            "self.first".into(),
            "self.second".into(),
            "self.absent".into(),
        ]),
        facts::NormalizedWriteFrame::opaque(),
    ] {
        let mut changed = checked.facts.clone();
        changed
            .mutation
            .machines
            .iter_mut()
            .find(|fact| fact.machine == machine.symbol)
            .unwrap()
            .state_write_frames[0]
            .frame = replacement;
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &changed,
                machine,
                state,
                &parameters,
                &[],
                0,
                None,
            )
            .is_none(),
            "missing, extra, or opaque successor writes cannot authorize stores"
        );
    }
}
