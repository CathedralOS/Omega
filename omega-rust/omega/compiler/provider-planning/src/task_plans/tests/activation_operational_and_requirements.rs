use super::{
    activation_operational_fixture, activation_requirement_fixture, operational_error,
    requirement_error,
};
use crate::task_plans::runtime_requirements::{
    exact_task_machine_blocking, exact_task_machine_suspension,
};
use crate::task_plans::specialization_commitments::exact_task_machine_contract;

#[test]
fn activation_operational_rejects_missing_contract() {
    let (mut program, target, _) = activation_operational_fixture();
    program
        .facts
        .contract_plans
        .machines
        .retain(|plan| plan.machine != target);

    assert!(
        operational_error(exact_task_machine_contract(&program, target, "Target"))
            .contains("no checked machine contract")
    );
}

#[test]
fn activation_operational_rejects_missing_suspension() {
    let (mut program, target, _) = activation_operational_fixture();
    program
        .facts
        .suspensions
        .machines
        .retain(|fact| fact.machine != target);

    assert!(
        operational_error(exact_task_machine_suspension(&program, target, "Target"))
            .contains("no checked suspension plan")
    );
}

#[test]
fn activation_operational_rejects_missing_blocking() {
    let (mut program, target, _) = activation_operational_fixture();
    program
        .facts
        .blocking
        .machines
        .retain(|fact| fact.machine != target);

    assert!(
        operational_error(exact_task_machine_blocking(&program, target, "Target"))
            .contains("no checked blocking plan")
    );
}

#[test]
fn activation_operational_rejects_duplicate_contract() {
    let (mut program, target, _) = activation_operational_fixture();
    let mut duplicate = program.facts.contract_plans.machines[0].clone();
    duplicate.report_fingerprint = 0x3333;
    program.facts.contract_plans.machines.push(duplicate);

    assert!(
        operational_error(exact_task_machine_contract(&program, target, "Target"))
            .contains("duplicate exact checked machine contracts")
    );
}

#[test]
fn activation_operational_rejects_duplicate_suspension() {
    let (mut program, target, _) = activation_operational_fixture();
    let mut duplicate = program.facts.suspensions.machines[0];
    duplicate.plan.checked_may_suspend = false;
    program.facts.suspensions.machines.push(duplicate);

    assert!(
        operational_error(exact_task_machine_suspension(&program, target, "Target"))
            .contains("duplicate exact checked suspension plans")
    );
}

#[test]
fn activation_operational_rejects_duplicate_blocking() {
    let (mut program, target, _) = activation_operational_fixture();
    let mut duplicate = program.facts.blocking.machines[0];
    duplicate.plan.checked_may_block = true;
    program.facts.blocking.machines.push(duplicate);

    assert!(
        operational_error(exact_task_machine_blocking(&program, target, "Target"))
            .contains("duplicate exact checked blocking plans")
    );
}

#[test]
fn activation_requirement_rejects_missing_owner() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement_owner = symbols::SymbolHandle::invalid();

    assert!(requirement_error(&fixture.program, &fixture.selection).contains("one exact"));
}

#[test]
fn activation_requirement_rejects_duplicate_owner() {
    let mut fixture = activation_requirement_fixture(false);
    fixture
        .program
        .typed
        .push_trait_definition(checked_trees::trait_definition::TraitDefinition {
            symbol: fixture.selection.requirement_owner,
            is_boundary: true,
            name: checked_trees::name::Identifier::generated("duplicate::TaskRuntime"),
            ..Default::default()
        });

    assert!(requirement_error(&fixture.program, &fixture.selection).contains("uniquely"));
}

#[test]
fn activation_requirement_rejects_non_boundary_owner() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement_owner = fixture.private_owner;
    fixture.selection.requirement = fixture.private_requirement;

    assert!(requirement_error(&fixture.program, &fixture.selection).contains("boundary"));
}

#[test]
fn activation_requirement_rejects_missing_owned_signature() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement = symbols::SymbolHandle::invalid();

    assert!(
        requirement_error(&fixture.program, &fixture.selection)
            .contains("belong to its exact retained owner")
    );
}

#[test]
fn activation_requirement_rejects_duplicate_owned_signature() {
    let fixture = activation_requirement_fixture(true);

    assert!(
        requirement_error(&fixture.program, &fixture.selection)
            .contains("resolve uniquely within its exact owner")
    );
}

#[test]
fn activation_requirement_rejects_cross_owner_signature_drift() {
    let mut fixture = activation_requirement_fixture(false);
    fixture.selection.requirement_owner = fixture.other_owner;

    assert!(
        requirement_error(&fixture.program, &fixture.selection)
            .contains("belong to its exact retained owner")
    );
}
