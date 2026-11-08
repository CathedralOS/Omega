//! Supply the completed target choices needed to turn shared code into an executable.
//!
//! These projections supply existing stage inputs; they do not lower code,
//! produce callback bodies, execute native proposal production, or assemble a
//! report — `CompileReportSettlement` owns report construction.
//! Omega 00 joins the recorded occurrences to provider selection.

use super::CheckedCompilation;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::{
    BoundaryApplicationSettlement, TerminalNativeProposalRequest,
};

impl CheckedCompilation {
    /// Borrow the exact checked selections for a produced Terminal occurrence
    /// roster. Supplied callbacks remain candidates, not admitted native code;
    /// Omega 00 must check their cardinality, identities and source associations.
    /// Provider identities are already settled against the shared Terminal
    /// program; this projection reads the saved target choices.
    pub fn terminal_native_proposal_request<'input>(
        &'input self,
        selections: &optimization_core::OptimizationSelections,
        exported_calling_applications: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<TerminalNativeProposalRequest<'input>, Vec<Diagnostic>> {
        let execution = self.execution();
        let providers = self.providers();
        let native_sources = execution
            .native_provider_sources
            .as_ref()
            .map_err(Clone::clone)?;
        if native_sources.declarations.len() != execution.selected_provider_provenance.len() {
            return Err(vec![Diagnostic::error(
                "native provider declarations lost selected-plan alignment",
            )]);
        }
        let selected_provider_provenance = execution.selected_provider_provenance.iter()
            .zip(native_sources.declarations.iter())
            .map(|(retained, declarations)| {
                if retained.plan != declarations.plan {
                    return Err(vec![Diagnostic::error(
                        "native provider declarations substituted a selected plan",
                    )]);
                }
                Ok(terminal_psi_to_abstract_operations::provider_admission::CompilerIntrinsicPlanProvenance {
                    plan: &retained.plan,
                    row_executions: &retained.row_compiler_intrinsic_executions,
                    semantic_schema: &retained.plan.schema,
                    row_numeric_realizations: &declarations.row_numeric_realizations,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let (
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
        ) = execution.terminal_authority_permissions();
        Ok(TerminalNativeProposalRequest {
            evaluated_binding_identities: providers.evaluated_via_bindings.identities(),
            direct_boundary_leaves: execution.direct_boundary_leaves.iter().collect(),
            provider_declarations: &native_sources.declarations,
            family_realizations: &native_sources.family_realizations,
            target_profile: execution.selected_target_profile,
            boundaries: BoundaryApplicationSettlement {
                selected_provider_plans: execution.plans(),
                selected_provider_provenance,
                calling_applications: providers
                    .calling_applications(exported_calling_applications)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?,
            },
            native_target: execution.selected_native_target(),
            program_entry: execution.selected_program_entry.as_ref().map(|entry| {
                terminal_psi_to_abstract_operations::NativeProgramEntrySettlement::new(
                    entry.source_signature(),
                    entry.calling_plans().map(|plans| {
                        (
                            &plans.semantic_calling_application,
                            &plans.physical_calling_application,
                            &plans.storage_entry,
                        )
                    }),
                )
            }),
            application: self.build_admission().native_application_metadata(
                &self.build().config.settings,
                execution.selected_target_profile,
            ),
            optimizations: selections.project_post_terminal(),
            external_bindings: &providers.external_binding_rows,
            behavior_exclusions: self.build().config.behavior_exclusions.as_slice(),
            package_terminal_authority_permissions,
            package_terminal_authority_permission_identities,
            x86_scalar_fma_provider: execution.x86_scalar_fma_provider,
        })
    }
}
