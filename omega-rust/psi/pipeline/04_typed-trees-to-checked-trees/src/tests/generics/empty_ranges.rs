//! Empty declarations do not authorize initialization or value delivery.

use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program_result;

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
        let typed = typed_program_result(&source).expect("empty declaration types");
        lower_typed_trees(typed, &CheckingRequest::settled())
            .expect("declaring an empty parameter creates no value");
    }
}

#[test]
fn empty_integer_ranges_project_to_rejecting_wire_bounds() {
    for empty_type in EMPTY_TYPES {
        let source = format!("machine unreachable(value: {empty_type}) {{ }}");
        let typed = typed_program_result(&source).expect("empty parameter types");
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
