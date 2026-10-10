//! Admit and project checked product custody for Terminal production and handoff.
//!
//! These projections supply existing stage inputs; they do not lower code,
//! produce callback bodies, execute native proposal production, or assemble a
//! report — `CompileReportSettlement` owns report construction.
//! Omega stage 00 joins the recorded occurrences to provider selection.

use super::CheckedCompilation;
use crate::TerminalCallbackThunkArtifact;
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::{
    BoundaryApplicationSettlement, TerminalNativeProposalRequest,
};

impl CheckedCompilation {
    /// Admit the checked product for Terminal production and borrow its exact
    /// entry. This is product eligibility, not proof or native admission of the
    /// artifact that Psi has yet to produce.
    pub fn admit_terminal_production(
        &self,
    ) -> Result<&provider_planning::program_entry::SelectedCompilerProgramEntry, Vec<Diagnostic>>
    {
        // An Independent edge has no product carrier yet. Reject it before
        // checking entry custody or allowing a fused artifact to be produced.
        let execution = self.execution();
        provider_planning::verify_selected_compositions_are_realized(
            execution.selected_provider_provenance.as_slice(),
        )?;
        // Preserve the build operand's lexical package choice, including when
        // another package contains a machine with the same display name.
        let entry = execution.selected_program_entry.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "terminal-artifact production requires one exact selected program entry",
            )]
        })?;
        Ok(entry)
    }

    /// Borrow the exact checked selections for a produced Terminal occurrence
    /// roster. Supplied callbacks remain candidates, not admitted native code;
    /// stage 00 must check their cardinality, identities and source associations.
    pub fn terminal_native_proposal_request<'input>(
        &'input self,
        callback_placements: &[provider_planning::calling_policy_plans::SelectedCallbackPlacement],
        callback_thunks: Vec<TerminalCallbackThunkArtifact>,
        selections: &optimization_core::OptimizationSelections,
    ) -> TerminalNativeProposalRequest<'input> {
        let execution = self.execution();
        let providers = self.providers();
        TerminalNativeProposalRequest {
            target_profile: execution.selected_target_profile,
            boundaries: BoundaryApplicationSettlement {
                selected_provider_plans: &execution.selected_provider_plan_facts,
                selected_provider_provenance: execution.selected_provider_provenance.iter().map(|retained| {
                    terminal_psi_to_abstract_operations::provider_admission::CompilerIntrinsicPlanProvenance {
                        plan: &retained.plan, row_executions: &retained.row_compiler_intrinsic_executions,
                    }
                }).collect(),
                calling_applications: providers.boundary_calling_plan_realizations.iter()
                    .map(|realization| realization.native_application()).collect(),
            },
            native_target: execution.selected_native_target(),
            program_entry: execution.selected_program_entry.as_ref().map(|entry| {
                terminal_psi_to_abstract_operations::NativeProgramEntrySettlement::new(
                    entry.source_signature(),
                    entry.calling_plans().map(|plans| (
                        &plans.semantic_calling_application,
                        &plans.physical_calling_application,
                        &plans.storage_entry,
                    )),
                    entry.fused_service_establishments(),
                )
            }),
            application: self.build_admission().native_application_metadata(
                &self.build().config.settings,
                execution.selected_target_profile,
            ),
            optimizations: selections.project_post_terminal(),
            external_bindings: &providers.external_binding_rows,
            behavior_exclusions: self.build().config.behavior_exclusions.as_slice(),
            package_terminal_authority_permissions: execution.terminal_authority_permissions(),
            callback_placements: callback_placements.iter().map(|placement| placement.native.clone()).collect(),
            callback_thunks,
            x86_scalar_fma_provider: execution.x86_scalar_fma_provider,
        }
    }
}
