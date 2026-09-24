use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use checked_trees::types::PrimitiveType;

/// An index read off a state-carried parameter stores through the primitive
/// store whose path ends at the selected element: `self.cells[position]` is
/// `[cells, RuntimeIndex(AssignmentIndex)]`, never the selector as an operand.
#[test]
fn state_carried_declared_range_index_stores_through_receiver_array_field() {
    let source = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> store(0, value) }
            state store(&mut self, position: u64 [0..4], value: u16) {
                self.cells[position] = value;
            }
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Ring::poke")
        .unwrap();
    let state = &program.machine_states(machine)[1];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached ranged state signature");
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
    .expect("declared-range state index produces a primitive element store");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
            ..
        },
    ] = stores.as_slice()
    else {
        panic!("one primitive store");
    };
    assert!(matches!(
        destination,
        checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 }
    ));
    assert!(matches!(
        path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity),
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex
            ),
        ] if identity == "cells"
    ));
    assert!(matches!(
        value,
        checked_trees::CheckedCallScalarArgument::Pure(
            checked_trees::CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U16,
            }
        )
    ));
}

/// The planner proves no bound for a runtime element: a plain `u64`
/// selector and declared ranges whose maximum does not fit the array (the
/// checker accepts them here through the literal argument's meet) still
/// retain the element, and Terminal re-proves `index < extent` from the
/// facts that reach the store or rejects it.
#[test]
fn state_carried_index_retains_its_runtime_element_whatever_its_declared_bound() {
    for position in ["u64", "u64 [0..=4]", "u64 [0..5]"] {
        let source = format!(
            r#"
            data Ring {{ cells: [u16; 4]; }}
            machine Ring::poke(&mut self, value: u16)
            {{
                transition {{ _ -> store(0, value) }}
                state store(&mut self, position: {position}, value: u16) {{
                    self.cells[position] = value;
                }}
            }}
        "#
        );
        let checked = checked_program(&source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Ring::poke")
            .unwrap();
        let state = &program.machine_states(machine)[1];
        let mut shapes = ShapeCollector::new(program);
        let (_, structural, scalar) = structural_scalar_signature(
            program,
            &mut shapes,
            machine,
            state,
            &machine_binders(program, machine),
            true,
        )
        .expect("attached state signature");
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
        .unwrap_or_else(|| panic!("position: {position} retains its runtime element"));
        assert!(
            matches!(
                stores.as_slice(),
                [CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { path, .. }]
                    if matches!(path.last(), Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
            ),
            "position: {position} stores through its runtime element: {stores:#?}"
        );
    }
}
