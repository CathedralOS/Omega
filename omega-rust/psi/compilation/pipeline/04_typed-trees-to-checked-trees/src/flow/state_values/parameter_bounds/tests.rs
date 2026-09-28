use super::{IntegerRange, ParameterBounds};
use crate::tests::front_end::typed_program;
use numerics::bignum::BigInt;

fn range(minimum: i64, maximum: i64) -> IntegerRange {
    IntegerRange {
        minimum: BigInt::from_i64(minimum),
        maximum: BigInt::from_i64(maximum),
    }
}

#[test]
fn growing_parameter_bounds_converge_outward_and_unknown_is_absorbing() {
    let program = typed_program("machine identity(value: i64) -> i64 { value }");
    let state = &program.machine_states(&program.machines()[0])[0];
    let symbol = program.state_parameters(state)[0].symbol;
    let thresholds = [-100000, 0, 100000].map(BigInt::from_i64);
    for direction in [-1, 1] {
        let mut bounds = ParameterBounds::new(symbol, Some(range(0, 0)));
        for step in 1..=4 {
            let value = step * direction;
            let incoming = range(value.min(0), value.max(0));
            let previous = bounds.bounds.clone().unwrap();
            assert!(bounds.join(&program, state, Some(&incoming), &thresholds));
            let joined = bounds.bounds.as_ref().unwrap();
            assert!(joined.minimum <= previous.minimum && joined.minimum <= incoming.minimum);
            assert!(joined.maximum >= previous.maximum && joined.maximum >= incoming.maximum);
            if step < 4 {
                assert_eq!(joined, &incoming);
            }
        }
        let widened = if direction < 0 {
            range(-100000, 0)
        } else {
            range(0, 100000)
        };
        assert_eq!(bounds.bounds.as_ref(), Some(&widened));
        assert!(!bounds.join(&program, state, Some(&range(0, 0)), &thresholds));
        assert_eq!(bounds.bounds.as_ref(), Some(&widened));
        assert!(bounds.join(&program, state, None, &thresholds));
        assert!(!bounds.join(&program, state, Some(&range(0, 0)), &thresholds));
        assert!(bounds.bounds.is_none());
    }
}

#[test]
fn widening_uses_the_carrier_without_truncating_inconsistent_evidence() {
    let program = typed_program("machine identity(value: u8) -> u8 { value }");
    let state = &program.machine_states(&program.machines()[0])[0];
    let symbol = program.state_parameters(state)[0].symbol;
    let thresholds = [0, 1000].map(BigInt::from_i64);
    let mut bounds = ParameterBounds::new(symbol, Some(range(0, 0)));
    for maximum in 1..=4 {
        bounds.join(&program, state, Some(&range(0, maximum)), &thresholds);
    }
    assert_eq!(bounds.bounds, Some(range(0, 255)));
    assert!(!bounds.join(&program, state, Some(&range(0, 255)), &thresholds));
    assert!(bounds.join(&program, state, Some(&range(0, 256)), &thresholds));
    assert!(bounds.bounds.is_none());
}

#[test]
fn unresolved_parameter_carrier_cannot_invent_a_widening_bound() {
    let program = typed_program("machine identity(value: &u64) {}");
    let state = &program.machine_states(&program.machines()[0])[0];
    let symbol = program.state_parameters(state)[0].symbol;
    let mut bounds = ParameterBounds::new(symbol, Some(range(0, 0)));
    for maximum in 1..=4 {
        bounds.join(&program, state, Some(&range(0, maximum)), &[]);
    }
    assert!(bounds.bounds.is_none());
}
