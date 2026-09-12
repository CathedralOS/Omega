//! Opaque callback custody across successful and rejected native realization.

use crate::tests::fixtures::hosted::hosted_custody;
use crate::{
    NativeProgramEntrySettlement, NativeRealizationRequest,
    current_compiler_intrinsic_terminal_authority_policy,
    current_terminal_authority_permission_policy, realize_native_artifact_with_callback_custody,
};

#[test]
fn native_realization_returns_exact_ordered_callback_custody_on_success() {
    let (artifact, _, source) = hosted_custody();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let custody = vec![(11u64, "first"), (29u64, "second")];

    let realized = realize_native_artifact_with_callback_custody(
        artifact,
        NativeRealizationRequest {
            checked_scope: None,
            prepared_input: None,
            target: target::NativeTarget::windows_x64(),
            image_request: image_emission::ExecutableImageEmissionRequest::direct(3),
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            terminal_authority_permission_policy: current_terminal_authority_permission_policy(),
            program_entry: NativeProgramEntrySettlement::new(&source, None, &[]),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
        },
        custody.clone(),
    )
    .expect("opaque callback custody crosses successful native realization");

    assert_eq!(realized.callback_custody(), &custody);
    realized
        .artifact()
        .as_direct()
        .expect("direct image requested")
        .validate()
        .expect("source-free native artifact remains independently valid");
    let (_, returned) = realized.into_parts();
    assert_eq!(returned, custody);
}

#[test]
fn native_realization_rejection_returns_callback_custody_without_reordering() {
    let (artifact, _, source) = hosted_custody();
    let profile = proof_admission::AdmissionProfile::default();
    let optimizations = optimization_core::PostTerminalOptimizationSelections::default();
    let providers = effects::SelectedProviderPlanFacts::default();
    let swapped = vec![(29u64, "second"), (11u64, "first")];
    let interpreter = target::normalize_elf_interpreter_plan(
        b"/lib/ld-linux-x86-64.so.2".to_vec(),
        target::TargetProfile::LinuxX64,
    )
    .unwrap();

    let rejected = realize_native_artifact_with_callback_custody(
        artifact,
        NativeRealizationRequest {
            checked_scope: None,
            prepared_input: None,
            target: target::NativeTarget::linux_x64(),
            image_request: image_emission::ExecutableImageEmissionRequest::dynamic_elf(
                interpreter.clone(),
            ),
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            terminal_authority_permission_policy: current_terminal_authority_permission_policy(),
            program_entry: NativeProgramEntrySettlement::new(&source, None, &[]),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
            boundary_application_coverage: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
            callback_thunks: &[],
        },
        swapped.clone(),
    )
    .expect_err("ProgramEntry target drift rejects native realization");

    assert_eq!(rejected.diagnostics().len(), 1);
    assert!(
        rejected.diagnostics()[0]
            .message
            .contains("native artifact ProgramEntry custody failed")
    );
    assert_eq!(rejected.callback_custody(), &swapped);
    let (realization, returned) = rejected.into_parts();
    assert_eq!(returned, swapped);
    let (image_request, _) = realization.into_parts();
    let image_emission::ExecutableImageEmissionRequest::DynamicElf {
        interpreter: returned,
    } = image_request
    else {
        panic!("rejection must retain dynamic image custody");
    };
    assert_eq!(returned, interpreter);
}
