//! Independent structural legalization replay rejection for roster and semantic corruption.

use crate::tests::fixtures::structural_call::structural_call_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};

#[test]
fn independent_replay_rejects_placement_effect_and_roster_erasure() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0].recipe =
        omega_legalized_operations::StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());

    let mut malformed_target = target.clone();
    let omega_target_operations::TargetOperation::UnitBody(caller) =
        &mut malformed_target.functions[0].operation
    else {
        panic!("fixture caller is Unit")
    };
    caller
        .operations
        .push(caller.operations.last().unwrap().clone());
    assert!(legalize_target_operations(&malformed_target, &abstract_plan, &unit).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call_plan
        .shadow_bytes += 8;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut corrupted_target = target.clone();
    let omega_target_operations::TargetOperation::UnitBody(callee) =
        &mut corrupted_target.functions[1].operation
    else {
        panic!("fixture callee is Unit")
    };
    callee.call_plan.shadow_bytes += 8;
    assert!(legalize_target_operations(&corrupted_target, &abstract_plan, &unit).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .arguments[0]
        .target
        .source_byte_offset = 1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .effect
        .output += 1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut erased = legalized.plan().clone();
    erased.structural_unit_functions.clear();
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, erased,).is_err());
}
