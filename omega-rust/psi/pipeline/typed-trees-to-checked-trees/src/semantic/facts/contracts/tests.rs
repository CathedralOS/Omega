use crate::tests::front_end::checked_program_result;
#[test]
fn boolean_call_promises_keep_actual_values_when_formal_spellings_overlap() {
    for (primitive, formal) in ["bool", "i32"]
        .into_iter()
        .flat_map(|primitive| ["value", "other", "input"].map(|formal| (primitive, formal)))
    {
        for (actual, admitted) in [("value", true), ("other", false)] {
            let result = checked_program_result(&format!(
                "machine identity({formal}: {primitive}) -> {primitive} ensures result == {formal} {{ {formal} }}
                 machine compute(value: {primitive}, other: {primitive}) -> {primitive} ensures result == value {{ identity({actual}) }}"
            ));
            assert_eq!(result.is_ok(), admitted, "{primitive} {formal}: {actual}");
            if let Err(diagnostics) = result {
                assert!(diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .contains("cannot prove ensures contract for exit from compute")
                }));
            }
        }
    }
}
