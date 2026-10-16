//! Whether fact contexts prove a contract proposition. The contract checks
//! ask through three entries: `assembly` and `exits` use the first, `calls`
//! the second, and `arrivals`, `calls` and `exits` the third.
//!
//! - `semantic_contexts_prove_boolean_expression`: a Boolean expression
//!   holds in some context, by `booleans` (conjunction and disjunction over
//!   the `direct` and `domains` provers).
//! - `call_entry_contexts_prove_boolean_contract_expression`: a callee's
//!   Boolean contract expression at one call site. It tries, in order, a
//!   surviving call guarantee (`call_guarantees`), the caller's case at the
//!   place a case test's subject names, Boolean field facts on the actual's
//!   referent (`field_actuals`), the instantiated expression in each entry
//!   context (`booleans`, skipped when `field_actuals` recognized the
//!   requirement), and last the expression evaluated with the call's
//!   arguments substituted (`evaluator`).
//! - `semantic_contexts_prove_contract_fact`: one contract fact, by payload
//!   kind: domain membership (a matching or covering fact, or
//!   `assigned_values`), carry permission, Boolean expression, proposition
//!   application or carry origin. Evidence and deferred payloads are never
//!   proved here.
//!
//! `assigned_values` (domain predicates over live assigned values) and
//! `scalars` (scalar Boolean evaluation) are re-exported for the other
//! contract checks, and `call_guarantees` is also used by `checks::borrows`.

use checked_trees::{FlowCallFact, FlowStateFact};
use facts::{FactPayload, FactPlace};

mod assigned_values;
pub(super) use assigned_values::{
    byte_predicate_proven_at_place, prove_domain_at_place, scalar_value_at_place,
    segments_cover_subject,
};
mod scalars;
pub(super) use crate::values::evaluate_checked_scalar;
pub(super) use scalars::{
    ScalarValue, closed_boolean_value, evaluate as evaluate_scalar, evaluate_for_atoms,
    has_builtin_operators,
};
mod booleans;
mod place_predicates;
pub(super) use place_predicates::place_predicate;
pub(in crate::checks) mod call_guarantees;
mod current_boolean;
mod field_actuals;
pub(crate) use current_boolean::boolean_value_at_place;

use self::booleans::{
    semantic_context_proves_boolean_expression,
    semantic_context_proves_instantiated_boolean_expression,
};
use super::evaluator::call_site_proves_boolean_contract_expression;

pub(super) fn semantic_contexts_prove_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    entry_contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Boolean(true)
    ) || entry_contexts.iter().any(|entry_context| {
        semantic_context_proves_boolean_expression(
            program,
            semantic,
            semantic.contexts.get(*entry_context),
            expression,
        )
    })
}

/// Evaluate selected scalar operations over the live storage observations at
/// one source point. Unlike symbolic entry propositions, these snapshots have
/// already survived the flow owner's write and call invalidation.
pub(super) fn live_contexts_prove_boolean_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(selected) = crate::values::lower_unit_scalar_argument(
        program,
        &facts.operators,
        state,
        statement_index,
        expression,
        typed_trees::types::PrimitiveType::Bool,
    ) else {
        return false;
    };
    let Some(symbols) = crate::values::unit_scalar_binding_symbols(program, state, statement_index)
    else {
        return false;
    };
    let semantic = &facts.semantic;
    let mut values = crate::values::PlaceScalarValues {
        program,
        parameters: program.state_parameters(state),
        symbols: &symbols,
        value_at_place: |place: &validation::CanonicalPlace| {
            scalar_value_at_place(
                program,
                semantic,
                contexts
                    .iter()
                    .map(|context| semantic.contexts.get(*context)),
                place,
            )
        },
    };
    crate::values::evaluate_checked_scalar(&selected, &mut values)
        == Some(ScalarValue::Boolean(true))
}

pub(super) fn call_entry_contexts_prove_boolean_contract_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let operators = &facts.operators;
    let semantic = &facts.semantic;
    let Some(call_site) = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_parameters) =
        validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };

    if call_guarantees::proves(
        program,
        facts,
        state_flow,
        call_flow,
        entry_contexts,
        expression,
        call_frames,
    ) {
        return true;
    }

    // Semantic place observations share one predicate owner. Ordinary calls
    // resolve the subject at invocation; captured callers supply the selected
    // operand's own place and contexts to the same query.
    if let Some((predicate, subject)) = place_predicate(program, expression)
        && let Some((value, remaining)) =
            case_test_subject_actual(program, &call_site, target_parameters, subject)
        && let Some(mut place) = validation::canonical_place_from_expression_in_state(
            program,
            state_flow.state_symbol,
            call_flow.statement_index,
            value,
        )
    {
        place.segments.extend(remaining);
        if predicate.proven(
            program,
            facts,
            state_flow,
            call_flow,
            entry_contexts,
            &place,
        ) {
            return true;
        }
    }

    let field_proof = field_actuals::proves(
        program,
        semantic,
        entry_contexts,
        state_flow.state_symbol,
        call_flow.statement_index,
        &call_site,
        target_parameters,
        expression,
    );
    if field_proof == Some(true) {
        return true;
    }
    (field_proof.is_none()
        && entry_contexts.iter().any(|entry_context| {
            let context = semantic.contexts.get(*entry_context);
            semantic_context_proves_instantiated_boolean_expression(
                program,
                facts,
                call_frames,
                context,
                entry_contexts,
                state_flow.state_symbol,
                call_flow.statement_index,
                &call_site,
                target_parameters,
                call_flow.target_symbol,
                expression,
            )
        }))
        || call_site_proves_boolean_contract_expression(
            program,
            operators,
            state_flow,
            call_flow,
            &call_site,
            call_flow.target_symbol,
            target_parameters,
            expression,
        )
}

pub(super) fn semantic_contexts_prove_contract_fact(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    entry_contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
) -> bool {
    if !contract_membership_place_is_accessible(program, semantic, entry_contexts, fact) {
        return false;
    }
    match fact.payload {
        facts::domain_membership_symbol_and_instance_pattern!(domain_symbol, semantic_domain) => {
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            if indexed_membership(program, fact.payload) {
                // Indexed applications are invariant. In particular, neither
                // declaration implication nor a scalar value alone supplies
                // equality of their indices. Flow has already substituted
                // exact caller subjects and refreshed specialized identities.
                return semantic_domain.is_valid() && entry_contexts.iter().any(|entry_context| {
                    semantic.context_view(semantic.contexts.get(*entry_context)).facts().any(|candidate| {
                        matches!(candidate.payload,
                            FactPayload::DomainMembership { domain_symbol: candidate_domain, semantic_domain: candidate_instance, .. }
                            | FactPayload::ContractDomainMembership { domain_symbol: candidate_domain, semantic_domain: candidate_instance, .. }
                            if candidate_domain == domain_symbol && candidate_instance == semantic_domain)
                            && matches!(candidate.place, FactPlace::Place(candidate_place)
                                if semantic.places_match(program, candidate_place, place))
                    })
                });
            }
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic
                    .context_view(context)
                    .proves_place_domain_membership_in_program(program, place, domain_symbol)
                    || semantic.context_view(context).facts().any(|candidate| {
                        let candidate_domain = match candidate.payload {
                            facts::domain_membership_symbol_pattern!(domain_symbol) => {
                                domain_symbol
                            }
                            _ => return false,
                        };
                        let Some(candidate_place) = candidate.place.as_place() else {
                            return false;
                        };
                        validation::field_domains::domain_membership_implies(
                            program,
                            candidate_domain,
                            domain_symbol,
                        ) && (semantic.places_match(program, candidate_place, place)
                            || place_covers_subject(program, semantic, candidate_place, place))
                    })
            }) || assigned_values::prove_domain(
                program,
                semantic,
                entry_contexts,
                place,
                domain_symbol,
            )
        }
        facts::carry_permission_pair_pattern!(permission, ..) => {
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic.context_view(context).facts().any(|candidate| {
                    let Some(candidate_permission) = candidate.payload.carry_permission() else {
                        return false;
                    };
                    let Some(candidate_place) = candidate.place.as_place() else {
                        return false;
                    };
                    candidate_permission == permission
                        && semantic.places_match(program, candidate_place, place)
                })
            })
        }
        FactPayload::BooleanExpression(expression)
        | FactPayload::ContractBooleanExpression { expression, .. } => {
            matches!(
                program.expression_table.expression(expression),
                checked_trees::expression::ExpressionNode::Boolean(true)
            ) || entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic_context_proves_boolean_expression(program, semantic, context, expression)
            })
        }
        FactPayload::PropositionApplication { .. }
        | FactPayload::ContractPropositionApplication { .. } => {
            let Some(required_label) = semantic.proposition_fact_label(program, fact) else {
                return false;
            };
            contexts_prove_proposition_label(semantic, program, entry_contexts, &required_label)
        }
        FactPayload::CarryOrigin { .. } => {
            let Some(place) = fact.place.as_place() else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                semantic
                    .context_view(semantic.contexts.get(*entry_context))
                    .facts()
                    .any(|candidate| {
                        matches!(candidate.payload, FactPayload::CarryOrigin { .. })
                            && matches!(candidate.place, FactPlace::Place(candidate_place)
                            if semantic.places_match(program, candidate_place, place))
                    })
            })
        }
        // These are evidence or deferred obligations, not propositions this
        // dispatcher can establish. An unfamiliar payload is never success.
        FactPayload::BooleanPredicate { .. }
        | FactPayload::AssignedCase { .. }
        | FactPayload::AssignedValue { .. }
        | FactPayload::AssignedIntegerBounds { .. }
        | FactPayload::FloatPredicate { .. }
        | FactPayload::AssignedScalarValue { .. }
        | FactPayload::StorageDependency { .. }
        | FactPayload::BytePredicate { .. }
        | FactPayload::BooleanValue { .. }
        | FactPayload::MatchPattern { .. }
        | FactPayload::TypeConstraint { .. }
        | FactPayload::ProofObligation { .. }
        | FactPayload::Contract { .. } => false,
    }
}

/// Explicit call membership requirements consume their projected subject.
/// Nominal field contracts are checked separately and retain conditional paths
/// below a whole-sum actual instead of extracting all its alternatives.
pub(super) fn contract_membership_place_is_accessible(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
) -> bool {
    if !matches!(
        fact.payload,
        FactPayload::DomainMembership { .. } | FactPayload::ContractDomainMembership { .. }
    ) {
        return true;
    }
    let facts::ProgramPoint::CallRequires {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = fact.point
    else {
        return true;
    };
    let Some(place) = fact.place.as_place() else {
        return false;
    };
    let Some(subject) = crate::flow::canonical_place_from_semantic_place(
        program,
        semantic,
        semantic.places.get(place),
    ) else {
        return false;
    };
    crate::flow::place_cases_are_selected(
        program,
        semantic,
        contexts,
        machine_symbol,
        state_symbol,
        statement_index,
        &subject,
    )
}

/// Whether a live fact at `candidate` covers the required `subject`
/// elementwise: same place root, and at every position either equal segments
/// or a containing `FixedRange` (the whole-extent `0..usize::MAX` row covers
/// a runtime `Index`; a finite range covers contained indices -- see
/// `assigned_values::segments_cover_subject`).
fn place_covers_subject(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    candidate: facts::PlaceHandle,
    subject: facts::PlaceHandle,
) -> bool {
    let candidate = semantic.places.get(candidate);
    let subject = semantic.places.get(subject);
    candidate.root == subject.root
        && assigned_values::segments_cover_subject(
            program,
            semantic.place_segments.span_or_empty(candidate.segments),
            semantic.place_segments.span_or_empty(subject.segments),
        )
}

pub(super) fn indexed_membership(
    program: &dyn ::validation::CheckingInput,
    payload: FactPayload,
) -> bool {
    let (facts::domain_membership_symbol_pattern!(domain_symbol)) = payload else {
        return false;
    };
    program.domain_definitions().iter().any(|domain| {
        domain.symbol == domain_symbol
            && !typed_trees::domain::index_parameters(program, domain).is_empty()
    })
}

/// The caller actual a case test's subject names: the positional argument for
/// an ordinary formal, or the receiver for `self`.
fn case_test_subject_actual(
    program: &dyn ::validation::CheckingInput,
    call_site: &validation::CallSite<'_>,
    parameters: &[typed_trees::signature::StateParameter],
    subject: typed_trees::expression::ExpressionHandle,
) -> Option<(
    typed_trees::expression::ExpressionHandle,
    Vec<facts::PlaceSegment>,
)> {
    crate::semantic::places::call_contract_argument_projection(
        program,
        parameters,
        validation::call_site_argument_expressions(program, call_site),
        subject,
    )
}

/// Whether any selected context proves `required_label` — directly, or through
/// a retained boolean fact whose label matches the `boolean:` suffix form.
/// Call `requires` resolution uses the same admission as the contract prover.
pub(crate) fn contexts_prove_proposition_label(
    semantic: &facts::FactPlan,
    program: &dyn ::validation::CheckingInput,
    contexts: &[facts::FactContextHandle],
    required_label: &str,
) -> bool {
    contexts.iter().any(|context| {
        let context = semantic.contexts.get(*context);
        semantic
            .context_view(context)
            .proves_proposition_label(program, required_label)
            || required_label
                .strip_prefix("boolean:")
                .is_some_and(|required_boolean| {
                    semantic.context_view(context).facts().any(|candidate| {
                        semantic
                            .boolean_fact_label(program, candidate)
                            .is_some_and(|label| label == required_boolean)
                    })
                })
    })
}
