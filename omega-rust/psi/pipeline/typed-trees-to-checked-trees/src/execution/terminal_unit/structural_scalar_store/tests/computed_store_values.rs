use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use checked_trees::types::PrimitiveType;

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

/// A store keeps the scalar-expression table's retained value for authored
/// roots the narrower unit-argument lowerer cannot re-spell — the
/// named-domain cast chain `(48 + remaining % 10) as u8` of
/// `put_digits_reversed` — rather than declining custody outright. The
/// retained expression is the canonical lowering of that authored root.
#[test]
fn computed_value_stores_through_a_guard_bounded_index() {
    let source = r#"
        data T { bytes: [u8; 16]; len: u64; }
        machine T::put_digits_reversed(&mut self, remaining: u64, digit_index: u64) {
            transition digit_index < 4 {
                true -> store_digit(remaining, digit_index)
                false -> done()
            }
            state store_digit(&mut self, remaining: u64, digit_index: u64) {
                self.bytes[digit_index] = (((48 as u64 in Wrapping) + ((remaining % 10) as u64 as u64 in Wrapping)) as u64) as u8;
                self.len = ((digit_index as u64 in Wrapping) + (1 as u64 in Wrapping)) as u64;
            }
            state done(&mut self) {}
        }
    "#;
    let stores = stores(source, 1).expect("retained cast chain stores");
    let Some(CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { value, .. }) =
        stores.first()
    else {
        panic!("element store leads the sequence");
    };
    assert!(matches!(
        value,
        checked_trees::CheckedCallScalarArgument::Pure(
            checked_trees::CheckedScalarExpression::IntegerExactCast {
                primitive_type: PrimitiveType::U8,
                ..
            }
        )
    ));
}

/// The relaxation reaches only values that still lower through the ordinary
/// scalar vocabulary: a named-domain cast chain whose leaf is a structural
/// field read needs binding lists the store's value lane does not carry, so
/// it must keep declining rather than stand on its retained expression.
#[test]
fn binding_needs_still_decline_inside_a_named_domain_chain() {
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
    assert!(
        stores(source, 1).is_none(),
        "a retained member-read leaf is not an ordinary scalar value"
    );
}
