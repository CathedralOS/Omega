//! Admission authorizes a route, not a contradiction. Exact applied provider
//! obligations stay separate from body assumptions and direct-call guarantees.
//! Completed values and live storage observations may refute their predicates;
//! unknown predicates grant neither a veto nor evidence of consistency.

use super::*;
use crate::facts::boolean_predicates;

pub(in crate::checks::contracts) struct BoundaryRefutations {
    machines: Vec<(
        symbols::SymbolHandle,
        Vec<validation::BoundaryQualificationObligation>,
        crate::flow::MachineParameterOrigins,
    )>,
}

impl BoundaryRefutations {
    pub(in crate::checks::contracts) fn new(program: &dyn validation::CheckingInput) -> Self {
        let machines = program
            .machines()
            .iter()
            .filter_map(|machine| {
                let obligations =
                    validation::boundary_provider_qualification_obligations(program, machine);
                if obligations.is_empty() {
                    return None;
                }
                Some((
                    machine.symbol,
                    obligations,
                    crate::flow::machine_parameter_origins(program, machine),
                ))
            })
            .collect();
        Self { machines }
    }

    pub(in crate::checks::contracts) fn check<'program>(
        &self,
        program: &'program dyn validation::CheckingInput,
        facts: &CheckFacts,
        exit: &FlowExitFact,
        call_frames: Option<&validation::CallFrameResolver<'program>>,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        let Some((_, obligations, origins)) = self
            .machines
            .iter()
            .find(|(machine, _, _)| *machine == exit.machine_symbol)
        else {
            return;
        };
        let Some((machine, state)) =
            program.state_flow_site(exit.machine_symbol, exit.state_symbol)
        else {
            return;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            return;
        };
        let origins = crate::flow::state_parameter_origins(program, machine, state, origins);
        let contexts = facts
            .flow
            .contexts
            .semantic_context_refs
            .span_or_empty(exit.entry_semantic_contexts)
            .iter()
            .map(|reference| reference.context)
            .collect::<Vec<_>>();
        let retained =
            boolean_predicates::retained(program, &facts.operators, &facts.semantic, &contexts);
        let evaluator = ExitScalars {
            program,
            facts,
            machine,
            exit,
            contexts: &contexts,
            contract: Default::default(),
            call_frames,
        };
        let returned = exit_return_expression(program, exit);
        for obligation in obligations {
            let Some(domain) = program.domain_definition(obligation.domain) else {
                continue;
            };
            let completed_refutation = !obligation.parameter.is_valid()
                && validation::completed_value_refutes_domain(
                    program,
                    machine,
                    state,
                    exit.statement_index,
                    returned,
                    obligation.domain,
                );
            let subject = if !obligation.parameter.is_valid() {
                canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    exit.statement_index,
                    returned,
                )
            } else if obligation.realization_state == entry.symbol {
                origins
                    .iter()
                    .find(|(source, _)| *source == obligation.parameter)
                    .and_then(|(_, target)| canonical_place_from_symbol(*target))
            } else if obligation.realization_state == state.symbol
                && validation::state_reference_parameter_binding_is_stable(
                    program,
                    machine,
                    state,
                    obligation.parameter,
                )
            {
                canonical_place_from_symbol(obligation.parameter)
            } else {
                None
            };
            let live_refutation = subject
                .and_then(|subject| {
                    boolean_predicates::domain_predicates_for_subject(
                        program,
                        &facts.operators,
                        domain,
                        &subject,
                    )
                })
                .is_some_and(|predicates| {
                    predicates.iter().any(|predicate| {
                        boolean_predicates::truth(&retained, predicate, |place| {
                            evaluator.value_at_place(place)
                        }) == Some(false)
                    })
                });
            if completed_refutation || live_refutation {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "boundary qualification is refuted at exit of {} in state `{}` for requirement `{}`: {}",
                    crate::labels::machine_name(program, machine.symbol), state.name,
                    program.symbols.name(obligation.requirement), domain.name,
                )));
            }
        }
    }
}
