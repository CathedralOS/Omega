//! Empty declarations do not authorize initialization or value delivery.

use super::typed_source;
use crate::lower_typed_trees;

const EMPTY_TYPES: [&str; 8] = [
    "u64[0..0]",
    "u64[0..0u64]",
    "i64[-9223372036854775808..-9223372036854775808]",
    "i64[-9223372036854775808..-9223372036854775808i64]",
    "i32[10..=5]",
    "u8[256..=300]",
    "i32[0..=10, 5..3]",
    "i32[5..3, 0..=10]",
];

#[test]
fn empty_integer_ranges_are_legal_without_value_establishment() {
    for empty_type in EMPTY_TYPES {
        let source = format!("machine unreachable(value: {empty_type}) {{ }}");
        let typed = typed_source(&source).expect("empty declaration types");
        lower_typed_trees(typed).expect("declaring an empty parameter creates no value");
    }
}

#[test]
fn empty_integer_ranges_reject_calls_stores_returns_and_zero_construction() {
    for empty_type in EMPTY_TYPES {
        for source in [
            format!("machine sink(value: {empty_type}) {{ }} machine caller() {{ sink(0); }}"),
            format!("machine caller() {{ let value: {empty_type} = 0; }}"),
            format!("machine caller() -> {empty_type} {{ 0 }}"),
            // Allocating gated zero storage is legal; constructing an
            // established record while omitting its impossible field is not.
            format!(
                "data Empty {{ value: {empty_type}; }} machine caller() {{ let value: Empty = Empty {{ }}; }}"
            ),
        ] {
            let typed = typed_source(&source).expect("empty destination source types");
            let result = lower_typed_trees(typed);
            assert!(
                result.is_err(),
                "empty destination accepted a concrete value: {source}"
            );
            let errors = result.err().unwrap();
            assert!(
                errors.iter().any(|error| {
                    error.message.contains("range")
                        || error.message.contains("zero")
                        || error.message.contains("bounded parameter")
                }),
                "expected delivery or establishment failure, not a declaration fence: {source}: {errors:?}",
            );
            assert!(
                errors
                    .iter()
                    .all(|error| !error.message.contains("declares an inverted range")),
                "empty declarations themselves remain legal: {errors:?}",
            );
        }
    }
}

#[test]
fn empty_integer_ranges_project_to_rejecting_wire_bounds() {
    for empty_type in EMPTY_TYPES {
        let source = format!("machine unreachable(value: {empty_type}) {{ }}");
        let typed = typed_source(&source).expect("empty parameter types");
        let machine = &typed.machines()[0];
        let state = &typed.machine_states(machine)[0];
        let parameter = &typed.state_parameters(state)[0];
        let range = validation::scalar_representation_range(&typed, parameter.type_reference)
            .expect("empty is a retained restriction, not an absent range");
        assert_eq!((range.minimum, range.maximum), (1, 0), "{empty_type}");
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert!(!(value >= range.minimum && value <= range.maximum));
        }
        for value in [0_u64, 1, u64::MAX] {
            assert!(!(value >= range.minimum as u64 && value <= range.maximum as u64));
        }
    }
}
