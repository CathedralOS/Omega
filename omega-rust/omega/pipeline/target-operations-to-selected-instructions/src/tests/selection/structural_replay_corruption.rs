//! Structural selection replay rejection for ABI, constraints, target, and semantic custody.

use crate::tests::fixtures::microsoft_environment::microsoft_selection_environment;
use crate::tests::fixtures::structural_call::{qualified_fixture_unit, structural_call_fixture};
use crate::{
    legalize_target_operations, select_instructions, selected_instruction_plan_identity,
    validate_selected_instructions,
};
use semantic_vocabulary::{FuelScheduleIdentity, IntegerSign, ScalarType};
use terminal_psi::{StructuralFieldType, StructuralTypeShape};

#[test]
fn selected_structural_replay_rejects_abi_constraint_and_semantic_custody_mutations() {
    let (abstract_plan, target, unit) = structural_call_fixture();
    let legalized = legalize_target_operations(&target, &abstract_plan, &unit).unwrap();
    let (physical, catalog, constraints) = microsoft_selection_environment();
    let selected = select_instructions(&legalized, &constraints, &physical, &catalog).unwrap();
    let selected_identity = selected.receipt().identity();

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .abi
        .layout
        .outgoing_frame_byte_count -= 8;
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .requirement_obligations
        .clear();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .crash_continuations
        .clear();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .implicit_uses
        .pop();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0].abi.parameters[0]
        .semantic
        .qualifications[0] = semantic_vocabulary::StructuralDomainId::new(2).unwrap();
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut corrupted = selected.plan().clone();
    corrupted.structural_unit_functions[0]
        .call
        .as_mut()
        .unwrap()
        .effect
        .output += 1;
    assert_ne!(
        selected_instruction_plan_identity(&corrupted),
        selected_identity
    );
    assert!(
        validate_selected_instructions(&legalized, &constraints, &physical, &catalog, corrupted)
            .is_err()
    );

    let mut missing_key = constraints.clone();
    missing_key.keys.structural_unit_call = None;
    assert!(select_instructions(&legalized, &missing_key, &physical, &catalog).is_err());

    let linux_target = abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let linux_legalized = legalize_target_operations(&linux_target, &abstract_plan, &unit).unwrap();
    assert!(select_instructions(&linux_legalized, &constraints, &physical, &catalog).is_err());

    let mut wrong_shape = abstract_plan.clone();
    let StructuralTypeShape::Record { fields } = &mut wrong_shape.structural_types[0].shape else {
        unreachable!()
    };
    fields[1].field_type = StructuralFieldType::Scalar(ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    ));
    let wrong_target = abstract_operations_to_target_operations::lower_to_target_operations(
        &wrong_shape,
        target::NativeTarget::uefi_x64(),
    )
    .unwrap();
    let wrong_unit = qualified_fixture_unit(
        optimization_unit::reconstruct_psi_optimization_unit_seed(
            &wrong_shape,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap(),
        wrong_shape.structural_types[0].id,
    );
    let wrong_legalized =
        legalize_target_operations(&wrong_target, &wrong_shape, &wrong_unit).unwrap();
    assert!(select_instructions(&wrong_legalized, &constraints, &physical, &catalog).is_err());
}
