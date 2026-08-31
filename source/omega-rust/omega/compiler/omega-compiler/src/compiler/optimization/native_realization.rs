use omega_optimization_core::OptimizationSelections;
use psi_diagnostics::Diagnostic;

pub(super) fn realize(
    checked: &crate::pipeline::CheckedCompilation,
    admission: super::admission::NativeOptimizationAdmission<'_>,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &OptimizationSelections,
) -> Result<omega_terminal_psi_to_native_artifact::NativeArtifact, Vec<Diagnostic>> {
    let entry_machine = admission.program_entry.machine_name().to_owned();
    let artifact =
        psi_checked_trees_to_terminal::produce_terminal_artifact(checked, &entry_machine).map_err(
            |error| {
                vec![Diagnostic::error(format!(
                    "native-artifact Terminal production failed: {error}"
                ))]
            },
        )?;
    let terminal_module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "native-artifact intrinsic settlement could not replay canonical Terminal semantics: {error}"
            ))]
        },
    )?;
    let demanded_intrinsics =
        crate::compiler::intrinsic_settlements::demanded_boundary_identities(&terminal_module)?;
    let intrinsic_evidence =
        crate::compiler::intrinsic_settlements::derive_compiler_intrinsic_settlement_evidence(
            checked,
            &demanded_intrinsics,
        )?;
    let selected_plans = checked.selected_provider_plans().plans();
    let intrinsic_settlements = intrinsic_evidence
        .iter()
        .map(|evidence| {
            let realization = match evidence.execution {
                omega_provider_planning::plans::CompilerIntrinsicExecutionIdentity::LinuxExitGroupI32 => {
                    omega_target_operations::LinuxExitGroupI32Realization.into()
                }
                _ => unreachable!("native intrinsic evidence admits only cataloged boundary realizations"),
            };
            omega_terminal_psi_to_native_artifact::NativeProviderSettlement {
                provider_execution: evidence,
                provider_plan: &selected_plans[evidence.plan_index],
                realization,
            }
        })
        .collect::<Vec<_>>();
    let calling_plans = admission
        .program_entry
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        admission.program_entry.source_signature(),
        calling_plans,
    );
    omega_terminal_psi_to_native_artifact::realize_native_artifact(
        artifact,
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target: admission.target,
            subsystem: checked.subsystem(),
            profile,
            program_entry,
            optimization_selections,
            selected_provider_plans: checked.selected_provider_plans(),
            external_binding_rows: checked.external_binding_rows(),
            settlements: &intrinsic_settlements,
        },
    )
}
