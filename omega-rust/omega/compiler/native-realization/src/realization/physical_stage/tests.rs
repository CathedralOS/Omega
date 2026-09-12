use super::*;
use crate::realization::input::lower_realization_input;
use crate::realization::optimization_stage::lower_realization_optimization_stage;
use crate::realization::optimized_fragment_projection::{
    OptimizedFragmentPublicationRequest, emit_optimized_fragments,
};
use crate::realization::target_stage::lower_realization_target_stage;
mod conditional;
mod conditional_fixture;
mod ordered_calls;

#[test]
fn return_programs_publish_replayable_native_evidence_on_every_target() {
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
        let (artifact, _, scope, _, _) =
            terminal_production::produce_program_entry_terminal_artifact(
                &checked,
                "Main::launch",
                signature.identity().bytes(),
            )
            .unwrap()
            .into_parts();
        let native = crate::realize_native_artifact(
            artifact,
            crate::NativeRealizationRequest {
                checked_scope: Some(&scope),
                prepared_input: None,
                target,
                image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
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
        .unwrap_or_else(|errors| panic!("{target_profile:?}: {errors:?}"))
        .into_direct()
        .expect("direct image requested");
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
fn malformed_unit_inputs_reject_at_legalization() {
    let checked =
        crate::tests::fixtures::checked_source::checked("data Main {} machine Main::launch() {}");
    let artifact =
        terminal_production::produce_terminal_artifact(&checked, "Main::launch").unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let optimized = crate::optimize_verified_abstract_input(
        input,
        crate::compiler_baseline_request_v1(&optimization_core::OptimizationSelections::default()),
    )
    .unwrap();
    let target = abstract_operations_to_target_operations::lower_optimized_to_target_operations(
        optimized,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let original = target.optimized().plan();
    let admitted = |plan: &abstract_operations::AbstractOperationPlan| {
        target_operations_to_selected_instructions::legalize_target_operations(
            target.target_operations(),
            plan,
            target.optimized().unit(),
        )
        .is_ok()
    };
    assert!(admitted(original));
    let value = semantic_vocabulary::ValueId::new(1).unwrap();
    let scalar_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    );
    let mut parameterized = original.clone();
    parameterized.functions[0]
        .parameters
        .push(abstract_operations::AbstractParameter { value, scalar_type });
    assert!(!admitted(&parameterized));
    let mut scalar = original.clone();
    scalar.functions[0].result =
        abstract_operations::AbstractFunctionResult::Scalar(abstract_operations::AbstractResult {
            value,
            scalar_type,
        });
    assert!(!admitted(&scalar));
    let mut extra_block = original.clone();
    extra_block.functions[0]
        .block_entries
        .push(original.functions[0].block_entries[0].clone());
    assert!(!admitted(&extra_block));
    let mut extra_operation = original.clone();
    extra_operation.functions[0]
        .operations
        .push(original.functions[0].operations[0].clone());
    assert!(!admitted(&extra_operation));
}
