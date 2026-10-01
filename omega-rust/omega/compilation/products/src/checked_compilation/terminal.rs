//! Admit and project checked product custody for Terminal production and handoff.
//!
//! These projections supply existing stage inputs and assemble reports; they do
//! not lower code, produce callback bodies, or execute native proposal production.
//! Omega stage 00 independently reconstructs occurrence and provider joins.

use super::CheckedCompilation;
use crate::{CompileReport, RetainedTerminalArtifact, TerminalCallbackThunkArtifact};
use diagnostics::Diagnostic;
use lowered_psi::{
    LoweredPsi, LoweredSelectedIeeeFloatComparisonOccurrence,
    LoweredSelectedIntegerComparisonOccurrence, LoweredSourceCallOccurrence,
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
        selected_dispatch::validate_selected_operator_terminal_custody(self)?;
        selected_dispatch::validate_fused_service_terminal_custody(
            self,
            execution.selected_provider_provenance.as_slice(),
        )?;
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

    /// Assemble a completed Terminal product with the source subject captured
    /// before lowering and the exact effective rollback receipt. Report custody
    /// and proof-product eligibility are checked by their existing constructors.
    pub fn terminal_report(
        &self,
        root_path: std::path::PathBuf,
        artifact: RetainedTerminalArtifact,
        production_subject: Option<crate::ProductionCompilationSubject>,
        rollback: Option<optimization_core::OptimizationRollbackReceipt>,
        profile: proof_admission::AdmissionProfile,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        CompileReport::from_retained_terminal_artifact(
            root_path,
            self.sources().source_file_count(),
            artifact,
            production_subject,
        )
        .and_then(|report| report.with_terminal_optimization_rollback(rollback))
        .and_then(|report| report.with_pcc_context(self.build().config.settings.pcc, profile))
        .map(|report| report.with_timings(self.timings().phases().to_vec()))
        .map_err(|message| vec![Diagnostic::error(message)])
    }
}

/// Rejoin an inspection product to its exact selected IEEE comparison meanings.
/// Portable verification cannot establish this source/provider association;
/// successful reconstruction grants no native execution authority.
pub fn validate_lowered_ieee_float_comparison_custody(
    checked: &CheckedCompilation,
    lowered: &LoweredPsi,
) -> Result<(), Vec<Diagnostic>> {
    terminal_psi_to_abstract_operations::settle_ieee_float_comparison_occurrences(
        checked,
        &lowered.semantic_module,
        &checked.execution().selected_provider_plan_facts,
        checked.execution().selected_provider_provenance.as_slice(),
        &lowered.selected_ieee_float_comparison_occurrences,
    )
    .map(|_| ())
}

/// Rejoin integer comparison occurrences to the admitted authored emission
/// triple. Swapped or negated recorded comparisons cannot substitute meanings.
pub fn validate_lowered_integer_comparison_custody(
    checked: &CheckedCompilation,
    lowered: &LoweredPsi,
) -> Result<(), Vec<Diagnostic>> {
    terminal_psi_to_abstract_operations::settle_integer_comparison_occurrences(
        checked,
        &lowered.semantic_module,
        &checked.execution().selected_provider_plan_facts,
        checked.execution().selected_provider_provenance.as_slice(),
        &lowered.selected_integer_comparison_occurrences,
    )
    .map(|_| ())
}
