use super::{
    IntegerSign, IntegerType, IntegerValue, TerminalExecutionResult, TerminalScalarValue,
    checked_arms, encode_module, encode_proof_section, execute,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
fn encoded_computed_arms(source: &str, combined: bool) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_arms(source, combined);
    assert_authored_return_roots(&checked, source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}, combined={combined}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof"),
    )
}

fn assert_authored_return_roots(checked: &checked_trees::CheckedTrees, source: &str) {
    use checked_trees::CheckedScalarExpressionRole;

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    assert_eq!(
        checked.typed.machine_states(machine).len(),
        1,
        "computed returns must not manufacture source continuation states"
    );
    assert!(
        checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| root.machine == machine.symbol
                && matches!(
                    root.role,
                    CheckedScalarExpressionRole::Return
                        | CheckedScalarExpressionRole::ContinuationReturn
                )),
        "authored return must retain its checked computation root: {source}"
    );
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

#[test]
fn guarded_computed_integer_returns_execute_the_selected_expression() {
    for (expression, expected) in [
        ("identity(input) + 1u8", 0),
        ("identity(input) - 1u8", 254),
        ("identity(input) * 2u8", 254),
        ("identity(input) / 3u8", 85),
        ("identity(input) % 3u8", 0),
        ("identity(input) & 15u8", 15),
        ("identity(input) | 0u8", 255),
        ("identity(input) ^ 15u8", 240),
        ("~identity(input)", 0),
        ("identity(input) << 1u16", 254),
        ("identity(input) >> 1u16", 127),
        ("identity(identity(input) - 1u8) - 2u8", 252),
    ] {
        let source = format!(
            r#"
            machine identity(input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{ input }}
            machine value(selected: bool, input: u8 in Wrapping) -> u8 in Wrapping
            requires 0u8 == 0u8
            ensures 0u8 == 0u8
            {{
                transition selected {{
                    true -> ({expression})
                    false -> 17u8
                }}
            }}
        "#
        );
        for combined in [false, true] {
            let artifact = encoded_computed_arms(&source, combined);
            for selected in [false, true] {
                assert_eq!(
                    execute(
                        &artifact,
                        &[TerminalScalarValue::Boolean(selected), unsigned(8, 255)],
                    )
                    .unwrap(),
                    TerminalExecutionResult::Scalar(unsigned(
                        8,
                        if selected { expected } else { 17 }
                    )),
                    "{expression}, selected={selected}, combined={combined}",
                );
            }
        }
    }
}
