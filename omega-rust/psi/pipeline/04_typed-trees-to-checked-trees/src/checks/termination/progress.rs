//!
//! This file analyzes checked progress. `qualification_correspondences.rs`
//! validates qualification correspondences and replays their types,
//! `machine_summaries.rs` derives machine and selected call summaries,
//! `fact_subjects.rs` names fact domains and subjects and `tests.rs`
//! holds the progress tests; the remaining files carry the lineage and
//! receipt vocabulary.

mod components;
mod fact_subjects;
mod lineage;
mod machine_summaries;
mod origins;
mod qualification_correspondences;
#[cfg(test)]
mod tests;

use checked_trees::{BuildBoundProgressDemand, FlowCallFact, FlowFacts, FlowStateFact};
use diagnostics::Diagnostic;
use language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use symbols::SymbolHandle;

use crate::checks::termination::progress::fact_subjects::{profile_label, subject_label};
use crate::checks::termination::progress::qualification_correspondences::validate_qualification_correspondences;

/// Derive checked termination premises from exact selected call contracts.
///
/// Published operation contracts remain the caller-facing authority. Private
/// checked callees contribute their derived summary through a fixed point, so
/// mentioning a qualified value does nothing while actually invoking a
/// premise-bearing operation instantiates exactly that premise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckedProgressSummary {
    pub(crate) machine: SymbolHandle,
    pub(crate) guarantee: TerminationGuarantee,
    pub(crate) build_bound_demands: Vec<BuildBoundProgressDemand>,
}

#[cfg(test)]
pub(crate) fn analyze_checked_progress(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
) -> Result<Vec<CheckedProgressSummary>, Vec<Diagnostic>> {
    analyze_checked_progress_with_call_frames(program, flow, semantic, None)
}

pub(crate) fn analyze_checked_progress_with_call_frames(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<Vec<CheckedProgressSummary>, Vec<Diagnostic>> {
    let correspondence_diagnostics = validate_qualification_correspondences(program, semantic);
    if !correspondence_diagnostics.is_empty() {
        return Err(correspondence_diagnostics);
    }
    let summaries = components::derive_summaries(program, flow, semantic, call_frames);

    let mut diagnostics = Vec::new();
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    {
        // Ranking diagnostics own local control-flow failure and can name the
        // missing or invalid witness precisely. Progress coverage begins only
        // after that independent obligation succeeds.
        if !super::infer_machine_checked_summary_with_call_frames(program, machine, call_frames)
            .promises_termination()
        {
            continue;
        }
        let Some(checked) = summaries
            .iter()
            .find(|summary| summary.machine == machine.symbol)
        else {
            continue;
        };
        let language_semantics::TerminationInterface::Published(published) =
            &machine.termination_plan.interface
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: published_premises,
        } = published
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: checked_premises,
        } = &checked.guarantee
        else {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove published termination for machine `{}`: its checked body reaches an operation without a usable termination guarantee",
                machine.name
            )));
            continue;
        };
        for premise in checked_premises {
            if !published_premises.contains(premise) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` derives progress premise `{}` for `{}` from a selected call, but its published termination contract does not cover that exact subject",
                    machine.name,
                    profile_label(program, premise.profile),
                    subject_label(program, &premise.subject),
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(summaries)
    } else {
        Err(diagnostics)
    }
}
