use super::*;
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use checked_trees::types::PrimitiveType;

/// A plain `u64` selector that carries no declared range still stores
/// through a receiver array field when every transition edge into its
/// state proves the argument under a literal conjunct: `position <
/// module_name.len && position < 256 { true -> store(position) }` gives
/// `position < extent` edge-locally.
#[test]
fn guard_bounded_index_stores_through_receiver_array_field() {
    let source = r#"
        data Ring { cells: [u16; 4]; count: u64; }
        machine Ring::poke(&mut self, position: u64, value: u16) {
            transition position < self.count && position < 4 {
                true -> store(position, value)
            }
            transition { _ -> done() }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
            state done(&mut self) {}
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
    .expect("attached guarded state signature");
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
    .expect("guard-bounded state index produces an indexed store");
    let [CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore { path, index, .. }] =
        stores.as_slice()
    else {
        panic!("one indexed store");
    };
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
}

/// The same edge-local proof works when the caller is a sibling state: the
/// argument is the caller's parameter and the guard still bounds it
/// literally — the shape `copy -> store` loops carry in the Squalr set_*
/// copy machines.
#[test]
fn guard_bounded_index_survives_an_intermediate_state_edge() {
    let source = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::fill(&mut self, value: u16) {
            transition { _ -> step(0, value) }
            state step(&mut self, position: u64, value: u16) {
                transition position < 4 {
                    true -> store(position, value)
                }
                transition { _ -> done() }
            }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
            state done(&mut self) {}
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Ring::fill")
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "store")
        .unwrap();
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached guarded state signature");
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
        .is_some(),
        "edge guard `position < 4` bounds the state-carried index",
    );
}

/// The selector need not be the first non-`self` parameter: `store_digit`
/// takes `remaining` before `digit_index`, and the guarded argument is
/// matched by position among the non-self parameters. A machine-level
/// `self.m(...)` re-entry transition inside the state is not an edge into
/// it and does not poison the proof.
#[test]
fn guard_bounded_index_resolves_the_argument_by_parameter_position() {
    let source = r#"
        data T { bytes: [u8; 16]; len: u64; }
        machine T::put_digit(&mut self, value: u8, digit_index: u64) {
            transition digit_index < 4 {
                true -> store_digit(value, digit_index)
                false -> done()
            }
            state store_digit(&mut self, value: u8, digit_index: u64) {
                self.bytes[digit_index] = value;
                self.len = ((digit_index as u64 in Wrapping) + (1 as u64 in Wrapping)) as u64;
                transition digit_index < 3 {
                    true -> self.put_digit(value, ((digit_index as u64 in Wrapping) + (1 as u64 in Wrapping)) as u64)
                    false -> done()
                }
            }
            state done(&mut self) {}
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "T::put_digit")
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
    .expect("second-parameter index bounded by the edge guard");
    let Some(CheckedUnitEffectOperationPlan::WriteOnlyIndexedPrimitiveStore { index, .. }) =
        stores.first()
    else {
        panic!("indexed store leads the sequence");
    };
    assert!(matches!(
        index,
        checked_trees::CheckedScalarExpression::Parameter {
            position: 1,
            primitive_type: PrimitiveType::U64,
        }
    ));
}

fn stores_for_store_state(checked: &checked_trees::CheckedTrees) -> bool {
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Ring::poke")
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "store")
        .unwrap();
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
    .is_some()
}

/// The indexed-access checker proves bounds through its whole-machine
/// meet — propagated declared ranges, literal arguments, and interval
/// arithmetic — while this admission only certifies a literal conjunct on
/// each incoming edge. Bounds proven only through those channels stay
/// declined here (under-bounded edges never reach this gate at all: the
/// checker's meet rejects them first).
#[test]
fn guard_bounded_index_declines_bounds_the_edge_does_not_carry() {
    // A declared range on the CALLER's parameter flows through the meet
    // but appears in no guard conjunct on the edge into `store`.
    let declared_on_caller = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> fill(0, value) }
            state fill(&mut self, position: u64 [0..4], value: u16) {
                transition { _ -> store(position, value) }
            }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
        }
    "#;
    assert!(
        !stores_for_store_state(&checked_program(declared_on_caller)),
        "caller-declared bound is a meet fact, not an edge conjunct",
    );

    // `position + 1` is bounded by interval arithmetic on `position < 3`;
    // the edge conjunct does not spell the argument expression itself.
    let arithmetic_argument = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> fill(0, value) }
            state fill(&mut self, position: u64, value: u16) {
                transition position < 3 {
                    true -> store(position + 1, value)
                }
                transition { _ -> done() }
            }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
            state done(&mut self) {}
        }
    "#;
    assert!(
        !stores_for_store_state(&checked_program(arithmetic_argument)),
        "an argument expression is not the conjunct's named operand",
    );

    // A constant argument is trivially in-extent but carries no conjunct.
    let literal_argument = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> store(0, value) }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
        }
    "#;
    assert!(
        !stores_for_store_state(&checked_program(literal_argument)),
        "a literal argument is not a guard-bounded index",
    );
}
