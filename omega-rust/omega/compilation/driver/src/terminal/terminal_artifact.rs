//! Checked compilation to the canonical Terminal artifact.
//!
//! One product leaves here: retained Terminal with callback custody and a
//! native-realization proposal. Both Terminal publication and immediate native
//! production consume this same boundary; no native route reopens checked source.

use crate::checked::{CheckedCompilation, OptimizationRollback};
use crate::terminal::native_proposal;
use artifacts::allocations::AllocationDelta;
use artifacts::compile_timings::{StageMeta, TimingCategory};
use diagnostics::Diagnostic;
use terminal_production::TerminalProductionCustody;

/// Terminal production internally lowers, optimizes and publishes
/// (checked-trees -> lowered-psi -> lowered-psi -> terminal-psi). The request
/// keeps this coarse row as the boundary total; the production sequence
/// records its per-leg rows into the same ladder, below it.
/// Psi's semantic stages have no reporting dependency.
const TERMINAL_PRODUCTION_STAGE: StageMeta = StageMeta::new(
    "terminal-production",
    "CheckedTrees",
    "CanonicalTerminalArtifact",
    TimingCategory::Pipeline,
);

const NATIVE_REALIZATION_PROPOSAL_STAGE: StageMeta = StageMeta::new(
    "native-realization-proposal",
    "CanonicalTerminalArtifact",
    "NativeRealizationProposal",
    TimingCategory::Pipeline,
);

/// What one Terminal stage sequence produces for the admitted settlement to
/// turn into a report: the retained product, the settled rollback receipt, the
/// post-production timing ladder and the producer-side rewrite explanation.
/// None of it is a report, an admission or a publication.
pub struct ProducedTerminalProduct {
    pub artifact: compilation_products::RetainedTerminalArtifact,
    pub rollback_receipt: Option<optimization_core::OptimizationRollbackReceipt>,
    pub timings: Vec<artifacts::compile_timings::PhaseTiming>,
    pub withheld_rewrites: Vec<optimization_core::WithheldRewrite>,
}

/// Run the Terminal stage sequence the product admission already allowed. The
/// stop, its Build requirements and its package production custody were
/// settled by `CheckedCompilation::prepare_product_report`; this owner never
/// re-admits them.
pub fn produce_terminal_product(
    mut checked: CheckedCompilation,
    rollback: &OptimizationRollback,
) -> Result<ProducedTerminalProduct, Vec<Diagnostic>> {
    let rollback = rollback.settle(&checked.build().config.settings.optimizations);
    let produced = produce_retained_terminal_artifact(&mut checked, rollback.effective())?;
    Ok(ProducedTerminalProduct {
        artifact: produced.artifact,
        rollback_receipt: rollback.into_receipt(),
        timings: checked.timings().phases().to_vec(),
        withheld_rewrites: produced.withheld_rewrites,
    })
}

/// Produce one retained Terminal product from the complete checked frontend
/// result.
///
/// This owner closes the checked-to-Terminal boundary and binds the
/// target-owned native-realization proposal. It does not re-verify the
/// artifact it just produced: under verification placement
/// (`omega-rust/pipeline.md`) the producer is the gate, and the corpus runner
/// and tests exercise the verifier. It does not assemble a compiler report or
/// enter native realization.
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
    // Production records its own legs into this same ladder, so the aggregate
    // row cannot be taken with `record_result`: that holds the accumulator
    // borrowed across the call. Measuring around it and adding the row
    // afterwards is what `record_result` does internally, rejection included,
    // and `add_completed` is a no-op on a disabled ladder.
    //
    // The row is opened at zero first so it keeps the position it held when a
    // separate carrier was merged in afterwards: the ladder orders phases by
    // first insertion, and the legs below would otherwise land ahead of their
    // own total. `add_completed` sums onto an existing label, so the real
    // elapsed is added to this row rather than replacing it.
    stage_timings.add_completed(TERMINAL_PRODUCTION_STAGE, 0, AllocationDelta::default());
    let production_started = std::time::Instant::now();
    let produced = terminal_production::TerminalProductionRequest {
        checked: &checked.execution().program,
        machine: terminal_production::TerminalMachineSelection::Symbol(entry_machine_symbol),
        optimization_selections: psi_optimizations.selections().clone(),
    }
    .produce(TerminalProductionCustody {
        entry_identity: Some(source_signature_identity),
        // The authored behavior exclusions are judged against the
        // unoptimized composition the artifact was published from.
        retain_unoptimized: !checked
            .build()
            .config
            .behavior_exclusions
            .as_slice()
            .is_empty(),
        timings: &mut stage_timings,
    });
    stage_timings.add_completed(
        TERMINAL_PRODUCTION_STAGE,
        production_started.elapsed().as_micros(),
        AllocationDelta::default(),
    );
    // A capacity refusal keeps its exceeded provision on the diagnostic so
    // the target's outcome publishes `Incomplete`, not a rejection.
    let produced = produced.map_err(|error| vec![error.diagnostic()])?;
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
        source_call_occurrences,
        selected_ieee_float_comparison_occurrences,
        selected_integer_comparison_occurrences,
    ) = produced.into_parts();
    let native_realization_proposal =
        stage_timings.record_result(NATIVE_REALIZATION_PROPOSAL_STAGE, || {
            native_proposal::project_terminal_native_realization_proposal(
                checked,
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
        );
    Ok(ProducedRetainedTerminal {
        artifact,
        withheld_rewrites,
    })
}
