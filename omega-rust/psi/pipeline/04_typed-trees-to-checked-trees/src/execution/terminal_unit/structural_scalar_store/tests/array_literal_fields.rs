use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;

fn stores(source: &str, machine_name: &str) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
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
    .unwrap();
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
}

/// An element that reads storage could observe an earlier element's store,
/// so a literal built from the destination's own elements keeps declining.
#[test]
fn an_array_literal_reading_storage_is_not_split_into_element_stores() {
    let stores = stores(
        r#"
        data Board { cells: [u16; 3]; }
        machine Board::rotate(&mut self) {
            self.cells = [self.cells[1], self.cells[2], self.cells[0]];
        }
    "#,
        "Board::rotate",
    );
    assert!(
        stores.is_none_or(|stores| !stores.iter().any(|store| matches!(
            store,
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
        ))),
        "a storage-reading element never becomes an element store"
    );
}
