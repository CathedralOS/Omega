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
