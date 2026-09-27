use super::{
    assert_structural_entry_requirement_artifact, assert_unconditional_call_trap,
    integer_field_entry_source, with_caller,
};

#[test]
fn fixed_integer_entry_comparisons_preserve_their_declared_crash_routes() {
    for primitive in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        for (comparison, actual, bound) in [
            (">", 1, 0),
            (">=", 1, 1),
            ("<", 0, 1),
            ("<=", 1, 1),
            ("==", 1, 1),
            ("!=", 1, 0),
        ] {
            assert_unconditional_call_trap(&with_caller(
                &format!(
                    "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                     machine guarded(input: {primitive}) -> bool\n\
                     requires input {comparison} {bound}\n\
                     crashes Trap input {comparison} {bound}\n\
                     {{ trigger() }}",
                ),
                &format!("guarded({actual})"),
            ));
        }
    }
}

#[test]
fn numeric_entry_bounds_preserve_signed_and_unsigned_carrier_extremes() {
    for (primitive, predicate, actual) in [
        ("i8", "input < -127", "-128"),
        ("i64", "input > 9223372036854775806", "9223372036854775807"),
        (
            "u64",
            "input > 18446744073709551614",
            "18446744073709551615",
        ),
    ] {
        assert_unconditional_call_trap(&with_caller(
            &format!(
                "machine trigger() -> bool crashes Trap {{ crash Trap; }}\n\
                 machine guarded(input: {primitive}) -> bool\n\
                 requires {predicate}\n\
                 crashes Trap {predicate}\n\
                 {{ trigger() }}",
            ),
            &format!("guarded({actual})"),
        ));
    }
}

#[test]
fn integer_field_entry_comparisons_retain_all_fixed_carriers_and_relations() {
    for carrier in ["i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64"] {
        for predicate in [
            "record.count == 1",
            "record.count != 0",
            "record.count < 2",
            "record.count <= 2",
            "record.count > 0",
            "record.count >= 1",
            "0 < record.count",
        ] {
            assert_structural_entry_requirement_artifact(&integer_field_entry_source(
                &format!("data Record {{ count: {carrier}; }}"),
                "&Record",
                predicate,
                predicate,
                "",
            ));
        }
    }
}

#[test]
fn integer_field_entry_formulas_preserve_nested_paths_and_root_access() {
    for ownership in ["", "&", "&mut "] {
        for predicate in [
            "record.inner.count > 0",
            "record.inner.count < record.inner.other",
            "record.inner.count <= record.inner.other",
            "record.inner.count != record.inner.other",
            "record.inner.count > 0 && record.inner.count < 10",
            "record.inner.count < 2 || record.inner.count > 3",
            "record.inner.count == 1 || record.inner.count == 2",
            "record.enabled && record.inner.count > 0",
        ] {
            assert_structural_entry_requirement_artifact(&integer_field_entry_source(
                "data Record { #0 count: i32; #11 other: i32; }\n\
                 data Envelope { #7 inner: Record; #9 enabled: bool; }",
                &format!("{ownership}Envelope"),
                predicate,
                predicate,
                "",
            ));
        }
    }
}
