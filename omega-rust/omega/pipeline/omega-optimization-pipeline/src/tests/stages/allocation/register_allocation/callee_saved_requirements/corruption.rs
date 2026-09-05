use crate::tests::*;

use super::fixture::{call_homes, stage, wide_budget};

fn rejects_noncanonical(
    source: &StagedOptimizedRegisterHomes,
    plan: AllocatedCalleeSavedRequirementPlan,
) {
    assert_eq!(
        validate_allocated_callee_saved_requirements(source, plan),
        Err(AllocatedCalleeSavedRequirementError::NonCanonicalRequirements)
    );
}

#[test]
fn replay_rejects_every_root_usage_roster_function_unit_and_witness_corruption() {
    let source = call_homes(NativeTarget::linux_x64());
    let canonical = stage(&source, wide_budget()).unwrap().plan().clone();

    let mut selected = canonical.clone();
    selected.selected =
        omega_selected_instructions::SelectedInstructionPlanIdentity::from_bytes([0x31; 32]);
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, selected),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );

    let mut homes = canonical.clone();
    homes.homes =
        omega_selected_instructions_to_register_homes::RegisterHomeIdentity::from_bytes([0x32; 32]);
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, homes),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );

    let mut manifest = canonical.clone();
    manifest.post_allocation_manifest =
        omega_optimization_core::PostAllocationOptimizationManifestIdentity::from_bytes([0x33; 32]);
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, manifest),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );

    let mut environment = canonical.clone();
    environment.register_environment =
        omega_register_model::TargetRegisterEnvironmentIdentity::from_bytes([0x34; 32]);
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, environment),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );

    let mut physical = canonical.clone();
    physical.physical_register_model =
        omega_register_model::PhysicalRegisterModelIdentity::from_bytes([0x35; 32]);
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, physical),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );

    let mut target = canonical.clone();
    target.target = NativeTarget::linux_arm64();
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, target),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );

    let mut usage = canonical.clone();
    usage.usage.validation_steps += 1;
    assert_eq!(
        validate_allocated_callee_saved_requirements(&source, usage),
        Err(AllocatedCalleeSavedRequirementError::UsageMismatch)
    );

    let mut abi = canonical.clone();
    abi.abi = FrameAbiPreservationConvention::MicrosoftX64;
    rejects_noncanonical(&source, abi);

    let mut roster = canonical.clone();
    roster.callee_saved_units.pop();
    rejects_noncanonical(&source, roster);

    let mut function = canonical.clone();
    function.functions[0].machine = MachineId::new(93_001).unwrap();
    rejects_noncanonical(&source, function);

    let modified_function = canonical
        .functions
        .iter()
        .position(|function| !function.modified_units.is_empty())
        .unwrap();

    let mut kind = canonical.clone();
    kind.functions[modified_function].kind = AllocatedCalleeSavedFunctionKind::StructuralUnit;
    rejects_noncanonical(&source, kind);

    let mut unit = canonical.clone();
    unit.functions[modified_function].modified_units[0].unit =
        omega_register_model::RegisterUnitId(u16::MAX);
    rejects_noncanonical(&source, unit);

    let mut witness = canonical.clone();
    witness.functions[modified_function].modified_units[0].witnesses[0] =
        CalleeSavedModificationWitness::ImplicitClobber {
            block: omega_selected_instructions::SelectedBlockId(999),
            instruction: SelectedInstructionId(999),
        };
    rejects_noncanonical(&source, witness);

    let mut duplicate = canonical.clone();
    let duplicate_unit = duplicate.functions[modified_function].modified_units[0].clone();
    duplicate.functions[modified_function]
        .modified_units
        .insert(0, duplicate_unit);
    rejects_noncanonical(&source, duplicate);

    let foreign = call_homes(NativeTarget::linux_arm64());
    assert_eq!(
        validate_allocated_callee_saved_requirements(&foreign, canonical),
        Err(AllocatedCalleeSavedRequirementError::RootMismatch)
    );
}
