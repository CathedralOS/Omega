//! The declared field facts assumed by a nominal parameter are obligations on
//! each incoming actual. Reuse the exact semantic declaration rows; do not
//! recover lost membership merely from an unchanged nominal type annotation.
//! Completed operand projections are the only actual correspondence: copies
//! keep completion evidence and views consult invocation facts on their frozen
//! referents. A historical local-origin replay is not a second proof route.
//! Conditional payload validity does not select an active case; only the
//! authored source access carries that separate selection obligation.

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use facts::{FactOrigin, FactPayload, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint};
use symbols::SymbolHandle;

pub(super) struct DeclaredFieldRequirements {
    rows: Vec<(SymbolHandle, Vec<FieldRequirement>)>,
}

type FieldRequirement = (
    Vec<PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
);

impl DeclaredFieldRequirements {
    pub(super) fn new(semantic: &FactPlan) -> Self {
        let mut rows: Vec<(SymbolHandle, Vec<FieldRequirement>)> = Vec::new();
        for (_, fact) in semantic.facts.iter() {
            let FactPayload::DomainMembership {
                domain_symbol,
                semantic_domain,
                ..
            } = fact.payload
            else {
                continue;
            };
            let Some(place) = fact.place.as_place() else {
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
                    let Some(symbol) = place.root.as_symbol() else {
                        continue;
                    };
                    symbol
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
                crate::collections::push_unique(&mut *requirements, requirement);
            } else {
                rows.push((owner, vec![requirement]));
            }
        }
        Self { rows }
    }

    fn requirements(&self, owner: SymbolHandle) -> &[FieldRequirement] {
        self.rows
            .iter()
            .find(|(candidate, _)| *candidate == owner)
            .map_or(&[], |(_, rows)| rows)
    }
}

pub(super) fn check(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirements: &DeclaredFieldRequirements,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if requirements.rows.is_empty() {
        return;
    }
    let Some(parameters) = validation::call_target_parameters(program, call.target_symbol) else {
        return;
    };
    if !parameters
        .iter()
        .any(|parameter| !requirements.requirements(parameter.symbol).is_empty())
    {
        return;
    }
    let Some(site) = validation::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return;
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    // Field obligations project different subjects from the same completed
    // operand. Select each exact namespace and epoch once; each field
    // still proves its own case validity and exact domain obligation below.
    let mut operand_contexts = crate::flow::OperandContextSelections::new(
        program,
        &facts.flow,
        &facts.semantic,
        state.machine_symbol,
    );
    let mut argument_index = 0;
    for parameter in parameters {
        let argument = {
            let argument = arguments.get(argument_index).copied();
            argument_index += 1;
            argument
        };
        // Every formal owes the same declared fields, including a subject
        // forwarded from the caller, independently of declaration qualification.
        // Its nominal type cannot restore evidence invalidated by a write.
        for (segments, domain_symbol, semantic_domain) in
            requirements.requirements(parameter.symbol)
        {
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
            // The required field may have completed before a later sibling
            // changed its source. Select its capture independently, including
            // every candidate of a runtime-indexed constructed array.
            let captured_actual = super::call_values::actual_at(
                program,
                state,
                call,
                argument_index - 1,
                segments.clone(),
            );
            // Every obligation uses the completed operand tree. Detached
            // children retain their completion epoch; live views use the
            // invocation facts on every retained referent. Replaying a local
            // initializer cannot recover a missing storage correspondence.
            let satisfied = captured_actual
                .as_ref()
                .and_then(|actual| {
                    super::call_values::captured_values(program, facts, state, call, actual)
                })
                .is_some_and(|projections| {
                    !projections.is_empty()
                        && projections.iter().all(|captured| {
                            // A reaching producer explains the value, but a
                            // later observation also retains qualifications
                            // established on the stored copy. Keep their
                            // subjects and epochs paired; never union contexts.
                            [
                                (
                                    captured.observation,
                                    captured.observation_type,
                                    crate::flow::captured_observation_places(
                                        program,
                                        &facts.flow,
                                        captured,
                                    ),
                                    call.entry_constraints,
                                    (state.state_symbol, call.statement_index),
                                ),
                                (
                                    captured.operand,
                                    captured.type_reference,
                                    crate::flow::captured_value_places(
                                        program,
                                        &facts.flow,
                                        captured,
                                    ),
                                    captured.producer_invocation(call.entry_constraints),
                                    captured.producer_frontier(
                                        state.state_symbol,
                                        call.statement_index,
                                    ),
                                ),
                            ]
                            .into_iter()
                            .any(
                                |(
                                    operand,
                                    reference,
                                    subjects,
                                    invocation,
                                    (source_state, source_statement),
                                )| {
                                    let Some(selection) = operand_contexts.for_operand(
                                        operand,
                                        reference,
                                        invocation,
                                        source_state,
                                        source_statement,
                                    ) else {
                                        return false;
                                    };
                                    subjects.is_some_and(|subjects| {
                                        !subjects.is_empty()
                                            && subjects.iter().all(|place| {
                                                // A captured empty collection has no
                                                // child occurrences to qualify. An
                                                // empty carrier itself still owes
                                                // its own domain; only a strict
                                                // pointwise ancestor is vacuous.
                                                if pointwise_field_is_empty(
                                                    program,
                                                    &operand,
                                                    &place.projected,
                                                ) {
                                                    return true;
                                                }
                                                // Only the authored source selects
                                                // a case. A declared field row is
                                                // conditional validity, not a read
                                                // that requires its case active.
                                                selection.cases_are_selected(&place.storage)
                                                    && (selection
                                                        .path_is_inactive(&place.projected)
                                                        || prove_declared_field_domain(
                                                            program,
                                                            facts,
                                                            selection,
                                                            &place.projected,
                                                            *domain_symbol,
                                                            *semantic_domain,
                                                        ))
                                            })
                                    })
                                },
                            )
                        })
                });
            if !satisfied {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove default-domain field requirement for call {} from {}::{}: parameter {} requires {}",
                    crate::labels::call_target_label(program, call.target_symbol),
                    crate::labels::machine_name(program, state.machine_symbol),
                    crate::labels::symbol_name(program, state.state_symbol),
                    facts::canonical_place_label_from_parts(program, PlaceRoot::Symbol(parameter.symbol), segments),
                    crate::labels::symbol_name(program, *domain_symbol),
                )));
            }
        }
    }
}

/// Empty pointwise ancestors contain no qualified field occurrences.
fn pointwise_field_is_empty(
    program: &dyn ::validation::CheckingInput,
    operand: &checked_trees::FlowOperandFact,
    place: &validation::CanonicalPlace,
) -> bool {
    place
        .segments
        .iter()
        .enumerate()
        .any(|(position, segment)| {
            let PlaceSegment::FixedRange { start, end } = segment else {
                return false;
            };
            if position + 1 == place.segments.len() {
                return false;
            }
            if start == end {
                return true;
            }
            if *start != 0 || *end != usize::MAX {
                return false;
            }
            // A whole fixed-array loan need not spell an address range. Resolve
            // the universal field dimension against its actual source declaration,
            // whose exact extent cannot change through qualification or mutation.
            let prefix = validation::CanonicalPlace {
                root: place.root,
                segments: place.segments[..position].to_vec(),
            };
            validation::canonical_place_type_reference(
                program,
                operand.state_symbol,
                operand.statement_index,
                &prefix,
            )
            .is_some_and(|reference| {
                validation::fixed_array_type_length(program, reference) == Some(0)
            })
        })
}

/// A field qualification carries its complete domain application. Predicate
/// proof may discharge an atomic domain; it cannot select an indexed instance
/// or manufacture establishment provenance. Both exact and candidate referents
/// use this same judgment.
fn prove_declared_field_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    selection: &crate::flow::OperandContextSelection<'_>,
    subject: &validation::CanonicalPlace,
    domain_symbol: SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
) -> bool {
    if validation::field_domains::domain_requires_provenance(program, domain_symbol)
        || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
    {
        selection.has_membership(subject, domain_symbol, semantic_domain)
    } else {
        super::prover::prove_domain_at_place(
            program,
            &facts.semantic,
            &selection.contexts,
            subject,
            domain_symbol,
        )
    }
}
