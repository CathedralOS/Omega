use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_program(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unexpected cancellation: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow")),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn cancellation_preserves_each_intermediate_overflow_obligation() {
    for expression in [
        "(ceiling + value) - value",
        "ceiling + ((value + 1) - (value + 1))",
        "ceiling + (value - other)",
    ] {
        check(
            &format!(
                "machine cancel(value: u64, other: u64, ceiling: u64) -> u64 {{ {expression} }}"
            ),
            false,
        );
    }
}
