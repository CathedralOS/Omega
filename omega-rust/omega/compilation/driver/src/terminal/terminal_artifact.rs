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
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

/// Terminal production internally lowers, optimizes and publishes
/// (checked-trees -> lowered-psi -> lowered-psi -> terminal-psi). The request
/// keeps this coarse row as the boundary total; the production sequence's
/// per-leg rows arrive through `TerminalProductionTimings` and merge below it.
/// Psi's semantic stages have no reporting dependency.
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

/// Merge the production sequence's rows under the boundary row the caller just
/// recorded. Rows drop silently when the accumulator is disabled.
fn merge_terminal_production_timings(
    stage_timings: &mut CompileTimings,
    production_timings: &TerminalProductionTimings,
) {
    for (stage, microseconds) in production_timings.rows() {
        let (name, input, output) = stage.observation_labels();
        stage_timings.add_completed(
            StageMeta::new(name, input, output, TimingCategory::Pipeline),
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
) -> Result<compilation_products::CompileReport, Vec<Diagnostic>> {
    let pcc_requests = checked.build().config.settings.pcc;
    compilation_products::RequestedCompileProduct::TerminalArtifact.admit_build_requirements(
        checked.build_admission().artifact_only(),
        pcc_requests,
        rollback,
    )?;
    let production_subject = checked.production_subject()?;
    let rollback = rollback.settle(&checked.build().config.settings.optimizations);
    let produced = produce_retained_terminal_artifact(&mut checked, profile, rollback.effective())?;
    checked
        .terminal_report(
            root_path,
            produced.artifact,
            production_subject,
            rollback.into_receipt(),
            profile.clone(),
        )
        .map(|report| report.with_withheld_rewrites(produced.withheld_rewrites))
}

/// Produce one verified retained Terminal product from the complete checked
/// frontend result.
///
/// This owner closes the checked-to-Terminal boundary, replays the canonical
/// semantic and proof sections under the request admission profile, and binds
/// the target-owned native-realization proposal. It does not assemble a
/// compiler report or enter native realization.
/// The retained product and the pass decisions that explain it.
///
/// The decisions are producer-side explanation, so they ride beside the
/// canonical artifact rather than on it: the artifact is the same bytes
/// whether a pass withheld a rewrite or found none.
pub(crate) struct ProducedRetainedTerminal {
    pub(crate) artifact: compilation_products::RetainedTerminalArtifact,
    pub(crate) withheld_rewrites: Vec<optimization_core::WithheldRewrite>,
}

pub(crate) fn produce_retained_terminal_artifact(
    checked: &mut CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    selections: &optimization_core::OptimizationSelections,
) -> Result<ProducedRetainedTerminal, Vec<Diagnostic>> {
    let selected_program_entry = checked.admit_terminal_production()?;
    let entry_machine_symbol = selected_program_entry.source_signature().machine_symbol();
    let source_signature_identity = selected_program_entry.source_signature().identity().bytes();
    let callback_placements = checked.execution().callback_placements.as_slice().to_vec();
    // The accumulator steps out of the checked record across the measured
    // legs below so each closure may borrow the record while it is timed; it
    // rejoins the record once the product is produced and admitted.
    let mut stage_timings = std::mem::take(checked.timings_mut());
    let psi_optimizations = selections.project_psi();
    let mut production_timings = TerminalProductionTimings::enabled_if(stage_timings.is_enabled());
    let produced = stage_timings
        .record_result(TERMINAL_PRODUCTION_STAGE, || {
            terminal_production::TerminalProductionRequest {
                checked: &checked.execution().program,
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
                retain_unoptimized: !checked
                    .build()
                    .config
                    .behavior_exclusions
                    .as_slice()
                    .is_empty(),
                timings: &mut production_timings,
            })
        })
        // A capacity refusal keeps its exceeded provision on the diagnostic so
        // the target's outcome publishes `Incomplete`, not a rejection.
        .map_err(|error| vec![error.error().diagnostic()])?;
    merge_terminal_production_timings(&mut stage_timings, &production_timings);
    let checked_program_entry = produced.receipt().cloned().ok_or_else(|| {
        vec![Diagnostic::error(
            "terminal-artifact production retained no checked ProgramEntry receipt",
        )]
    })?;
    trust_model::verify_entry_behavior_exclusions(
        checked,
        checked.build().config.behavior_exclusions.as_slice(),
        &checked.execution().selected_provider_plan_facts,
        produced.unoptimized(),
        entry_machine_symbol,
    )?;
    // Read before `into_parts` consumes the production result.
    let withheld_rewrites = produced.withheld_rewrites().to_vec();
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
    let artifact =
        compilation_products::RetainedTerminalArtifact::new_with_native_realization_proposal(
            artifact,
            callback_placements,
            native_realization_proposal,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
    Ok(ProducedRetainedTerminal {
        artifact,
        withheld_rewrites,
    })
}
