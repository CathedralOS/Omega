//! Admit and project checked product custody for Terminal production and handoff.
//!
//! These projections supply existing stage inputs; they do not lower code,
//! produce callback bodies, execute native proposal production, or assemble a
//! report — `CompileReportSettlement` owns report construction.
//! Omega stage 00 joins the recorded occurrences to provider selection.

use super::CheckedCompilation;
use crate::TerminalCallbackThunkArtifact;
use diagnostics::Diagnostic;
use lowered_psi::{
    LoweredSelectedIeeeFloatComparisonOccurrence, LoweredSelectedIntegerComparisonOccurrence,
    LoweredSourceCallOccurrence,
};
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
        callback_placements: &'input [backend_plan::BoundNominalCallbackPlacement],
        callback_thunks: Vec<TerminalCallbackThunkArtifact>,
        source_call_occurrences: &'input [LoweredSourceCallOccurrence],
        selected_ieee_float_comparison_occurrences: &'input [LoweredSelectedIeeeFloatComparisonOccurrence],
        selected_integer_comparison_occurrences: &'input [LoweredSelectedIntegerComparisonOccurrence],
        selections: &optimization_core::OptimizationSelections,
    ) -> TerminalNativeProposalRequest<'input> {
        let execution = self.execution();
        let providers = self.providers();
        TerminalNativeProposalRequest {
            boundaries: BoundaryApplicationSettlement {
                checked: self,
                selected_provider_plans: &execution.selected_provider_plan_facts,
                selected_provider_provenance: execution.selected_provider_provenance.as_slice(),
                exact_toolchain_sources: self.sources().exact_toolchain_sources(),
                target_profile: execution.selected_target_profile,
                console_exit_binding: execution.resolved_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                ),
                process_exit_binding: execution.resolved_semantic_binding(
                    package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
                ),
                calling_plan_realizations: &providers.boundary_calling_plan_realizations,
            },
            native_target: execution.selected_native_target(),
            program_entry: execution.selected_program_entry.as_ref(),
            application: self.build_admission().native_application_metadata(
                &self.build().config.settings,
                execution.selected_target_profile,
            ),
            optimizations: selections.project_post_terminal(),
            external_bindings: &providers.external_binding_rows,
            behavior_exclusions: self.build().config.behavior_exclusions.as_slice(),
            package_terminal_authority_permissions: execution.terminal_authority_permissions(),
            callback_placements,
            callback_thunks,
            source_call_occurrences,
            selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences,
        }
    }
}
