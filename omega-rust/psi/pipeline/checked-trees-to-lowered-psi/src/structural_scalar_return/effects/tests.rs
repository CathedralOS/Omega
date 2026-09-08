use super::*;

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

#[test]
fn pure_primitive_reference_returns_lower_without_fabricated_effects_or_attachment() {
    for source in [
        "machine hold(value: &u64) -> u64 { 11 }",
        "machine hold(first: &u64, second: &u64) -> u64 { 11 }",
        "machine hold(seed: u64, first: &u64, value: u64, second: &u64) -> u64 { value }",
        "machine hold(value: &mut u64) -> u64 { 11 }",
        "machine hold(value: &write u64) -> u64 { 11 }",
    ] {
        let checked = checked(source);
        let plan = &checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines[0];
        assert!(
            validate(&checked, plan).unwrap(),
            "primitive-reference cohort: {source}"
        );
        let lowered = crate::lower_machine(&checked, "hold").unwrap();
        let machine = &lowered.semantic_module.machines[0];
        assert!(machine.attachment.is_none());
        assert_eq!(
            machine.structural_parameters.len(),
            plan.structural_parameters.len()
        );
        assert!(
            !machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::WriteOnlyPrimitiveStore { .. }
                ))
        );
    }
}

#[test]
fn pure_primitive_reference_returns_reject_signature_and_return_custody_substitution() {
    let checked = checked("machine hold(first: &u64, value: u64, second: &u64) -> u64 { value }");
    let original = &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0];
    for mutation in 0..7 {
        let mut plan = original.clone();
        match mutation {
            0 => plan.structural_parameters.swap(0, 1),
            1 => {
                plan.structural_parameters.pop();
            }
            2 => {
                plan.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::MutableBorrow
            }
            3 => plan.scalar_parameters[0].source_position = 0,
            4 => plan.return_statement_ordinal = 1,
            5 => {
                plan.attachment_type_identity =
                    Some(plan.structural_parameters[0].type_identity.clone())
            }
            _ => plan.structural_parameters[0].type_identity = String::from("unrelated primitive"),
        }
        assert!(
            validate(&checked, &plan).is_err(),
            "custody mutation {mutation}"
        );
    }
}

#[test]
fn pure_primitive_reference_returns_cannot_erase_an_authored_assignment() {
    let checked = checked("machine reset(value: &mut u64) -> u64 { value = 7; 11 }");
    let mut plan = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .clone();
    plan.effects.clear();
    plan.return_statement_ordinal = 0;
    assert!(validate(&checked, &plan).is_err());
}

#[test]
fn pure_primitive_reference_returns_replay_authored_contract_and_range_restrictions() {
    let plain = checked("machine hold(value: &u64, seed: u64) -> u64 { 0 }");
    for source in [
        "machine hold(value: &u64, seed: u64) -> u64 requires true; { 0 }",
        "machine hold(value: &u64, seed: u64) -> u64 ensures result == 0; { 0 }",
        "machine hold(value: &u64, seed: u64) -> u64 crashes Trap { 0 }",
        "machine hold(value: &u64 [0..=5], seed: u64) -> u64 { 0 }",
        "machine hold(value: &u64, seed: u64 [0..=5]) -> u64 { 0 }",
        "machine hold(value: &u64, seed: u64) -> u64 [0..=5] { 0 }",
    ] {
        let mut checked = checked(source);
        let machine = &checked.machines()[0];
        let mut forged = plain.facts.flow.terminal_structural_scalar_returns.machines[0].clone();
        forged.machine = machine.symbol;
        forged.state = checked.machine_states(machine)[0].symbol;
        checked.facts.proof.contract_facts = Default::default();
        checked.facts.flow.terminal_structural_scalar_returns =
            plain.facts.flow.terminal_structural_scalar_returns.clone();
        assert!(validate(&checked, &forged).is_err(), "{source}");
    }
}
