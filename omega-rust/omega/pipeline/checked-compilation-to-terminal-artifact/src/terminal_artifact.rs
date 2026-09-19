//! Checked compilation to the canonical Terminal artifact.
//!
//! Two products leave here. The retained Terminal product carries its
//! callback custody and a native-realization proposal, and returns as the
//! compiler report; the program-entry artifact is what the direct native
//! route realizes. Both replay the canonical semantic and proof sections
//! under the request admission profile before anything downstream sees them.

use crate::{
    application_coverage, float_comparisons, float_fma, integer_comparisons, native_proposal,
};
use assembled_syntax_to_checked_compilation::{CheckedCompilation, OptimizationRollback};
use diagnostics::Diagnostic;

pub(crate) mod behavior_exclusions;
pub(crate) mod verification;

/// Produce the retained Terminal product and its ordinary compiler report.
pub fn produce_terminal_report(
    root_path: std::path::PathBuf,
    checked: CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    rollback: &OptimizationRollback,
) -> Result<compilation_report::CompileReport, Vec<Diagnostic>> {
    let pcc_requests = checked.pcc_requests();
    let production_subject = checked.production_subject()?;
    let source_file_count = checked.source_file_count();
    let rollback = rollback.settle(checked.optimization_selections());
    let artifact = produce_retained_terminal_artifact(&checked, profile, rollback.effective())?;
    compilation_report::CompileReport::from_retained_terminal_artifact(
        root_path,
        source_file_count,
        artifact,
        production_subject,
    )
    .and_then(|report| report.with_terminal_optimization_rollback(rollback.into_receipt()))
    .map(|report| report.with_pcc_context(pcc_requests, profile.clone()))
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// Rejoin a checked-source inspection product to its exact selected IEEE
/// comparison meanings. Portable verification alone cannot establish this
/// source/provider association, and this check grants no native execution.
pub fn validate_lowered_ieee_float_comparison_custody(
    checked: &CheckedCompilation,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<(), Vec<Diagnostic>> {
    float_comparisons::associate(
        checked,
        &lowered.semantic_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        &lowered.selected_ieee_float_comparison_occurrences,
    )
    .map(|_| ())
}

/// Rejoin a checked-source inspection product to its exact selected integer
/// comparison meanings — the integer counterpart of
/// [`validate_lowered_ieee_float_comparison_custody`]. The recorded emission
/// triple must be the admitted emission of the authored spelling, so a
/// swapped or negated comparison cannot substitute a different selected
/// semantic. This check grants no native execution.
pub fn validate_lowered_integer_comparison_custody(
    checked: &CheckedCompilation,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<(), Vec<Diagnostic>> {
    integer_comparisons::associate(
        checked,
        &lowered.semantic_module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        &lowered.selected_integer_comparison_occurrences,
    )
    .map(|_| ())
}

/// Produce one verified retained Terminal product from the complete checked
/// frontend result.
///
/// This owner closes the checked-to-Terminal boundary, replays the canonical
/// semantic and proof sections under the request admission profile, and binds
/// the target-owned native-realization proposal. It does not assemble a
/// compiler report or enter native realization.
fn produce_retained_terminal_artifact(
    checked: &CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    selections: &optimization_core::OptimizationSelections,
) -> Result<compilation_report::RetainedTerminalArtifact, Vec<Diagnostic>> {
    let callback_placements = checked.callback_placements().to_vec();
    // The selected entry rejoins Terminal production by its exact checked
    // machine symbol: the build product operand's lexical package choice must
    // survive a same-named declaration in another package. The retained
    // product also carries the checked entry receipt so later native re-entry
    // can settle the hosted receiver without the checked frontend.
    let selected_program_entry = checked.selected_program_entry().ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production requires one exact selected program entry",
        )]
    })?;
    let entry_machine_symbol = selected_program_entry.source_signature().machine_symbol();
    let source_signature_identity = selected_program_entry.source_signature().identity().bytes();
    selected_dispatch::validate_selected_operator_terminal_custody(
        checked,
        checked.selected_provider_plans(),
    )?;
    selected_dispatch::validate_fused_service_terminal_custody(
        checked,
        checked.selected_provider_provenance(),
    )?;
    // The root and provider selections are now fixed; the authored behavior
    // exclusions must hold in the unoptimized composition before the product
    // is produced and admitted.
    behavior_exclusions::verify_entry_behavior_exclusions(checked, entry_machine_symbol)?;
    let psi_optimizations = selections.project_psi();
    let terminal_trees = checked.terminal_production_trees();
    let produced = terminal_production::TerminalProductionRequest {
        checked: terminal_trees,
        machine: terminal_production::TerminalMachineSelection::Symbol(entry_machine_symbol),
        optimization_selections: psi_optimizations.selections().clone(),
    }
    .produce_program_entry_with_callback_custody(source_signature_identity, callback_placements)
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact production failed: {}",
            error.error(),
        ))]
    })?;
    let (
        artifact,
        checked_program_entry,
        checked_boundary_operator_scope,
        callback_placements,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts_with_source_calls();
    verification::verify_terminal_artifact(&artifact, profile)?;
    let native_realization_proposal =
        native_proposal::project_terminal_native_realization_proposal(
            checked,
            profile,
            &artifact,
            checked_program_entry,
            checked_boundary_operator_scope,
            &callback_placements,
            &source_call_occurrences,
            &selected_ieee_float_fma_occurrences,
            &selected_ieee_float_comparison_occurrences,
            &selected_integer_comparison_occurrences,
            selections,
        )?;
    compilation_report::RetainedTerminalArtifact::new_with_native_realization_proposal(
        artifact,
        callback_placements,
        native_realization_proposal,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

/// The Terminal artifact produced for one exact selected program entry, with
/// the checked receipts and application coverage that native realization
/// rejoins. Its fields are read only by consuming it.
pub struct ProgramEntryTerminalArtifact {
    artifact: terminal_codec::CanonicalTerminalArtifact,
    checked_program_entry: terminal_psi::CheckedProgramEntryTerminalReceipt,
    checked_boundary_operator_scope:
        lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    boundary_application_coverage: boundary_applications::TerminalBoundaryApplicationCoverage,
    ieee_float_fma_occurrences: Vec<compilation_report::TerminalIeeeFloatFmaOccurrenceProposal>,
}

impl ProgramEntryTerminalArtifact {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    /// Every selected nearest fused multiply-add the artifact executes,
    /// rejoined to its exact selected plan (and x86 deployment admission)
    /// before the direct native route realizes the operation.
    pub fn ieee_float_fma_occurrences(
        &self,
    ) -> &[compilation_report::TerminalIeeeFloatFmaOccurrenceProposal] {
        &self.ieee_float_fma_occurrences
    }

    /// The artifact, its checked program-entry receipt, the checked
    /// boundary-operator scope, and the boundary application coverage.
    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        terminal_psi::CheckedProgramEntryTerminalReceipt,
        lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
        boundary_applications::TerminalBoundaryApplicationCoverage,
    ) {
        (
            self.artifact,
            self.checked_program_entry,
            self.checked_boundary_operator_scope,
            self.boundary_application_coverage,
        )
    }
}

/// Produce the Terminal artifact for one exact selected program entry: the
/// direct native route's input. The artifact carries the checked entry
/// receipt, the checked boundary-operator scope and the application coverage
/// that native realization rejoins; it does not enter realization.
pub fn produce_program_entry_terminal_artifact(
    checked: &CheckedCompilation,
    program_entry: &build_evaluation::SelectedCompilerProgramEntry,
    optimization_selections: &optimization_core::OptimizationSelections,
) -> Result<ProgramEntryTerminalArtifact, Vec<Diagnostic>> {
    // Same admission requirement as the retained product: the authored
    // exclusions are verified against the unoptimized composition before the
    // direct native route's artifact is produced.
    behavior_exclusions::verify_entry_behavior_exclusions(
        checked,
        program_entry.source_signature().machine_symbol(),
    )?;
    let psi_optimizations = optimization_selections.project_psi();
    let terminal_trees = checked.terminal_production_trees();
    let produced = terminal_production::TerminalProductionRequest {
        checked: terminal_trees,
        machine: terminal_production::TerminalMachineSelection::Symbol(
            program_entry.source_signature().machine_symbol(),
        ),
        optimization_selections: psi_optimizations.selections().clone(),
    }
    .produce_program_entry(program_entry.source_signature().identity().bytes())
    .map_err(|error| {
        vec![Diagnostic::error(format!(
            "native-artifact Terminal production failed: {error}"
        ))]
    })?;
    let (
        artifact,
        checked_program_entry,
        checked_boundary_operator_scope,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        vec![Diagnostic::error(format!(
            "native comparison custody could not decode Terminal semantics: {error}"
        ))]
    })?;
    // The direct entrance must perform the same source/provider join as retained
    // Terminal re-entry. Portable numeric semantics alone do not select a provider.
    // Realization below retains the exact checked application coverage; ordinary
    // graph lowering carries each surviving operation into its physical child.
    float_comparisons::associate(
        checked,
        &module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        &selected_ieee_float_comparison_occurrences,
    )?;
    integer_comparisons::associate(
        checked,
        &module,
        checked.selected_provider_plans(),
        checked.selected_provider_provenance(),
        &selected_integer_comparison_occurrences,
    )?;
    // The direct route carries the same nearest-FMA custody the retained
    // product's proposal carries: each Terminal occurrence rejoins exactly one
    // selected plan that binds its requirement, so realizing the operation
    // never drops the provider the checked program selected.
    let native_target = checked.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "native-artifact production requires one selected native target",
        )]
    })?;
    let ieee_float_fma_occurrences =
        float_fma::associate(checked, native_target, &selected_ieee_float_fma_occurrences)?;
    let boundary_application_coverage =
        application_coverage::project_terminal_boundary_application_coverage(
            checked,
            &artifact,
            &checked_boundary_operator_scope,
        )?;
    Ok(ProgramEntryTerminalArtifact {
        artifact,
        checked_program_entry,
        checked_boundary_operator_scope,
        boundary_application_coverage,
        ieee_float_fma_occurrences,
    })
}
