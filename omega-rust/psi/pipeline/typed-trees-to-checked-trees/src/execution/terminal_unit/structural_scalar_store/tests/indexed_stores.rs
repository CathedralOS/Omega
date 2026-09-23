use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use checked_trees::types::PrimitiveType;

/// An index read off a state-carried parameter cannot cite the entry
/// contract's integer ranges: the selector's own declared integer range is
/// the bound, so `self.cells[position]` in a non-entry state stores through
/// the indexed op with the selector kept as an operand.
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
    .expect("declared-range state index produces an indexed store");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore {
            destination,
            path,
            index,
            value,
            ..
        },
    ] = stores.as_slice()
    else {
        panic!("one indexed store");
    };
    assert!(matches!(
        destination,
        checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 }
    ));
    assert!(matches!(
        path.as_slice(),
        [checked_trees::CheckedUnitStructuralPathSegment::Field(identity)] if identity == "cells"
    ));
    assert!(matches!(
        index,
        checked_trees::CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: PrimitiveType::U64,
        }
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

/// Bounds the declaration does not carry stay declined in a non-entry
/// state: a plain `u64` selector and declared ranges whose maximum does
/// not fit the array both fail closed.
#[test]
fn state_carried_index_without_a_fitting_declared_bound_stays_declined() {
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
        assert!(
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
            .is_none(),
            "position: {position} cannot prove index < extent in a non-entry state",
        );
    }
}
