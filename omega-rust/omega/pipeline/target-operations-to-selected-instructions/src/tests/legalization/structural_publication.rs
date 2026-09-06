//! Publication classification agrees with the complete existing structural family.

use crate::legalization::accepts_fragment_publication_input as accepts;
use crate::tests::fixtures::{
    claim_completion::claim_completion_settlement_fixture,
    installed_provider::installed_provider_legalization_fixture,
    microsoft_environment::microsoft_selection_environment,
    structural_call::structural_call_fixture,
};
use crate::{legalize_target_operations, select_instructions};
use target_operations::{TargetOperation, TargetUnitOperation};

#[test]
fn structural_publication_admits_every_existing_roster_form() {
    for (abstract_plan, target, unit) in [
        structural_call_fixture(),
        installed_provider_legalization_fixture(),
        singleton_claim_completion_fixture(),
    ] {
        assert!(accepts(&target, &abstract_plan, &unit));
        let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
        assert_eq!(
            legalized.plan().structural_unit_functions.len(),
            target.functions.len()
        );
        let (physical, catalog, constraints) = microsoft_selection_environment();
        select_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
    }
    let (mut abstract_plan, mut target, mut unit) = structural_call_fixture();
    abstract_plan.functions.remove(0);
    target.functions.remove(0);
    unit.functions.remove(0);
    abstract_plan.entry = abstract_plan.functions[0].machine;
    target.entry = abstract_plan.entry;
    unit.entry = abstract_plan.entry;
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    assert!(accepts(&target, &abstract_plan, &unit));
    legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
}

#[test]
fn structural_publication_rejects_changed_abi_call_and_source_custody() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let mut changed = target.clone();
    changed.target = target::NativeTarget::windows_x64();
    // NativeTarget records the physical ABI, not the Windows/UEFI profile.
    assert_eq!(changed.target, target.target);
    assert!(accepts(&changed, &abstract_plan, &unit));
    changed.target = target::NativeTarget::linux_x64();
    assert!(!accepts(&changed, &abstract_plan, &unit));
    let mut changed = target.clone();
    let TargetOperation::UnitBody(body) = &mut changed.functions[0].operation else {
        unreachable!()
    };
    body.parameters[0].access = terminal_psi::StructuralAccess::SharedBorrow;
    assert!(!accepts(&changed, &abstract_plan, &unit));
    let mut changed = target.clone();
    let TargetOperation::UnitBody(body) = &mut changed.functions[0].operation else {
        unreachable!()
    };
    let TargetUnitOperation::Call { arguments, .. } = &mut body.operations[0] else {
        unreachable!()
    };
    arguments.swap(0, 1);
    assert!(!accepts(&changed, &abstract_plan, &unit));
    let mut changed = target.clone();
    changed.functions[1].machine = changed.functions[0].machine;
    assert!(!accepts(&changed, &abstract_plan, &unit));
    let mut changed = unit.clone();
    changed.functions[0].blocks[0].nodes[0].effect.output += 1;
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    assert!(!accepts(&target, &abstract_plan, &changed));
}

#[test]
fn structural_publication_rejects_changed_provider_and_completion_evidence() {
    let (abstract_plan, mut target, unit) = installed_provider_legalization_fixture();
    let TargetOperation::UnitBody(body) = &mut target.functions[0].operation else {
        unreachable!()
    };
    let TargetUnitOperation::InstalledProviderCall { provider, .. } = &mut body.operations[0]
    else {
        unreachable!()
    };
    provider.candidate = target.entry;
    assert!(!accepts(&target, &abstract_plan, &unit));
    let (abstract_plan, mut target, unit) = singleton_claim_completion_fixture();
    let TargetOperation::UnitBody(body) = &mut target.functions[0].operation else {
        unreachable!()
    };
    body.operations.swap(0, 1);
    assert!(!accepts(&target, &abstract_plan, &unit));
}

fn singleton_claim_completion_fixture() -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut abstract_plan, mut target, mut unit) = claim_completion_settlement_fixture();
    // The settlement fixture retains its former provider leaf for per-function
    // legalization tests. Publication has no call to that unrelated function.
    abstract_plan.functions.truncate(1);
    target.functions.truncate(1);
    unit.functions.truncate(1);
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    (abstract_plan, target, unit)
}

#[test]
fn structural_publication_rejects_two_leaves_without_restricting_legalization() {
    let (abstract_plan, target, unit) = claim_completion_settlement_fixture();
    assert!(!accepts(&target, &abstract_plan, &unit));
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    select_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
}

#[test]
fn structural_publication_rejects_three_function_chain_without_restricting_legalization() {
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, PlaceId};

    let (mut abstract_plan, _, _) = structural_call_fixture();
    let mut middle = abstract_plan.functions[0].clone();
    middle.machine = MachineId::new(3).unwrap();
    middle.entry = BlockId::new(3).unwrap();
    middle.block_entries[0].block = middle.entry;
    let places = [PlaceId::new(5).unwrap(), PlaceId::new(6).unwrap()];
    for (parameter, place) in middle.structural_parameters.iter_mut().zip(places) {
        parameter.place = place;
    }
    let AbstractOperation::CallUnit {
        psi_operation,
        structural_arguments,
        ..
    } = &mut middle.operations[0]
    else {
        unreachable!()
    };
    *psi_operation = OperationId::new(3).unwrap();
    for (argument, place) in structural_arguments.iter_mut().zip(places) {
        argument.place = place;
    }
    let AbstractOperation::ReturnUnit { psi_edge, .. } = &mut middle.operations[1] else {
        unreachable!()
    };
    *psi_edge = EdgeId::new(3).unwrap();
    let AbstractOperation::CallUnit { callee, .. } = &mut abstract_plan.functions[0].operations[0]
    else {
        unreachable!()
    };
    *callee = middle.machine;
    abstract_plan.functions.push(middle);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        target::NativeTarget::uefi_x64(),
    )
    .unwrap();
    let unit = crate::tests::fixtures::structural_call::qualified_fixture_unit(
        optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap(),
        abstract_plan.structural_types[0].id,
    );
    assert!(!accepts(&target, &abstract_plan, &unit));
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    select_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
}

#[test]
fn structural_publication_does_not_promote_borrowed_roots_to_owned_abi() {
    let (mut abstract_plan, _, _) = structural_call_fixture();
    abstract_plan.functions.remove(0);
    abstract_plan.entry = abstract_plan.functions[0].machine;
    for parameter in &mut abstract_plan.functions[0].structural_parameters {
        parameter.access = terminal_psi::StructuralAccess::SharedBorrow;
    }
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        target::NativeTarget::uefi_x64(),
    )
    .unwrap();
    let unit = crate::tests::fixtures::structural_call::qualified_fixture_unit(
        optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap(),
        abstract_plan.structural_types[0].id,
    );
    assert!(!accepts(&target, &abstract_plan, &unit));
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    assert!(select_instructions(&legalized, &constraints, &physical, &catalog).is_err());
}
