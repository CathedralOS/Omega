use omega_optimization_core::OptimizationSelections;
use psi_diagnostics::Diagnostic;

pub(super) fn realize(
    checked: &crate::pipeline::CheckedCompilation,
    admission: super::admission::NativeOptimizationAdmission<'_>,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &OptimizationSelections,
) -> Result<omega_terminal_psi_to_native_artifact::NativeArtifact, Vec<Diagnostic>> {
    let entry_machine = admission.program_entry.machine_name().to_owned();
    let produced =
        psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            checked,
            &entry_machine,
        ).map_err(
            |error| {
                vec![Diagnostic::error(format!(
                    "native-artifact Terminal production failed: {error}"
                ))]
            },
        )?;
    let (artifact, checked_boundary_operator_scope, selected_ieee_float_fma_occurrences) =
        produced.into_parts();
    if !selected_ieee_float_fma_occurrences.is_empty() {
        return Err(vec![Diagnostic::error(
            "optimized direct native realization does not yet consume retained IEEE-FMA occurrence custody",
        )]);
    }
    let terminal_module = psi_terminal_codec::decode_module(artifact.semantic_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "native-artifact intrinsic settlement could not replay canonical Terminal semantics: {error}"
            ))]
        },
    )?;
    let demanded_intrinsics =
        crate::compiler::intrinsic_settlements::demanded_boundary_identities(&terminal_module)?;
    let intrinsic_proposals =
        crate::compiler::intrinsic_settlements::derive_compiler_intrinsic_settlement_proposals(
            checked,
            &demanded_intrinsics,
        )?;
    let selected_plans = checked.selected_provider_plans().plans();
    let compiler_builtins = intrinsic_proposals
        .iter()
        .map(
            |proposal| omega_terminal_psi_to_native_artifact::NativeCompilerBuiltinSettlement {
                requirement_identity: &proposal.requirement_identity,
                provider_plan: &selected_plans[proposal.plan_index],
                execution: proposal.execution,
            },
        )
        .collect::<Vec<_>>();
    let calling_plans = admission
        .program_entry
        .calling_plans()
        .map(|plans| (&plans.semantic_boundary_entry_plan, &plans.storage_entry));
    let program_entry = omega_terminal_psi_to_native_artifact::NativeProgramEntrySettlement::new(
        admission.program_entry.source_signature(),
        calling_plans,
    );
    omega_terminal_psi_to_native_artifact::realize_native_artifact_with_checked_boundary_operator_scope(
        artifact,
        &checked_boundary_operator_scope,
        omega_terminal_psi_to_native_artifact::NativeRealizationRequest {
            target: admission.target,
            subsystem: checked.subsystem(),
            profile,
            terminal_authority_policy:
                omega_terminal_psi_to_native_artifact::current_compiler_intrinsic_terminal_authority_policy(),
            program_entry,
            optimization_selections,
            selected_provider_plans: checked.selected_provider_plans(),
            external_binding_rows: checked.external_binding_rows(),
            settlements: &[],
            compiler_builtins: &compiler_builtins,
            ieee_float_fma: &[],
            native_callbacks: &[],
        },
    )
}
