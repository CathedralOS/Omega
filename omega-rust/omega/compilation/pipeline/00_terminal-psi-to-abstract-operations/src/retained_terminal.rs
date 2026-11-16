/*
  Retained Terminal product (`RetainedTerminalArtifact`) and target-constrained
  native realization proposal (`TerminalNativeRealizationProposal`,
  `project_terminal_native_realization_proposal`).

  Retains canonical semantics, program-entry custody, provider choices, callbacks
  and registrar imports without replaying source joins. Native preparation uses
  the final entry and callback roster to select functions, then settles their
  boundary coverage and comparisons before lowering those same functions.
*/
#[path = "retained_terminal_codec.rs"]
mod codec;
pub use codec::{
    decode_installation_assumptions, encode_installation_assumptions,
    recover_native_derivation_inputs, retain_native_derivation_inputs,
};

pub use verified_optimization_input::component_inputs::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    RetainedComponentNativeInputs,
};

mod native_providers {
    use super::TerminalNativeRealizationProposal;
    use crate::providers::provider_admission::{
        NativeCompilerBuiltinSettlement, NativeProviderSettlement, PreparedSourceEvaluatedBindings,
        ProviderAdmissionRequest, SourceEvaluatedImportSettlement,
        TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
        prepare_source_evaluated_bindings,
    };
    use diagnostics::Diagnostic;
    use std::collections::BTreeSet;

    pub struct RetainedNativeProviderInputs<'input> {
        pub execution: &'input PreparedNativeExecution,
        pub optimization_selections: &'input optimization_core::PostTerminalOptimizationSelections,
        pub accepted_package_permissions: &'input TerminalAuthorityPermissionPolicy,
        pub imports: &'input [SourceEvaluatedImportSettlement<'input>],
    }

    pub use verified_optimization_input::PreparedNativeExecution;

    pub fn lower_native_callable(
        execution: &PreparedNativeExecution,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        proposal: &TerminalNativeRealizationProposal,
        entry: terminal_psi::TargetMachineCallable,
        scalar_applications: Option<crate::NativeScalarApplications<'_>>,
    ) -> Result<
        (
            crate::VerifiedNativeArtifactInput,
            boundary_applications::TerminalBoundaryApplicationCoverage,
        ),
        Vec<Diagnostic>,
    > {
        execution
            .authorization()
            .require_artifact(artifact)
            .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        let checked_scope = proposal.callable_execution_scope(artifact, entry, Some(execution))?;
        let selection = crate::NativeExecutionSelection {
            scope: Some(&checked_scope),
            scalar_applications,
            providers: checked_scope.local_adapters(),
            plans: &proposal.providers().selection,
            callbacks: checked_scope.callbacks(),
            entry_call: checked_scope.entry_call(),
        };
        let scope = crate::artifact_admission::NativeScopePreparation::prepare(
            execution.authorization(),
            proposal.target_profile(),
            entry,
            &selection,
        )
        .map_err(crate::native_admission::artifact_lowering_error)?;
        let provenance = proposal.selected_intrinsic_provenance(execution.selection())?;
        let coverage = proposal.settle_scoped_boundary_applications(
            artifact,
            &checked_scope,
            execution.selection(),
            &provenance,
        )?;
        let input = scope
            .lower(
                artifact,
                &[],
                &selection,
                execution.evidence().map(std::sync::Arc::as_ref),
            )
            .map_err(crate::native_admission::artifact_lowering_error)?;
        let input = input
            .bind_selected_provider_coordinates(artifact, execution.selection())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok((input, coverage))
    }

    /// Lower one prepared native execution to the abstract program in a single
    /// pass. The execution scope, its provider demand and the scalar values it
    /// reads are all settled before Omega 00 starts, so this stage runs once,
    /// front to back, with no caller work between preparing and lowering.
    pub fn lower_native_execution(
        proposal: &TerminalNativeRealizationProposal,
        execution: PreparedNativeExecution,
        private_entry_ram: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        scalar_applications: Option<crate::NativeScalarApplications<'_>>,
    ) -> Result<(crate::VerifiedNativeArtifactInput, PreparedNativeExecution), Vec<Diagnostic>>
    {
        let authorization = execution.authorization().clone();
        let artifact = authorization.artifact();
        let checked = execution.scope();
        let scope = crate::artifact_admission::NativeScopePreparation::prepare(
            &authorization,
            proposal.target_profile(),
            terminal_psi::TargetMachineCallable::Machine(checked.entry()),
            &crate::NativeExecutionSelection {
                scope: Some(checked),
                scalar_applications: None,
                providers: checked.local_adapters(),
                plans: &proposal.providers().selection,
                callbacks: checked.callbacks(),
                entry_call: checked.entry_call(),
            },
        )
        .map_err(crate::native_admission::artifact_lowering_error)?;
        let input = scope
            .lower_with_private_entry_ram(
                artifact,
                private_entry_ram,
                &crate::NativeExecutionSelection {
                    scope: Some(execution.scope()),
                    scalar_applications,
                    providers: execution.local_adapters(),
                    plans: &proposal.providers().selection,
                    callbacks: &proposal.occurrences().callbacks,
                    entry_call: execution.scope().entry_call(),
                },
                execution.evidence().map(std::sync::Arc::as_ref),
            )
            .map_err(crate::native_admission::artifact_lowering_error)?;
        let input = input
            .bind_selected_provider_coordinates(artifact, execution.selection())
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        Ok((input, execution))
    }

    pub struct PreparedRetainedNativeProviders<'input> {
        execution: &'input PreparedNativeExecution,
        proposal: &'input TerminalNativeRealizationProposal,
        bindings: PreparedSourceEvaluatedBindings<'input>,
        program_demands: BTreeSet<String>,
        callbacks: &'input [crate::NativeCallbackArgumentProposal],
        pub callback_thunks: Vec<backend_plan::NativeCallbackThunkSettlement<'input>>,
        compiler_builtins: Vec<NativeCompilerBuiltinSettlement<'input>>,
    }

    impl PreparedRetainedNativeProviders<'_> {
        pub fn program_settlements(&self) -> Vec<NativeProviderSettlement<'_>> {
            self.settlements_for(&self.program_demands)
        }

        fn settlements_for(&self, demands: &BTreeSet<String>) -> Vec<NativeProviderSettlement<'_>> {
            self.bindings
                .settlements()
                .iter()
                .filter(|settlement| {
                    demands.contains(settlement.provider_execution.requirement_identity())
                })
                .copied()
                .collect()
        }

        pub fn admission_request<'request>(
            &'request self,
            settlements: &'request [NativeProviderSettlement<'request>],
            terminal_authority_policy: &'request TerminalAuthorityPolicy,
            receiving_permissions: &'request Option<TerminalAuthorityPermissionPolicy>,
            installation_assumptions: &'request [installation_evidence::InstallationEnvironmentAssumption],
        ) -> crate::NativeAdmissionRequest<'request, 'request> {
            let proposal = self.proposal;
            crate::NativeAdmissionRequest {
                private_entry_ram: &[],
                entry: terminal_psi::TargetMachineCallable::Machine(
                    proposal.program_entry().checked_entry().terminal_entry(),
                ),
                boundary_application_coverage: Some(self.execution.boundary_application_coverage()),
                behavior_exclusions: proposal.behavior_exclusions(),
                providers: ProviderAdmissionRequest {
                    profile: proposal.target_profile(),
                    bootstrap_entry: proposal.program_entry().checked_entry().terminal_entry(),
                    program_provider_context: None,
                    installation_assumptions,
                    selected_provider_plans: self.execution.selection(),
                    checked_adapters: self.execution.local_adapters(),
                    external_binding_rows: self.execution.external_bindings(),
                    settlements,
                    compiler_builtins: &self.compiler_builtins,
                    native_bindings: &proposal.providers().native_bindings,
                    native_callbacks: self.callbacks,
                    installed_interrupts: &proposal.occurrences().installed_interrupts,
                    installed_calling: &proposal.boundary_applications().calling,
                    callback_occurrences: &proposal.occurrences().callbacks,
                    terminal_authority_policy,
                    terminal_authority_permission_policy: receiving_permissions,
                    x86_scalar_fma_provider: proposal.providers().x86_scalar_fma,
                },
            }
        }
    }

    pub fn prepare_native_providers<'input>(
        proposal: &'input TerminalNativeRealizationProposal,
        _artifact: &terminal_codec::CanonicalTerminalArtifact,
        inputs: RetainedNativeProviderInputs<'input>,
    ) -> Result<PreparedRetainedNativeProviders<'input>, Vec<Diagnostic>> {
        if proposal.post_terminal_optimizations().selections() != inputs.optimization_selections {
            return Err(diagnostics::error_in_context(
                "Terminal native proposal",
                "receiving lowerer selections differ from the exact post-Terminal build proposal",
            ));
        }
        crate::providers::provider_admission::terminal_authority_permissions::validate_retained_package_terminal_authority_permissions(
            proposal.package_terminal_authority_permissions(),
            proposal.package_terminal_authority_permission_identities(),
            inputs.accepted_package_permissions,
        )?;
        let callbacks = proposal
            .occurrences()
            .callback_arguments
            .as_deref()
            .ok_or_else(|| {
                diagnostics::error_in_context(
                    "Terminal callback custody",
                    "callback placement has no private registrar materialization",
                )
            })?;
        let callback_thunks =
            backend_plan::native_callback_thunks(proposal.occurrences().callbacks.as_slice());

        let program_demands = inputs.execution.demands().clone();
        let bindings = prepare_source_evaluated_bindings(
            inputs.execution.selection().plans(),
            &program_demands,
            inputs.imports,
            inputs.execution.selection().identities(),
        )?;

        let compiler_builtins =
            crate::providers::provider_admission::prepare_retained_compiler_builtins(
                inputs.execution.compiler_builtins(),
                inputs.execution.selection().plans(),
            )?;

        Ok(PreparedRetainedNativeProviders {
            execution: inputs.execution,
            proposal,
            bindings,
            program_demands,
            callbacks,
            callback_thunks,
            compiler_builtins,
        })
    }
}

pub use backend_plan::TerminalTargetProviderProposal;
pub use backend_plan::{TerminalCallbackOccurrenceProposal, TerminalCallbackThunkArtifact};
pub use native_providers::{
    PreparedNativeExecution, PreparedRetainedNativeProviders, RetainedNativeProviderInputs,
    lower_native_callable, lower_native_execution, prepare_native_providers,
};

pub use crate::providers::provider_admission::TerminalCompilerBuiltinProposal;

pub use boundary_applications::{
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
};

pub use verified_optimization_input::RetainedTerminalArtifact;
pub use verified_optimization_input::native_proposal::{
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProposalRequest,
    TerminalNativeProviders, TerminalNativeRealizationProposal,
    project_terminal_native_realization_proposal,
};
