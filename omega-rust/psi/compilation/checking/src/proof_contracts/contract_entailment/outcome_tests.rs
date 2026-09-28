use crate::proven_machine_contract_expressions;

#[test]
fn declarations_and_admissions_do_not_supply_checked_body_outcomes() {
    let mut program =
        crate::front_end::typed_program("machine theorem(value: u64) ensures value == value {}");
    let symbol = program.machines()[0].symbol;
    for mode in [
        language_semantics::MachineSupplyMode::Boundary,
        language_semantics::MachineSupplyMode::AdmissionClaim,
        language_semantics::MachineSupplyMode::TopLevelRequirement,
    ] {
        program.machines_mut()[0].supply_mode = mode;
        assert!(
            proven_machine_contract_expressions(&program, symbol).is_empty(),
            "{mode:?}"
        );
    }
}

#[test]
fn inherited_law_matching_rejects_exact_requirement_drift() {
    let mut program = crate::front_end::typed_program(
        r#"
        trait Reflexive {
            machine law(value: u64) ensures value == value;
            machine other(value: u64) ensures value == value;
        }
        machine theorem(value: u64) satisfies Reflexive::law ensures value == value {}
    "#,
    );
    let machine = program.machines()[0].clone();
    let other = program
        .trait_machine_signatures(&program.traits()[0])
        .iter()
        .find(|requirement| requirement.name.as_str() == "other")
        .expect("other requirement")
        .symbol;
    let conformance = program.machine_trait_conformances(&machine)[0].clone();
    let handle = program
        .machine_trait_conformances
        .iter()
        .find_map(|(handle, candidate)| (candidate == &conformance).then_some(handle))
        .expect("conformance handle");
    program
        .machine_trait_conformances
        .get_mut(handle)
        .requirement_symbol = other;
    assert!(crate::matched_machine_law_guarantees(&program, machine.symbol).is_empty());
}
