//! scalar exits slice of result_domains.

use validation::{CanonicalPlace, canonical_place_from_expression_in_state};

use super::super::super::return_values::{exit_return_call, exit_return_expression};
use super::proofs::*;
use crate::flow::canonical_place_from_semantic_place;
use checked_trees::{CheckFacts, FlowExitFact};
use diagnostics::Diagnostic;
use facts::FactContextHandle;
use typed_trees::types::TypeReferenceNode;

/// Shared by the exit census and its consumer: a scalar domain annotation
/// needs a live return context even when there is no authored `ensures`.
pub(crate) fn scalar_result_domains(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Vec<&typed_trees::types::DomainConstraint> {
    // Reference and nominal field obligations retain their existing owners.
    // All primitive scalar carriers participate, not only integer predicates.
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(return_type);
    if matches!(
        program.type_reference_table.type_reference(carrier),
        TypeReferenceNode::Reference { .. }
    ) || program.primitive_type_reference(carrier).is_none()
    {
        return Vec::new();
    }
    let mut domains = Vec::new();
    let mut reference = return_type;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        domains.extend(
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .filter_map(|constraint| match constraint {
                    typed_trees::types::TypeConstraintNode::Domain(domain)
                        if domain.symbol.is_valid() =>
                    {
                        Some(domain)
                    }
                    _ => None,
                }),
        );
        reference = *base_type;
    }
    domains
}

/// A scalar signature promises membership to callers, but supplies no evidence
/// to its own body. Consume the returned subject's live exit facts before
/// attempting fresh establishment. In particular, a declaration symbol alone
/// cannot identify an indexed domain application, and predicate truth alone
/// cannot establish a routed qualification.
pub(in crate::checks::contracts) fn check_scalar_result_domains(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    decisions: &mut Vec<checked_trees::CheckedScalarReturnQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(machine) = program.machine(exit.machine_symbol) else {
        return;
    };
    let domains = scalar_exit_domains(program, machine, exit.state_symbol);
    if domains.is_empty() {
        return;
    }
    let returned = exit_return_expression(program, exit);
    let contexts = facts
        .flow
        .contexts
        .semantic_context_refs
        .span_or_empty(exit.entry_semantic_contexts)
        .iter()
        .map(|reference| reference.context)
        .collect::<Vec<_>>();
    let subject = returned
        .is_valid()
        .then(|| {
            canonical_place_from_expression_in_state(
                program,
                exit.state_symbol,
                exit.statement_index,
                returned,
            )
        })
        .flatten();
    if subject.as_ref().is_some_and(|subject| {
        !crate::flow::place_cases_are_selected(
            program,
            &facts.semantic,
            &contexts,
            exit.machine_symbol,
            exit.state_symbol,
            exit.statement_index,
            subject,
        )
    }) {
        diagnostics.push(Diagnostic::error(
            "cannot prove selected case for qualified scalar return",
        ));
        return;
    }
    for constraint in domains {
        let domain_symbol = constraint.symbol;
        let semantic_domain = constraint.semantic_id;
        // Empty theories add no establishment obligation, including indexed
        // instances and aliases whose constituents are all empty theories.
        // Reuse the same rule as checked qualification casts; index identity
        // remains significant when transporting nonvacuous membership below.
        if crate::facts::domain_is_vacuous(program, domain_symbol, &mut Vec::new()) {
            continue;
        }
        let transported = subject.as_ref().is_some_and(|subject| {
            exact_scalar_membership(
                program,
                facts,
                &contexts,
                subject,
                domain_symbol,
                semantic_domain,
            )
        });
        // No actual value means no subject on which to establish a nonempty
        // theory or routed provenance. A signature does not synthesize zero.
        let supplied_by_return = exit_return_call(program, facts, exit)
            .is_some_and(|call| call_result_has_domain(program, call.target_symbol, constraint));
        let origin = if supplied_by_return || (returned.is_valid() && transported) {
            Some(checked_trees::CheckedScalarReturnOrigin::Forwarded)
        } else {
            program.domain_definition(domain_symbol).and_then(|domain| {
                // Authorization cannot choose a domain's indexed application.
                if !returned.is_valid()
                    || (!typed_trees::domain::index_parameters(program, domain).is_empty()
                        && !domain.establishment_routes.is_empty())
                {
                    return None;
                }
                let origin =
                    if domain.establishment_routes.is_empty() {
                        checked_trees::CheckedScalarReturnOrigin::Predicate
                    } else {
                        checked_trees::CheckedScalarReturnOrigin::Established(
                        crate::facts::qualification_evidence::machine_domain_establishment_route(
                            program, machine, domain_symbol,
                        )?,
                    )
                    };
                establishes_scalar_predicates(program, facts, exit, &contexts, domain, constraint)
                    .then_some(origin)
            })
        };
        if let Some(origin) = origin {
            decisions.push(checked_trees::CheckedScalarReturnQualification {
                machine: exit.machine_symbol,
                state: exit.state_symbol,
                statement_index: exit.statement_index,
                transition_target: exit.transition_target,
                expression: returned,
                semantic_domain,
                domain: domain_symbol,
                origin,
            });
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove scalar result domain for return from {} at statement {}: {}",
                crate::labels::machine_name(program, exit.machine_symbol),
                exit.statement_index,
                crate::labels::symbol_name(program, domain_symbol),
            )));
        }
    }
}

/// Both the machine's external result and the current state's result are
/// obligations. Identical instances need only one establishment judgment.
pub(super) fn scalar_exit_domains<'program>(
    program: &'program dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state_symbol: symbols::SymbolHandle,
) -> Vec<&'program typed_trees::types::DomainConstraint> {
    let mut domains = Vec::new();
    for state in program.machine_states(machine).iter().filter(|state| {
        state.symbol == state_symbol
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == state.symbol)
    }) {
        for constraint in scalar_result_domains(program, state.return_type) {
            if !domains
                .iter()
                .any(|existing: &&typed_trees::types::DomainConstraint| {
                    existing.symbol == constraint.symbol
                        && existing.semantic_id == constraint.semantic_id
                })
            {
                domains.push(constraint);
            }
        }
    }
    domains
}

/// A local state transfer keeps the machine invocation alive, but cannot erase
/// a stronger result qualification promised by its source state's signature.
/// Consume its retained call occurrence and checked result contract; it has
/// no authored value expression to manufacture for the ordinary exit prover.
/// Transfers within this machine preserve additional state promises through
/// the selected target contract. The common machine promise is checked at leaf
/// exits, so intermediate states need not repeat it in their annotations.
pub(in crate::checks::contracts) fn check_state_transfer_result_domains(
    program: &dyn ::validation::CheckingInput,
    state_flow: &checked_trees::FlowStateFact,
    call: &checked_trees::FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(validation::CallSite::TransitionNamed { path, .. }) = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    ) else {
        return;
    };
    let Some(machine) = program.machine(state_flow.machine_symbol) else {
        return;
    };
    let Some(state) = program.state_in_machine(machine, state_flow.state_symbol) else {
        return;
    };
    if !matches!(program.statement_table.statement_at(state, call.statement_index),
        Some(typed_trees::statement::StatementNode::Transition(transition))
        if transition.exit == typed_trees::statement::TransitionExit::Ordinary)
    {
        return;
    }
    let selected = crate::proof::contract_target_from_state_symbol(program, call.target_symbol);
    let exact_invocation = selected.is_some()
        && crate::proof::contract_target_from_state_symbol(program, path.symbol) == selected;
    let local_transfer =
        exact_invocation && selected.is_some_and(|(owner, _)| owner == machine.symbol);
    if !local_transfer {
        return;
    }
    let common_domains = program
        .machine_states(machine)
        .first()
        .map(|entry| scalar_result_domains(program, entry.return_type))
        .unwrap_or_default();
    for required in scalar_exit_domains(program, machine, state.symbol) {
        if common_domains.iter().any(|common| {
            common.symbol == required.symbol && common.semantic_id == required.semantic_id
        }) {
            continue;
        }
        if crate::facts::domain_is_vacuous(program, required.symbol, &mut Vec::new()) {
            continue;
        }
        if exact_invocation && call_result_has_domain(program, call.target_symbol, required) {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove scalar result domain for tail return from {} at statement {}: {}",
            crate::labels::machine_name(program, machine.symbol),
            call.statement_index,
            crate::labels::symbol_name(program, required.symbol),
        )));
    }
}

pub(super) fn call_result_has_domain(
    program: &dyn ::validation::CheckingInput,
    target: symbols::SymbolHandle,
    required: &typed_trees::types::DomainConstraint,
) -> bool {
    let target = crate::proof::contract_target_from_state_symbol(program, target)
        .map_or(target, |(_, state)| state);
    validation::call_target_return_type(program, target).is_some_and(|return_type| {
        required.semantic_id.is_valid()
            && scalar_result_domains(program, return_type)
                .iter()
                .any(|provided| {
                    provided.symbol == required.symbol
                        && provided.semantic_id == required.semantic_id
                })
    })
}

pub(crate) fn exact_scalar_membership(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    subject: &CanonicalPlace,
    domain_symbol: symbols::SymbolHandle,
    semantic_domain: language_semantics::SemanticDomainId,
) -> bool {
    // A live whole-extent row also proves one selected element's membership.
    // Narrow its coordinates with the same coverage rule as call obligations;
    // preserve the exact domain instance and never reread a declaration to
    // restore a row that storage invalidation has retired.
    semantic_domain.is_valid()
        && contexts.iter().any(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
                .any(|fact| {
                    let Some((_, _, candidate, instance)) = fact.payload.domain_membership() else {
                        return false;
                    };
                    let Some(place) = fact.place.as_place() else {
                        return false;
                    };
                    candidate == domain_symbol
                        && instance == semantic_domain
                        && canonical_place_from_semantic_place(
                            program,
                            &facts.semantic,
                            facts.semantic.places.get(place),
                        )
                        .is_some_and(|candidate| {
                            candidate.root == subject.root
                                && crate::checks::contracts::prover::segments_cover_subject(
                                    program,
                                    &candidate.segments,
                                    &subject.segments,
                                )
                        })
                })
        })
}
