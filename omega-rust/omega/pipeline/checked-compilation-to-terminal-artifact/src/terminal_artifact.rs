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
use artifacts::allocations::AllocationDelta;
use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use assembled_syntax_to_checked_compilation::{CheckedCompilation, OptimizationRollback};
use diagnostics::Diagnostic;
use terminal_production::{
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
pub fn produce_terminal_report(
    root_path: std::path::PathBuf,
    mut checked: CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    rollback: &OptimizationRollback,
) -> Result<compilation_report::CompileReport, Vec<Diagnostic>> {
    let pcc_requests = checked.pcc_requests();
    let production_subject = checked.production_subject()?;
    let source_file_count = checked.source_file_count();
    let rollback = rollback.settle(checked.optimization_selections());
    let artifact = produce_retained_terminal_artifact(&mut checked, profile, rollback.effective())?;
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
    checked: &mut CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    selections: &optimization_core::OptimizationSelections,
) -> Result<compilation_report::RetainedTerminalArtifact, Vec<Diagnostic>> {
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
    selected_dispatch::validate_selected_operator_terminal_custody(
        checked,
        checked.selected_provider_plans(),
    )?;
    selected_dispatch::validate_fused_service_terminal_custody(
        checked,
        checked.selected_provider_provenance(),
    )?;
    let psi_optimizations = selections.project_psi();
    // The accumulator steps out of the checked record across the measured
    // legs below so each closure may borrow the record while it is timed; it
    // rejoins the record once the product is produced and admitted.
    let mut stage_timings = std::mem::take(checked.timings_mut());
    let mut production_timings = TerminalProductionTimings::enabled();
    let produced = stage_timings
        .record_result(TERMINAL_PRODUCTION_STAGE, || {
            terminal_production::TerminalProductionRequest {
                checked: checked.terminal_production_trees(),
                machine: terminal_production::TerminalMachineSelection::Symbol(
                    entry_machine_symbol,
                ),
                optimization_selections: psi_optimizations.selections().clone(),
            }
            .produce(TerminalProductionCustody {
                entry_identity: Some(source_signature_identity),
                callback_custody: callback_placements,
                // The authored behavior exclusions are judged against the
                // unoptimized composition the artifact was published from.
                retain_unoptimized: true,
                timings: &mut production_timings,
            })
        })
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact production failed: {}",
                error.error(),
            ))]
        })?;
    merge_terminal_production_timings(&mut stage_timings, &production_timings);
    let (
        artifact,
        checked_program_entry,
        unoptimized,
        checked_boundary_operator_scope,
        callback_placements,
        source_call_occurrences,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    let checked_program_entry = checked_program_entry.ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production retained no checked ProgramEntry receipt",
        )]
    })?;
    // The root and provider selections are fixed; the authored behavior
    // exclusions must hold in the unoptimized composition before the product
    // is admitted.
    behavior_exclusions::verify_entry_behavior_exclusions(
        checked,
        unoptimized.as_ref(),
        entry_machine_symbol,
    )?;
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
                &selected_ieee_float_fma_occurrences,
                &selected_ieee_float_comparison_occurrences,
                &selected_integer_comparison_occurrences,
                selections,
            )
        });
    *checked.timings_mut() = stage_timings;
    let native_realization_proposal = native_realization_proposal?;
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
    // Same admission requirement as the retained product: a settled
    // `Independent` edge has no product carrier, and the authored exclusions
    // are verified against the unoptimized composition the direct native
    // route's artifact is published from.
    composition_modes::verify_selected_compositions_are_realized(checked)?;
    let psi_optimizations = optimization_selections.project_psi();
    let terminal_trees = checked.terminal_production_trees();
    let mut stage_timings = checked.timings().clone();
    let mut production_timings = TerminalProductionTimings::enabled();
    let produced = stage_timings
        .record_result(TERMINAL_PRODUCTION_STAGE, || {
            terminal_production::TerminalProductionRequest {
                checked: terminal_trees,
                machine: terminal_production::TerminalMachineSelection::Symbol(
                    program_entry.source_signature().machine_symbol(),
                ),
                optimization_selections: psi_optimizations.selections().clone(),
            }
            .produce(TerminalProductionCustody {
                entry_identity: Some(program_entry.source_signature().identity().bytes()),
                callback_custody: (),
                retain_unoptimized: true,
                timings: &mut production_timings,
            })
        })
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "native-artifact Terminal production failed: {error}"
            ))]
        })?;
    merge_terminal_production_timings(&mut stage_timings, &production_timings);
    let (
        artifact,
        checked_program_entry,
        unoptimized,
        checked_boundary_operator_scope,
        (),
        _source_call_occurrences,
        selected_ieee_float_fma_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    let checked_program_entry = checked_program_entry.ok_or_else(|| {
        vec![Diagnostic::error(
            "native-artifact Terminal production retained no checked ProgramEntry receipt",
        )]
    })?;
    behavior_exclusions::verify_entry_behavior_exclusions(
        checked,
        unoptimized.as_ref(),
        program_entry.source_signature().machine_symbol(),
    )?;
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
        stage_timings,
    })
}

#[cfg(test)]
mod tests {
    use super::merge_terminal_production_timings;
    use artifacts::compile_timings::CompileTimings;
    use terminal_production::{TerminalProductionStage, TerminalProductionTimings};

    #[test]
    fn production_stage_rows_merge_under_the_boundary_label() {
        let mut stage_timings = CompileTimings::enabled();
        let mut production_timings = TerminalProductionTimings::enabled();
        production_timings
            .record_result(TerminalProductionStage::Lowering, || Ok::<_, ()>(()))
            .unwrap();
        production_timings
            .record_result(TerminalProductionStage::Publication, || Ok::<_, ()>(()))
            .unwrap();

        merge_terminal_production_timings(&mut stage_timings, &production_timings);

        let phases: Vec<&str> = stage_timings
            .phases()
            .iter()
            .map(|timing| timing.phase.as_str())
            .collect();
        assert_eq!(
            phases,
            [
                "terminal-production/lowering: CheckedTrees -> LoweredPsi",
                "terminal-production/publication: OptimizedLoweredPsi -> CanonicalTerminalArtifact",
            ]
        );
    }

    #[test]
    fn disabled_accumulator_drops_production_rows() {
        let mut stage_timings = CompileTimings::default();
        let mut production_timings = TerminalProductionTimings::enabled();
        production_timings
            .record_result(TerminalProductionStage::Lowering, || Ok::<_, ()>(()))
            .unwrap();

        merge_terminal_production_timings(&mut stage_timings, &production_timings);

        assert!(stage_timings.phases().is_empty());
    }
}
