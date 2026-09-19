//! The declared field facts assumed by a nominal parameter are obligations on
//! each incoming actual. Reuse the exact semantic declaration rows; do not
//! recover lost membership merely from an unchanged nominal type annotation.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use facts::{
    FactContextHandle, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment,
    ProgramPoint,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Parameter(SymbolHandle),
    Machine(SymbolHandle),
}

pub(super) struct DeclaredFieldRequirements {
    rows: Vec<(Owner, Vec<FieldRequirement>)>,
}

type FieldRequirement = (
    Vec<PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
);

impl DeclaredFieldRequirements {
    pub(super) fn new(semantic: &FactPlan) -> Self {
        let mut rows: Vec<(Owner, Vec<FieldRequirement>)> = Vec::new();
        for (_, fact) in semantic.facts.iter() {
            let FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            } = fact.payload
            else {
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
                semantic_domain,
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

    fn requirements(&self, owner: Owner) -> &[FieldRequirement] {
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
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(site) = crate::semantic_calls::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return;
    };
    let Some(parameters) =
        crate::semantic_calls::call_target_parameters(program, call.target_symbol)
    else {
        return;
    };
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, &site);
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
                call.statement_index,
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
        for (segments, domain_symbol, semantic_domain) in requirements.requirements(owner) {
            if argument.is_some_and(|argument| {
                crate::flow::literal_value_path_is_inactive(
                    program,
                    argument,
                    parameter.type_reference,
                    segments,
                )
            }) {
                continue;
            }
            let proves = |subject: &crate::flow::CanonicalPlace| {
                if crate::facts::field_domain::domain_requires_provenance(program, *domain_symbol) {
                    super::exits::exact_scalar_membership(
                        program,
                        facts,
                        contexts,
                        subject,
                        *domain_symbol,
                        *semantic_domain,
                    )
                } else {
                    super::prover::prove_domain_at_place(
                        program,
                        &facts.semantic,
                        contexts,
                        subject,
                        *domain_symbol,
                    )
                }
            };
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
                                if !crate::flow::place_cases_are_selected(
                                    program,
                                    &facts.semantic,
                                    contexts,
                                    state.machine_symbol,
                                    state.state_symbol,
                                    call.statement_index,
                                    &subject,
                                ) {
                                    return false;
                                }
                                subject.extend_segments(&projection.remaining);
                                proves(&subject)
                            })
                        })
                })
                || actual.as_ref().is_some_and(|actual| {
                    if !crate::flow::place_cases_are_selected(
                        program,
                        &facts.semantic,
                        contexts,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        actual,
                    ) {
                        return false;
                    }
                    let mut subject = actual.clone();
                    subject.extend_segments(segments);
                    proves(&subject)
                        || candidate_referents_prove_domain(
                            program,
                            facts,
                            state,
                            call,
                            actual,
                            segments,
                            *domain_symbol,
                            *semantic_domain,
                            contexts,
                            call_frames,
                        )
                });
            if !satisfied {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove default-domain field requirement for call {} from {}::{}: parameter {} requires {}",
                    crate::labels::call_target_label(program, call.target_symbol),
                    crate::labels::machine_name(program, state.machine_symbol),
                    crate::labels::symbol_name(program, state.state_symbol),
                    crate::labels::canonical_place_label_from_parts(program, PlaceRoot::Symbol(parameter.symbol), segments),
                    crate::labels::symbol_name(program, *domain_symbol),
                )));
            }
        }
    }
}

/// A `&mut` local bound from a checked reference result names one of
/// finitely many referent candidates -- `level.rooms[0]` .. `level.rooms[15]`
/// for `level.room_mut(cell)`. The callee receives whichever storage the loan
/// actually names, so an actual spelled through the local discharges a
/// declared-field row only when EVERY candidate proves it; anything less is
/// the one-exact-origin refusal the single-origin resolver already gives.
fn candidate_referents_prove_domain(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &crate::flow::CanonicalPlace,
    segments: &[PlaceSegment],
    domain_symbol: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
    contexts: &[FactContextHandle],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state.machine_symbol)
    else {
        return false;
    };
    let mut owned_frames = None;
    let Some(frames) = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
    else {
        return false;
    };
    let Some(candidates) = crate::flow::local_reference_candidate_storages_at_call(
        program,
        frames,
        &facts.borrow,
        machine,
        &facts.flow,
        state,
        call,
        actual.clone(),
    ) else {
        return false;
    };
    candidates.iter().all(|candidate| {
        let mut subject = candidate.clone();
        subject.extend_segments(segments);
        if crate::facts::field_domain::domain_requires_provenance(program, domain_symbol) {
            super::exits::exact_scalar_membership(
                program,
                facts,
                contexts,
                &subject,
                domain_symbol,
                semantic_domain,
            )
        } else {
            super::prover::prove_domain_at_place(
                program,
                &facts.semantic,
                contexts,
                &subject,
                domain_symbol,
            )
        }
    })
}
