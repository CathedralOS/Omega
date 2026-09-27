use super::{StructuralScalarReturnTypes, validate};

#[test]
fn pure_primitive_reference_returns_reject_signature_and_return_custody_substitution() {
    let checked = crate::front_end::checked_program(
        "machine hold(first: &u64, value: u64, second: &u64) -> u64 { value }",
    );
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
            validate(
                &checked,
                StructuralScalarReturnTypes::published(&checked),
                &plan
            )
            .is_err(),
            "custody mutation {mutation}"
        );
    }
}

#[test]
fn pure_primitive_reference_returns_replay_authored_contract_and_range_restrictions() {
    let plain =
        crate::front_end::checked_program("machine hold(value: &u64, seed: u64) -> u64 { 0 }");
    for source in [
        "machine hold(value: &u64, seed: u64) -> u64 requires true; { 0 }",
        "machine hold(value: &u64, seed: u64) -> u64 ensures result == 0; { 0 }",
        "machine hold(value: &u64, seed: u64) -> u64 crashes Trap { 0 }",
        "machine hold(value: &u64 [0..=5], seed: u64) -> u64 { 0 }",
        "machine hold(value: &u64, seed: u64 [0..=5]) -> u64 { 0 }",
        "machine hold(value: &u64, seed: u64) -> u64 [0..=5] { 0 }",
    ] {
        let mut checked = crate::front_end::checked_program(source);
        let machine = &checked.machines()[0];
        let mut forged = plain.facts.flow.terminal_structural_scalar_returns.machines[0].clone();
        forged.machine = machine.symbol;
        forged.state = checked.machine_states(machine)[0].symbol;
        checked.facts.proof.contract_facts = Default::default();
        checked.facts.flow.terminal_structural_scalar_returns =
            plain.facts.flow.terminal_structural_scalar_returns.clone();
        assert!(
            validate(
                &checked,
                StructuralScalarReturnTypes::published(&checked),
                &forged
            )
            .is_err(),
            "{source}"
        );
    }
}
