use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

fn execute(source: &str) -> TerminalExecutionResult {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("encode proof");
    interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), &[])
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

#[test]
fn scalar_storage_remapping_preserves_direct_calls_and_explicit_state_arguments() {
    for source in [
        "machine identity(input: bool) -> bool\nrequires true == true\nensures true == true\n{ input }\nmachine value() -> bool\nrequires true == true\nensures true == true\n{ let mut flag: bool = false; let saved: bool = flag; flag = true; let returned: bool = identity(flag); returned && !saved }",
        "machine value() -> bool\nrequires true == true\nensures true == true\n{ let mut flag: bool = false; let saved: bool = flag; flag = flag || true; transition { _ -> finish(flag, saved) } state finish(current: bool, prior: bool) -> bool { current && !prior } }",
    ] {
        assert_eq!(
            execute(source),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
    }
}

#[test]
fn scalar_storage_reads_reject_stale_symbols_and_duplicate_computations() {
    use checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole};
    let source = "machine value() -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ let mut current: u8 = 7; current }";
    let original = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(&original, TerminalMachineSelection::Name("value"))
        .expect("unmodified storage read lowers");
    for mutation in 0..3 {
        let mut changed = original.clone();
        let expressions = &mut changed.facts.values.scalar_expressions.expressions;
        let position = expressions
            .iter()
            .position(|expression| expression.role == CheckedScalarExpressionRole::Return)
            .expect("selected return");
        if mutation == 2 {
            expressions.push(expressions[position].clone());
        } else {
            let CheckedScalarExpression::StorageRead { symbol, .. } =
                &mut expressions[position].expression
            else {
                panic!("return reads exact storage")
            };
            *symbol = if mutation == 0 {
                symbols::SymbolHandle::invalid()
            } else {
                symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1)
            };
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("value")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
