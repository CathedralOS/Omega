//! Checked compilation to the canonical Terminal artifact.
//!
//! One product leaves here: retained Terminal with callback custody and a
//! native-realization proposal. Both Terminal publication and immediate native
//! production consume this same boundary; no native route reopens checked source.

use crate::checked::{CheckedCompilation, OptimizationRollback};
use crate::terminal::native_proposal;
use artifacts::allocations::AllocationDelta;
use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use diagnostics::Diagnostic;
use terminal_production::{
    TerminalProductionCustody, TerminalProductionStage, TerminalProductionTimings,
};

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

pub fn produce_terminal_report(
    root_path: std::path::PathBuf,
    mut checked: CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    rollback: &OptimizationRollback,
) -> Result<crate::report::CompileReport, Vec<Diagnostic>> {
    let pcc_requests = checked.pcc_requests();
    let production_subject = checked.production_subject()?;
    let source_file_count = checked.source_file_count();
    let rollback = rollback.settle(checked.optimization_selections());
    let artifact = produce_retained_terminal_artifact(&mut checked, profile, rollback.effective())?;
    crate::report::CompileReport::from_retained_terminal_artifact(
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
    terminal_psi_to_abstract_operations::settle_ieee_float_comparison_occurrences(
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
    terminal_psi_to_abstract_operations::settle_integer_comparison_occurrences(
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
pub(crate) fn produce_retained_terminal_artifact(
    checked: &mut CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    selections: &optimization_core::OptimizationSelections,
) -> Result<crate::report::RetainedTerminalArtifact, Vec<Diagnostic>> {
    // A settled `Independent` edge has no product carrier: the composition
    // fence rejects before any fused artifact can be produced or admitted.
    provider_planning::verify_selected_compositions_are_realized(
        checked.selected_provider_provenance(),
    )?;
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
    selected_dispatch::validate_selected_operator_terminal_custody(checked)?;
    selected_dispatch::validate_fused_service_terminal_custody(
        checked,
        checked.selected_provider_provenance(),
    )?;
    // The accumulator steps out of the checked record across the measured
    // legs below so each closure may borrow the record while it is timed; it
    // rejoins the record once the product is produced and admitted.
    let mut stage_timings = std::mem::take(checked.timings_mut());
    let psi_optimizations = selections.project_psi();
    let mut production_timings = TerminalProductionTimings::enabled_if(stage_timings.is_enabled());
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
                callback_custody: (),
                // The authored behavior exclusions are judged against the
                // unoptimized composition the artifact was published from.
                retain_unoptimized: !checked.behavior_exclusions().is_empty(),
                timings: &mut production_timings,
            })
        })
        .map_err(|error| {
            vec![Diagnostic::error(
                match error.error().unimplemented_report() {
                    Some(report) => report.to_owned(),
                    // A named proof-search capacity that declined during
                    // optional inference is an `Incomplete` outcome, not a
                    // defect of the program: name the resource and the
                    // obligation left unproved instead of reporting the
                    // module as rejected. The `incomplete:` prefix is the
                    // outcome marker the corpus records against its
                    // `incomplete` status.
                    None => match error.error().incomplete_capacity() {
                        Some((capacity, obligation)) => format!(
                            "incomplete: terminal-artifact production stopped at the private \
                             `{}` proof-search capacity (provision {}) with obligation {} \
                             unproved; the module is not shown invalid",
                            capacity.resource_name(),
                            capacity.limit(),
                            obligation,
                        ),
                        None => format!("terminal-artifact production failed: {error}"),
                    },
                },
            )]
        })?;
    merge_terminal_production_timings(&mut stage_timings, &production_timings);
    let checked_program_entry = produced.receipt().cloned().ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production retained no checked ProgramEntry receipt",
        )]
    })?;
    trust_model::verify_entry_behavior_exclusions(
        checked,
        checked.behavior_exclusions(),
        checked.selected_provider_plans(),
        produced.unoptimized(),
        entry_machine_symbol,
    )?;
    let (
        artifact,
        _,
        _,
        checked_boundary_operator_scope,
        _,
        source_call_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    stage_timings.record_result(TERMINAL_VERIFICATION_STAGE, || {
        terminal_codec::verify_terminal_artifact_proof(&artifact, profile)
            .map(|_| ())
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "terminal-artifact verification failed for {}: {}",
                    error.subject, error.reason,
                ))]
            })
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
    crate::report::RetainedTerminalArtifact::new_with_native_realization_proposal(
        artifact,
        callback_placements,
        native_realization_proposal,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}
