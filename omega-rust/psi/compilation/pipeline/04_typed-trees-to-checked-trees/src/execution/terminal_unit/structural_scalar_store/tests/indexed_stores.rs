use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;

/// The ordered stores of `Grid::update`'s entry state.
fn grid_stores(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Grid::update")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached grid signature");
    build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
    .expect("the grid stores compose")
}

fn runtime(depth: u32) -> checked_trees::CheckedUnitStructuralPathSegment {
    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
    )
}

#[test]
fn computed_u32_selector_retains_its_runtime_element() {
    let stores = grid_stores(
        r#"
        data Grid { cells: [u16; 12]; }
        machine Grid::update(&mut self, row: u32 [0..=2], column: u32 [0..=3], value: u16) {
            self.cells[row * 4 + column] = value;
        }
    "#,
    );
    let [CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { path, .. }] = stores.as_slice()
    else {
        panic!("one computed-index store: {stores:#?}");
    };
    assert_eq!(
        path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field("cells".into()),
            runtime(0),
        ]
    );
}

/// `mut` grants whole-binding replacement only, so an element store into an
/// established local array that is not `mut` keeps the local's own root and
/// literal element instead of declining for want of a parameter loan.
#[test]
fn an_element_store_into_a_local_array_needs_no_mut() {
    let checked = checked_program(
        r#"
        machine pad(title: [u8; 3]) {
            let padded: [u8; 4];
            padded[2] = title[1];
        }
    "#,
    );
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "pad")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine.symbol)
            .is_none(),
        "the element store composes into the machine's unit plan"
    );
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (structural, scalar) =
        crate::execution::terminal_unit::calls::free_structural_scalar_signature(
            program,
            &mut shapes,
            state,
            &machine_binders(program, machine),
        )
        .expect("free signature");
    let stores = build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
    .expect("the local element store composes");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            destination: checked_trees::CheckedStoreRoot::Local { .. },
            path,
            ..
        },
    ] = stores.as_slice()
    else {
        panic!("one owned-local element store: {stores:#?}");
    };
    assert_eq!(
        path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
            2
        )]
    );
}
