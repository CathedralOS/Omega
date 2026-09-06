use super::*;
use crate::realization::input::lower_realization_input;
use crate::realization::optimization_stage::lower_realization_optimization_stage;
use crate::realization::optimized_fragment_projection::{
    OptimizedFragmentPublicationRequest, emit_return_only_optimized_fragments,
};
use crate::realization::target_stage::lower_realization_target_stage;

#[test]
fn identity_return_programs_use_fragments_and_preserve_native_bytes() {
    let checked =
        crate::tests::fixtures::checked_source::checked("data Main {} machine Main::launch() {}");
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .unwrap();
    let artifact =
        terminal_production::produce_terminal_artifact(&checked, "Main::launch").unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    let selections = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    for (target_profile, target) in [
        (
            target::TargetProfile::WindowsX64,
            target::NativeTarget::windows_x64(),
        ),
        (
            target::TargetProfile::LinuxX64,
            target::NativeTarget::linux_x64(),
        ),
        (
            target::TargetProfile::LinuxArm64,
            target::NativeTarget::linux_arm64(),
        ),
        (
            target::TargetProfile::MacosArm64,
            target::NativeTarget::macos_arm64(),
        ),
    ] {
        let signature =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                target_profile.program_entry_slot(),
                selection.machine,
                selection.machine,
                selection.name.clone(),
                "entry".into(),
                "Main::launch() -> Unit".into(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
                Vec::new(),
            )
            .unwrap();
        let request = NativeRealizationCoreRequest {
            target,
            profile: &profile,
            terminal_authority_policy: crate::current_terminal_authority_policy(),
            terminal_authority_permission_policy:
                crate::current_terminal_authority_permission_policy(),
            program_entry: crate::NativeProgramEntrySettlement::new(&signature, None, &[]),
            optimization_selections: &selections,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
        };
        let input =
            lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
                .unwrap();
        let baseline_target = abstract_operations_to_target_operations::lower_to_target_operations(
            input.plan(),
            target,
        )
        .unwrap();
        let baseline_assignment =
            target_operations_to_assigned_target_operations::assign_registers(&baseline_target)
                .unwrap();
        let baseline = machine_emission::emit_machine_code(&baseline_assignment).unwrap();
        let optimized = lower_realization_optimization_stage(input, &request).unwrap();
        let target_stage = lower_realization_target_stage(optimized, None, &[], &request).unwrap();
        let physical = lower_realization_physical_stage(target_stage, &request).unwrap();
        let NativePhysicalStageResult::Optimized(physical) = physical else {
            panic!("empty selections must use the fragment route for return programs");
        };
        let (plan, _) = emit_return_only_optimized_fragments(
            physical.physical,
            OptimizedFragmentPublicationRequest {
                identity_scope: Some(native_artifact::NativePhysicalEvidenceScope::Unavailable),
                has_provider_installation: false,
                has_boundary_settlements: false,
                boundary_application_coverage: None,
                optimized_plan: &physical.optimized_plan,
                terminal: physical.terminal,
                validation: physical.validation,
                final_unit: physical.final_unit,
            },
        )
        .unwrap();
        assert_eq!(plan.functions.len(), baseline.functions.len());
        for (new, old) in plan.functions.iter().zip(&baseline.functions) {
            assert_eq!(new.machine, old.machine);
            assert_eq!(new.attachment, old.attachment);
            assert_eq!(new.bytes, old.bytes, "{target_profile:?}");
        }
        image_emission::build_object_artifact(&plan).unwrap();

        // The shape classifier must inspect every function, not just the entry.
        let input =
            lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), &profile)
                .unwrap();
        let optimized = lower_realization_optimization_stage(input, &request).unwrap();
        let target_stage = lower_realization_target_stage(optimized, None, &[], &request).unwrap();
        let NativePhysicalStageResult::Optimized(physical) =
            lower_realization_physical_stage(target_stage, &request).unwrap()
        else {
            panic!("identity fragment route");
        };
        let source = physical.physical.into_function_fragment_emission_source();
        let mut selected = source.selected_plan().clone();
        let mut additional = selected.functions[0].clone();
        additional.machine = semantic_vocabulary::MachineId::new(selected.entry.get() + 1).unwrap();
        selected.functions.push(additional);
        machine_emission::validate_unit_shape(&selected).unwrap();
        let extra_block = selected.functions[1].blocks[0].clone();
        selected.functions[1].blocks.push(extra_block);
        assert!(machine_emission::validate_unit_shape(&selected).is_err());
    }
}

#[test]
fn selected_return_programs_publish_replayable_native_evidence_on_every_target() {
    let checked =
        crate::tests::fixtures::checked_source::checked("data Main {} machine Main::launch() {}");
    let selection = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|machine| machine.name == "Main::launch")
        .unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    let selections = optimization_core::PostTerminalOptimizationSelections::new(
        optimization_core::OptimizationSelections::new([
            optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap(),
    )
    .unwrap();
    let providers = effects::SelectedProviderPlanFacts::default();
    for (target_profile, target) in [
        (
            target::TargetProfile::WindowsX64,
            target::NativeTarget::windows_x64(),
        ),
        (
            target::TargetProfile::LinuxX64,
            target::NativeTarget::linux_x64(),
        ),
        (
            target::TargetProfile::LinuxArm64,
            target::NativeTarget::linux_arm64(),
        ),
        (
            target::TargetProfile::MacosArm64,
            target::NativeTarget::macos_arm64(),
        ),
    ] {
        let signature =
            program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
                target_profile.program_entry_slot(),
                selection.machine,
                selection.machine,
                selection.name.clone(),
                "entry".into(),
                "Main::launch() -> Unit".into(),
                program_entry_plan::ProgramEntrySourceReceiverSignature::Free,
                Vec::new(),
            )
            .unwrap();
        let (artifact, _, scope, _) = terminal_production::produce_program_entry_terminal_artifact(
            &checked,
            "Main::launch",
            signature.identity().bytes(),
        )
        .unwrap()
        .into_parts();
        let native = crate::realize_native_artifact_with_checked_boundary_operator_scope(
            artifact,
            &scope,
            crate::NativeRealizationRequest {
                target,
                subsystem: 3,
                profile: &profile,
                terminal_authority_policy: crate::current_terminal_authority_policy(),
                terminal_authority_permission_policy:
                    crate::current_terminal_authority_permission_policy(),
                program_entry: crate::NativeProgramEntrySettlement::new(&signature, None, &[]),
                optimization_selections: &selections,
                selected_provider_plans: &providers,
                external_binding_rows: &[],
                settlements: &[],
                compiler_builtins: &[],
                boundary_application_coverage: None,
                ieee_float_fma: &[],
                native_callbacks: &[],
                callback_thunks: &[],
            },
        )
        .unwrap_or_else(|errors| panic!("{target_profile:?}: {errors:?}"));
        assert!(matches!(
            native.physical_evidence_scope(),
            native_artifact::NativePhysicalEvidenceScope::ValidatedOptimizedProjection(_)
        ));
        assert!(native.physical_evidence().is_some());
        native
            .validate()
            .expect("native evidence independently replays after publication");
    }
}

#[test]
fn result_and_runtime_parameter_programs_stay_outside_return_migration() {
    let checked =
        crate::tests::fixtures::checked_source::checked("data Main {} machine Main::launch() {}");
    let artifact =
        terminal_production::produce_terminal_artifact(&checked, "Main::launch").unwrap();
    let input = lower_realization_input(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let original = input.plan();
    assert!(return_only_fragment_program(original));
    let value = semantic_vocabulary::ValueId::new(1).unwrap();
    let scalar_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    );
    let mut parameterized = original.clone();
    parameterized.functions[0]
        .parameters
        .push(abstract_operations::AbstractParameter { value, scalar_type });
    assert!(!return_only_fragment_program(&parameterized));
    let mut scalar = original.clone();
    scalar.functions[0].result =
        abstract_operations::AbstractFunctionResult::Scalar(abstract_operations::AbstractResult {
            value,
            scalar_type,
        });
    assert!(!return_only_fragment_program(&scalar));
    let mut extra_block = original.clone();
    extra_block.functions[0]
        .block_entries
        .push(original.functions[0].block_entries[0].clone());
    assert!(!return_only_fragment_program(&extra_block));
    let mut extra_operation = original.clone();
    extra_operation.functions[0]
        .operations
        .push(original.functions[0].operations[0].clone());
    assert!(!return_only_fragment_program(&extra_operation));
}
