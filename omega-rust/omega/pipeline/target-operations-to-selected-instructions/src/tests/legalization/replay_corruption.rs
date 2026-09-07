//! Independent structural legalization replay rejection for roster and semantic corruption.

use crate::tests::fixtures::structural_call::structural_call_fixture;
use crate::{legalize_target_operations, validate_legalized_operations};

#[test]
fn independent_replay_rejects_placement_effect_and_roster_erasure() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();

    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].structural = None;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted).is_err());

    let mut malformed_target = target.clone();
    let target_operations::TargetOperation::UnitBody(caller) =
        &mut malformed_target.functions[0].operation
    else {
        panic!("fixture caller is Unit")
    };
    caller
        .operations
        .push(caller.operations.last().unwrap().clone());
    assert!(legalize_target_operations(&malformed_target, &abstract_plan, &unit).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].call_plan.shadow_bytes += 8;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut corrupted_target = target.clone();
    let target_operations::TargetOperation::UnitBody(callee) =
        &mut corrupted_target.functions[1].operation
    else {
        panic!("fixture callee is Unit")
    };
    callee.call_plan.shadow_bytes += 8;
    assert!(legalize_target_operations(&corrupted_target, &abstract_plan, &unit).is_err());

    let mut corrupted = legalized.plan().clone();
    let legalized_operations::LegalizedScalarArgument::Structural {
        target: argument, ..
    } = &mut crate::tests::fixtures::ordinary_graph::call_mut(&mut corrupted.scalar_functions[0])
        .arguments[0]
    else {
        panic!("structural argument");
    };
    argument.source_byte_offset = 1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut corrupted = legalized.plan().clone();
    corrupted.scalar_functions[0].blocks[0].instructions[0]
        .effect
        .output += 1;
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, corrupted,).is_err());

    let mut erased = legalized.plan().clone();
    erased.scalar_functions.clear();
    assert!(validate_legalized_operations(&target, &abstract_plan, &unit, erased,).is_err());
}
