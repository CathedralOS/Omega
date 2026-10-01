use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;

fn stores(source: &str, state_index: usize) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program.machines().iter().next().unwrap();
    let state = &program.machine_states(machine)[state_index];
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
}

/// A policy cast chain ending in an exact narrowing re-lowers through the
/// unit-argument vocabulary once that lowering sees validation's exact-cast
/// facts, so the store keeps the retained value by exact equality.
#[test]
fn an_exact_cast_chain_over_a_member_read_relowers_and_composes() {
    let source = r#"
        data T { bytes: [u8; 16]; v: u8; }
        machine T::poke(&mut self, digit_index: u64) {
            transition digit_index < 4 {
                true -> store_digit(digit_index)
            }
            transition { _ -> done() }
            state store_digit(&mut self, digit_index: u64) {
                self.bytes[digit_index] = (((48 as u64 in Wrapping) + (((self.v % 10) as u64) as u64 in Wrapping)) as u64) as u8;
            }
            state done(&mut self) {}
        }
    "#;
    let stores = stores(source, 1).expect("the exact-cast chain re-lowers");
    assert!(
        matches!(
            stores.as_slice(),
            [CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }]
        ),
        "one element store: {stores:#?}"
    );
}
