//! A structural bound on finite premise transport, not an iteration budget.

use super::super::{
    CheckedProgressSummary, call_argument_subject_with_parameters, fact_subject,
    selected_call_summary,
};
use checked_trees::FlowFacts;
use language_semantics::TerminationGuarantee;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(super) fn finite_projection_limit(
    program: &TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    component: &[SymbolHandle],
    summaries: &[CheckedProgressSummary],
) -> Option<usize> {
    let mut parameter_count = 0usize;
    let mut maximum_prefix_length = 0usize;
    let mut maximum_seed_length = semantic
        .facts
        .iter()
        .filter_map(|(_, fact)| fact_subject(semantic, fact.place))
        .map(|subject| subject.projections.len())
        .max()
        .unwrap_or(0);
    for machine in program
        .machines()
        .iter()
        .filter(|machine| component.contains(&machine.symbol))
    {
        // The shared runtime ranking currently admits one entry state and no
        // internal state loop. Rebinding through multiple states needs its own
        // finite transport judgment before it can extend this component rule.
        let [entry] = program.machine_states(machine) else {
            return None;
        };
        parameter_count = parameter_count.checked_add(program.state_parameters(entry).len())?;
        for (_, state) in flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == machine.symbol)
        {
            for call in flow.control.calls.span_or_empty(state.calls) {
                let private_component_call = program.machines().iter().any(|callee| {
                    component.contains(&callee.symbol)
                        && matches!(
                            callee.termination_plan.interface,
                            language_semantics::TerminationInterface::InternalDerived
                        )
                        && (callee.symbol == call.target_symbol
                            || program
                                .machine_states(callee)
                                .iter()
                                .any(|state| state.symbol == call.target_symbol))
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
                if let Some(parameters) = crate::call_target_parameters(program, call.target_symbol)
                {
                    for parameter in parameters {
                        if let Some(subject) = call_argument_subject_with_parameters(
                            program,
                            machine,
                            state,
                            call,
                            parameters,
                            parameter.symbol,
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
