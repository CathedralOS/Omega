//! A structural bound on finite premise transport, not an iteration budget.

use super::super::CheckedProgressSummary;
use crate::checks::termination::progress::fact_subjects::fact_subject;
use crate::checks::termination::progress::machine_summaries::{
    call_argument_subject, selected_call_summary,
};
use checked_trees::FlowFacts;
use language_semantics::TerminationGuarantee;
use symbols::SymbolHandle;

pub(super) fn maximum_fact_projection_length(semantic: &facts::FactPlan) -> usize {
    semantic
        .facts
        .iter()
        .filter_map(|(_, fact)| fact_subject(semantic, fact.place))
        .map(|subject| subject.projections.len())
        .max()
        .unwrap_or(0)
}

pub(super) fn finite_projection_limit(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    maximum_fact_projection_length: usize,
    component: &[SymbolHandle],
    summaries: &[CheckedProgressSummary],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<usize> {
    let mut parameter_count = 0usize;
    let mut maximum_prefix_length = 0usize;
    let mut maximum_seed_length = maximum_fact_projection_length;
    for machine in program
        .machines()
        .iter()
        .filter(|machine| component.contains(&machine.symbol))
    {
        // Each state formal is a distinct transport root. The exact call
        // argument walk below includes local arrivals as well as nested calls;
        // finite-prefix growth therefore counts the whole activation graph.
        for state in program.machine_states(machine) {
            parameter_count = parameter_count.checked_add(program.state_parameters(state).len())?;
        }
        for (_, state) in flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == machine.symbol)
        {
            for call in flow.control.calls.span_or_empty(state.calls) {
                let private_component_call = program
                    .machine_by_symbol(call.target_symbol)
                    .or_else(|| {
                        validation::find_state(program, call.target_symbol)
                            .map(|(machine, _)| machine)
                    })
                    .is_some_and(|callee| {
                        component.contains(&callee.symbol)
                            && matches!(
                                callee.termination_plan.interface,
                                language_semantics::TerminationInterface::InternalDerived
                            )
                    });
                if !private_component_call
                    && let Some(selected) =
                        selected_call_summary(program, call.target_symbol, summaries)
                    && let TerminationGuarantee::Terminates { premises } = selected.guarantee
                {
                    for premise in premises {
                        maximum_seed_length =
                            maximum_seed_length.max(premise.subject.projections.len());
                    }
                }
                if let Some(parameters) =
                    validation::call_target_parameters(program, call.target_symbol)
                {
                    for parameter in parameters {
                        if let Some(subject) = call_argument_subject(
                            program,
                            machine,
                            state,
                            call,
                            parameters,
                            parameter.symbol,
                            call_frames,
                        ) {
                            maximum_prefix_length =
                                maximum_prefix_length.max(subject.projections.len());
                        }
                    }
                }
            }
        }
    }
    // Transport only prefixes a fixed call-argument path and rebinds its root.
    // Beyond every seed and exact receipt length, more positive-prefix edges
    // than parameter roots repeat a root on a growing cycle. That cycle can
    // be repeated without reaching any finite exact receipt. One extra step
    // includes the initial external call's actual-argument projection.
    maximum_seed_length.checked_add(
        parameter_count
            .checked_add(1)?
            .checked_mul(maximum_prefix_length)?,
    )
}
