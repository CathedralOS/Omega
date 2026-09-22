use super::BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE;
#[test]
fn byte_content_entry_routes_reject_unknown_actuals_and_unrelated_guards() {
    for source in [
        // A pristine `let mut` actual still transports its bound value back to
        // the caller's entry operand, so `let mut current = left` alone would
        // prove `current` is `left` and the published route would cover the
        // call. The unknown-actual control must first overwrite the bound
        // snapshot: the field write leaves `current` unrelated to any entry
        // operand the published route can name.
        BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE.replace(
            "\n        Helper::inspect(left, right);",
            "\n        let mut current: Borrowed = left; current.active = right.active; Helper::inspect(current, right);",
        ),
        BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE.replace(
            "machine Root::enter(left: Borrowed, right: Borrowed)\n    crashes Abort\n        left == right",
            "machine Root::enter(left: Borrowed, right: Borrowed)\n    crashes Abort\n        left.active",
        ),
    ] {
        let diagnostics = crate::front_end::checked_program_result(&source).expect_err("entry route does not cover the call");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("call from `Root::enter` to `Helper::inspect`")
                && diagnostic.message.contains("uncovered Abort crash route")
        }), "{diagnostics:?}");
    }
}
