//! Checked compilation to the canonical Terminal artifact.
//!
//! Two products leave here. The retained Terminal product carries its
//! callback custody and a native-realization proposal, and returns as the
//! compiler report; the program-entry artifact is what the direct native
//! route realizes. Both replay the canonical semantic and proof sections
//! under the request admission profile before anything downstream sees them.

use crate::artifacts::allocations::AllocationDelta;
use crate::artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use crate::compiler::checked::{CheckedCompilation, OptimizationRollback};
use crate::compiler::terminal::{
    application_coverage, float_comparisons, integer_comparisons, native_proposal,
};
use diagnostics::Diagnostic;
use lowered_psi_to_terminal_psi::terminal_production::{
    TerminalProductionCustody, TerminalProductionStage, TerminalProductionTimings,
};

pub(crate) mod behavior_exclusions;
pub(crate) mod composition_modes;
pub(crate) mod verification;

/// Terminal production internally lowers, optimizes and publishes
/// (checked-trees -> lowered-psi -> lowered-psi -> terminal-psi). The request
/// keeps this coarse row as the boundary total; the Psi-side crate cannot
/// name the shared tooling accumulator, so its per-leg rows arrive through
/// `TerminalProductionTimings` and merge below it.
const TERMINAL_PRODUCTION_STAGE: StageMeta = StageMeta::new(
    "terminal-production",
    "CheckedTrees",
    "CanonicalTerminalArtifact",
    TimingCategory::Pipeline,
);

const TERMINAL_VERIFICATION_STAGE: StageMeta = StageMeta::new(
    "terminal-verification",
    "CanonicalTerminalArtifact",
    "VerifiedTerminalArtifact",
    TimingCategory::Pipeline,
);

const NATIVE_REALIZATION_PROPOSAL_STAGE: StageMeta = StageMeta::new(
    "native-realization-proposal",
    "VerifiedTerminalArtifact",
    "NativeRealizationProposal",
    TimingCategory::Pipeline,
);

/// The Omega-side ladder names each Psi production leg rows arrive under.
fn terminal_production_stage_meta(stage: TerminalProductionStage) -> StageMeta {
    let (name, input, output) = match stage {
        TerminalProductionStage::MachineSelection => (
            "terminal-production/machine-selection",
            "CheckedTrees",
            "TerminalMachine",
        ),
        TerminalProductionStage::LedgerCheck => (
            "terminal-production/ledger-check",
            "CheckedTrees",
            "CheckedLedger",
        ),
        TerminalProductionStage::Lowering => {
            ("terminal-production/lowering", "CheckedTrees", "LoweredPsi")
        }
        TerminalProductionStage::Optimization => (
            "terminal-production/optimization",
            "LoweredPsi",
            "OptimizedLoweredPsi",
        ),
        TerminalProductionStage::EntryReceipt => (
            "terminal-production/entry-receipt",
            "OptimizedLoweredPsi",
            "ProgramEntryReceipt",
        ),
        TerminalProductionStage::TerminalIdentity => (
            "terminal-production/terminal-identity",
            "SemanticModule",
            "TerminalPsiIdentity",
        ),
        TerminalProductionStage::ReceiverEligibility => (
            "terminal-production/receiver-eligibility",
            "CheckedTrees+SemanticModule",
            "ReceiverEligibility",
        ),
        TerminalProductionStage::Publication => (
            "terminal-production/publication",
            "OptimizedLoweredPsi",
            "CanonicalTerminalArtifact",
        ),
        TerminalProductionStage::BoundaryOperatorScope => (
            "terminal-production/boundary-scope",
            "LoweredPsi+CanonicalTerminalArtifact",
            "BoundaryOperatorScope",
        ),
    };
    StageMeta::new(name, input, output, TimingCategory::Pipeline)
}

/// The production-row collector inherits the request's own collection state:
/// a disabled `CompileTimings` produces a disabled collector, so untimed
/// production runs the same legs without inner clock reads or retained rows.
/// Both production paths build their collector through this one seam.
fn production_timings_for(stage_timings: &CompileTimings) -> TerminalProductionTimings {
    TerminalProductionTimings::enabled_if(stage_timings.is_enabled())
}

/// Merge the Psi-owned production rows under the boundary row the caller just
/// recorded. Rows drop silently when the accumulator is disabled.
fn merge_terminal_production_timings(
    stage_timings: &mut CompileTimings,
    production_timings: &TerminalProductionTimings,
) {
    for (stage, microseconds) in production_timings.rows() {
        stage_timings.add_completed(
            terminal_production_stage_meta(*stage),
            *microseconds,
            AllocationDelta::default(),
        );
    }
}

/// Produce the retained Terminal product and its ordinary compiler report.
/// The admitted entry product both routes leave through. Terminal production
/// is keyed on the selected entry's exact machine symbol and retains the
/// unoptimized module beside the artifact; production must hand back the
/// checked entry receipt, and the authored behavior exclusions are judged on
/// that unoptimized composition before either route continues. The routes
/// differ only in the callback custody they carry through production, the
/// product label their diagnostics wear, and what they do with the admitted
/// product afterwards.
fn produce_admitted_entry_artifact<C>(
    checked: &CheckedCompilation,
    stage_timings: &mut CompileTimings,
    entry_machine_symbol: symbols::SymbolHandle,
    source_signature_identity: [u8; 32],
    selections: &optimization_core::OptimizationSelections,
    callback_custody: C,
    product_label: &str,
) -> Result<
    (
        lowered_psi_to_terminal_psi::terminal_production::ProducedTerminalArtifact<C>,
        terminal_psi::CheckedProgramEntryTerminalReceipt,
    ),
    Vec<Diagnostic>,
> {
    let psi_optimizations = selections.project_psi();
    let mut production_timings = production_timings_for(stage_timings);
    let produced = stage_timings
        .record_result(TERMINAL_PRODUCTION_STAGE, || {
            lowered_psi_to_terminal_psi::terminal_production::TerminalProductionRequest {
                checked: checked.terminal_production_trees(),
                machine: lowered_psi_to_terminal_psi::terminal_production::TerminalMachineSelection::Symbol(
                    entry_machine_symbol,
                ),
                optimization_selections: psi_optimizations.selections().clone(),
            }
            .produce(TerminalProductionCustody {
                entry_identity: Some(source_signature_identity),
                callback_custody,
                // The authored behavior exclusions are judged against the
                // unoptimized composition the artifact was published from.
                retain_unoptimized: true,
                timings: &mut production_timings,
            })
        })
        .map_err(|error| {
            vec![Diagnostic::error(
                match error.error().unimplemented_report() {
                    Some(report) => report.to_owned(),
                    None => format!("{product_label} production failed: {error}"),
                },
            )]
        })?;
    merge_terminal_production_timings(stage_timings, &production_timings);
    let checked_program_entry = produced.receipt().cloned().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "{product_label} production retained no checked ProgramEntry receipt"
        ))]
    })?;
    behavior_exclusions::verify_entry_behavior_exclusions(
        checked,
        produced.unoptimized(),
        entry_machine_symbol,
    )?;
    Ok((produced, checked_program_entry))
}

pub fn produce_terminal_report(
    root_path: std::path::PathBuf,
    mut checked: CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    rollback: &OptimizationRollback,
) -> Result<crate::compiler::report::CompileReport, Vec<Diagnostic>> {
    let pcc_requests = checked.pcc_requests();
    let production_subject = checked.production_subject()?;
    let source_file_count = checked.source_file_count();
    let rollback = rollback.settle(checked.optimization_selections());
    let artifact = produce_retained_terminal_artifact(&mut checked, profile, rollback.effective())?;
    crate::compiler::report::CompileReport::from_retained_terminal_artifact(
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
    lowered: &checked_trees_to_lowered_psi::lowered_psi::LoweredPsi,
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
    lowered: &checked_trees_to_lowered_psi::lowered_psi::LoweredPsi,
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
    checked: &mut CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    selections: &optimization_core::OptimizationSelections,
) -> Result<crate::compiler::report::RetainedTerminalArtifact, Vec<Diagnostic>> {
    // A settled `Independent` edge has no product carrier: the composition
    // fence rejects before any fused artifact can be produced or admitted.
    composition_modes::verify_selected_compositions_are_realized(checked)?;
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
    crate::selected_dispatch::validate_selected_operator_terminal_custody(checked)?;
    crate::selected_dispatch::validate_fused_service_terminal_custody(
        checked,
        checked.selected_provider_provenance(),
    )?;
    // The accumulator steps out of the checked record across the measured
    // legs below so each closure may borrow the record while it is timed; it
    // rejoins the record once the product is produced and admitted.
    let mut stage_timings = std::mem::take(checked.timings_mut());
    let (produced, checked_program_entry) = produce_admitted_entry_artifact(
        checked,
        &mut stage_timings,
        entry_machine_symbol,
        source_signature_identity,
        selections,
        callback_placements,
        "terminal-artifact",
    )?;
    let (
        artifact,
        _,
        _,
        checked_boundary_operator_scope,
        callback_placements,
        source_call_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    stage_timings.record_result(TERMINAL_VERIFICATION_STAGE, || {
        verification::verify_terminal_artifact(&artifact, profile)
    })?;
    let native_realization_proposal =
        stage_timings.record_result(NATIVE_REALIZATION_PROPOSAL_STAGE, || {
            native_proposal::project_terminal_native_realization_proposal(
                checked,
                profile,
                &artifact,
                checked_program_entry,
                checked_boundary_operator_scope,
                &callback_placements,
                &source_call_occurrences,
                &selected_ieee_float_comparison_occurrences,
                &selected_integer_comparison_occurrences,
                selections,
            )
        });
    *checked.timings_mut() = stage_timings;
    let native_realization_proposal = native_realization_proposal?;
    crate::compiler::report::RetainedTerminalArtifact::new_with_native_realization_proposal(
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
    boundary_application_coverage: resolved_layout_to_resolved_layout::boundary_applications::TerminalBoundaryApplicationCoverage,
    /// The stage measurements this artifact's production recorded; the caller
    /// owns merging them back into the checked accumulator it cloned from.
    stage_timings: CompileTimings,
}

impl ProgramEntryTerminalArtifact {
    pub const fn artifact(&self) -> &terminal_codec::CanonicalTerminalArtifact {
        &self.artifact
    }

    /// The stage measurements recorded during this artifact's production.
    pub const fn stage_timings(&self) -> &CompileTimings {
        &self.stage_timings
    }

    /// The artifact, its checked program-entry receipt, the checked
    /// boundary-operator scope, and the boundary application coverage.
    pub fn into_parts(
        self,
    ) -> (
        terminal_codec::CanonicalTerminalArtifact,
        terminal_psi::CheckedProgramEntryTerminalReceipt,
        lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
        resolved_layout_to_resolved_layout::boundary_applications::TerminalBoundaryApplicationCoverage,
    ){
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
    program_entry: &crate::build_evaluation::SelectedCompilerProgramEntry,
    optimization_selections: &optimization_core::OptimizationSelections,
) -> Result<ProgramEntryTerminalArtifact, Vec<Diagnostic>> {
    // Same admission requirement as the retained product: a settled
    // `Independent` edge has no product carrier, and the authored exclusions
    // are verified against the unoptimized composition the direct native
    // route's artifact is published from.
    composition_modes::verify_selected_compositions_are_realized(checked)?;
    let mut stage_timings = checked.timings().clone();
    let (produced, checked_program_entry) = produce_admitted_entry_artifact(
        checked,
        &mut stage_timings,
        program_entry.source_signature().machine_symbol(),
        program_entry.source_signature().identity().bytes(),
        optimization_selections,
        (),
        "native-artifact Terminal",
    )?;
    let (
        artifact,
        _,
        _,
        checked_boundary_operator_scope,
        (),
        _source_call_occurrences,
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
        stage_timings,
    })
}
