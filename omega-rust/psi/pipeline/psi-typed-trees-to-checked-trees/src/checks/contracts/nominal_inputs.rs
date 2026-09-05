//! The declared field facts assumed by a nominal parameter are obligations on
//! each incoming actual. Reuse the exact semantic declaration rows; do not
//! recover lost membership merely from an unchanged nominal type annotation.

use psi_checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use psi_diagnostics::Diagnostic;
use psi_facts::{
    FactContextHandle, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment,
    ProgramPoint,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Parameter(SymbolHandle),
    Machine(SymbolHandle),
}

pub(super) struct DeclaredFieldRequirements {
    rows: Vec<(Owner, Vec<(Vec<PlaceSegment>, SymbolHandle)>)>,
}

impl DeclaredFieldRequirements {
    pub(super) fn new(semantic: &FactPlan) -> Self {
        let mut rows: Vec<(Owner, Vec<(Vec<PlaceSegment>, SymbolHandle)>)> = Vec::new();
        for (_, fact) in semantic.facts.iter() {
            let FactPayload::DomainMembership { domain_symbol, .. } = fact.payload else {
                continue;
            };
            let FactPlace::Place(place) = fact.place else {
                continue;
            };
            let place = semantic.places.get(place);
            if place.segments.is_empty() {
                continue;
            }
            let owner = match fact.origin {
                FactOrigin::StateParameterDomain {
                    machine_symbol,
                    state_symbol,
                } if fact.point
                    == (ProgramPoint::State {
                        machine_symbol,
                        state_symbol,
                    }) =>
                {
                    let PlaceRoot::Symbol(symbol) = place.root else {
                        continue;
                    };
                    Owner::Parameter(symbol)
                }
                FactOrigin::MachineFieldDomain { machine_symbol }
                    if fact.point == (ProgramPoint::Machine { machine_symbol }) =>
                {
                    Owner::Machine(machine_symbol)
                }
                _ => continue,
            };
            let requirement = (
                semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .to_vec(),
                domain_symbol,
            );
            if let Some((_, requirements)) =
                rows.iter_mut().find(|(candidate, _)| *candidate == owner)
            {
                if !requirements.contains(&requirement) {
                    requirements.push(requirement);
                }
            } else {
                rows.push((owner, vec![requirement]));
            }
        }
        Self { rows }
    }

    fn requirements(&self, owner: Owner) -> &[(Vec<PlaceSegment>, SymbolHandle)] {
        self.rows
            .iter()
            .find(|(candidate, _)| *candidate == owner)
            .map_or(&[], |(_, rows)| rows)
    }
}

pub(super) fn check(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirements: &DeclaredFieldRequirements,
    contexts: &[FactContextHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(site) = crate::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return;
    };
    let Some(parameters) = crate::call_target_parameters(program, call.target_symbol) else {
        return;
    };
    let arguments = crate::call_site_argument_expressions(program, &site);
    let target_machine = program.machines().iter().find(|machine| {
        machine.symbol == call.target_symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
    });
    let mut argument_index = 0;
    for parameter in parameters {
        let argument = if parameter.is_self {
            None
        } else {
            let argument = arguments.get(argument_index).copied();
            argument_index += 1;
            argument
        };
        let actual = if parameter.is_self {
            crate::flow::canonical_receiver_place_for_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                &site,
            )
        } else {
            argument.and_then(|argument| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.state_symbol,
                    call.statement_index,
                    argument,
                )
            })
        };
        let owner = if parameter.is_self {
            Owner::Machine(target_machine.map_or(SymbolHandle::invalid(), |machine| machine.symbol))
        } else {
            Owner::Parameter(parameter.symbol)
        };
        for (segments, domain_symbol) in requirements.requirements(owner) {
            let satisfied = argument
                .and_then(|argument| {
                    crate::flow::literal_value_projections(
                        program,
                        argument,
                        parameter.type_reference,
                        segments,
                        false,
                    )
                })
                .is_some_and(|projections| {
                    !projections.is_empty()
                        && projections.iter().all(|projection| {
                            crate::flow::canonical_place_from_expression_in_state(
                                program,
                                state.state_symbol,
                                call.statement_index,
                                projection.expression,
                            )
                            .is_some_and(|mut subject| {
                                subject.extend_segments(&projection.remaining);
                                super::prover::prove_domain_at_place(
                                    program,
                                    &facts.semantic,
                                    contexts,
                                    &subject,
                                    *domain_symbol,
                                )
                            })
                        })
                })
                || actual.as_ref().is_some_and(|actual| {
                    let mut subject = actual.clone();
                    subject.extend_segments(segments);
                    super::prover::prove_domain_at_place(
                        program,
                        &facts.semantic,
                        contexts,
                        &subject,
                        *domain_symbol,
                    )
                });
            if !satisfied {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove default-domain field requirement for call {} from {}: parameter {} requires {}",
                    crate::labels::call_target_label(program, call.target_symbol),
                    crate::labels::machine_name(program, state.machine_symbol),
                    crate::labels::canonical_place_label_from_parts(program, PlaceRoot::Symbol(parameter.symbol), segments),
                    crate::labels::symbol_name(program, *domain_symbol),
                )));
            }
        }
    }
}
