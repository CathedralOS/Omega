//! Opaque callback custody across successful and rejected native realization.

use crate::tests::fixtures::hosted::hosted_custody;
use crate::{
    NativeProgramEntrySettlement, NativeRealizationRequest,
    current_compiler_intrinsic_terminal_authority_policy,
    realize_native_artifact_with_callback_custody,
};

#[test]
fn native_realization_returns_exact_ordered_callback_custody_on_success() {
    let (artifact, _, source) = hosted_custody();
    let profile = psi_proof_admission::AdmissionProfile::default();
    let optimizations = omega_optimization_core::OptimizationSelections::default();
    let providers = omega_effects::SelectedProviderPlanFacts::default();
    let custody = vec![(11u64, "first"), (29u64, "second")];

    let realized = realize_native_artifact_with_callback_custody(
        artifact,
        NativeRealizationRequest {
            target: omega_target::NativeTarget::windows_x64(),
            subsystem: 3,
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            program_entry: NativeProgramEntrySettlement::new(&source, None),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
        },
        custody.clone(),
    )
    .expect("opaque callback custody crosses successful native realization");

    assert_eq!(realized.callback_custody(), &custody);
    realized
        .artifact()
        .validate()
        .expect("source-free native artifact remains independently valid");
    let (_, returned) = realized.into_parts();
    assert_eq!(returned, custody);
}

#[test]
fn native_realization_rejection_returns_callback_custody_without_reordering() {
    let (artifact, _, source) = hosted_custody();
    let profile = psi_proof_admission::AdmissionProfile::default();
    let optimizations = omega_optimization_core::OptimizationSelections::default();
    let providers = omega_effects::SelectedProviderPlanFacts::default();
    let swapped = vec![(29u64, "second"), (11u64, "first")];

    let rejected = realize_native_artifact_with_callback_custody(
        artifact,
        NativeRealizationRequest {
            target: omega_target::NativeTarget::linux_x64(),
            subsystem: 0,
            profile: &profile,
            terminal_authority_policy: current_compiler_intrinsic_terminal_authority_policy(),
            program_entry: NativeProgramEntrySettlement::new(&source, None),
            optimization_selections: &optimizations,
            selected_provider_plans: &providers,
            external_binding_rows: &[],
            settlements: &[],
            compiler_builtins: &[],
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
    let (_, returned) = rejected.into_parts();
    assert_eq!(returned, swapped);
}
