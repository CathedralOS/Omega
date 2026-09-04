use psi_checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use psi_diagnostics::Diagnostic;
use psi_facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionNode;
use psi_typed_trees::statement::StatementNode;

use super::places::expression_is_boolean_place_like;
use super::prover::{
    call_entry_contexts_prove_boolean_contract_expression, semantic_contexts_prove_contract_fact,
};
use crate::labels::{
    call_target_label, canonical_place_label_from_parts, joined_place_label, machine_name,
    semantic_fact_requirement_label, symbol_name,
};

pub(super) fn check_call_requires(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The state-parameter domain facts (origin StateParameterDomain, surfaced at
    // ProgramPoint::State by append_state_parameter_domain_facts) are folded into
    // the state entry and reach every call -- INCLUDING guarded-transition
    // fallthrough arms -- through the flow's context threading, now that a
    // transition's branch-taken exit context is no longer leaked onto its sibling
    // fallthrough (see flow/statements.rs). No direct-context consultation needed.
    let entry_contexts: Vec<_> = facts
        .flow
        .state_call_entry_semantic_contexts(
            state_flow,
            call_flow.statement_index,
            call_flow.call_ordinal,
            call_flow.target_symbol,
            call_flow.receiver_symbol,
        )
        .collect();
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = match fact.payload {
                FactPayload::ContractBooleanExpression { expression, .. } => {
                    if expression_is_boolean_place_like(program, expression) {
                        semantic_contexts_prove_contract_fact(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            fact,
                        )
                    } else {
                        call_entry_contexts_prove_boolean_contract_expression(
                            program,
                            &facts.semantic,
                            state_flow,
                            call_flow,
                            &entry_contexts,
                            expression,
                        )
                        // R1: a DOMINATING incoming-arm guard establishes a
                        // boolean requires fact -- the ranges machinery's
                        // IncomingGuard walk-back is the soundness gate
                        // (single-predecessor edges, rewrite-fenced across
                        // intermediate states), and the caller state's OWN
                        // statements must preserve the named fields up to
                        // the call (conservative whole-state scan).
                        || incoming_guard_proves_requires(
                            program,
                            state_flow,
                            call_flow,
                            expression,
                            incoming_guards,
                        )
                    }
                }
                _ => semantic_contexts_prove_contract_fact(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    fact,
                ),
            };
            // ch8 construction-grant: a string literal whose compile-time bytes
            // satisfy a domain's declared byte-predicate fact grants that
            // domain without a validating boundary call -- this is how a literal
            // flows into a `&[u8] in Utf8` target.
            let satisfied = satisfied
                || transition_guard_proves_requires(program, facts, state_flow, call_flow, fact)
                || string_literal_grants_domain(program, &facts.semantic, fact.payload, fact.place)
                || value_call_return_domain_grants(
                    program,
                    &facts.semantic,
                    fact.payload,
                    fact.place,
                )
                || subslice_grants_domain(
                    program,
                    facts,
                    &entry_contexts,
                    fact.payload,
                    fact.place,
                )
                || parameter_domain_grants(program, facts, state_flow, fact.payload, fact.place);

            if !satisfied {
                let detail = match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            unreachable!("contract domain membership already handled above")
                        };
                        explain_domain_requirement_failure(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            place,
                            domain_symbol,
                        )
                    }
                    FactPayload::ContractBooleanExpression { expression, .. } => {
                        explain_missing_boolean_fact(program, expression)
                    }
                    _ => None,
                };
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove requires contract for call {} from {}: {}{}",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                    detail
                        .map(|message| format!(" ({message})"))
                        .unwrap_or_default()
                )));
            }
        }
    }
}

/// ch8 construction-grant: a string literal grants a domain `D` -- satisfying a
/// `requires <arg> in D` membership without a validating boundary call -- iff
/// `D`'s sole fact is a recognized comptime byte-predicate over `self`
/// (`valid_utf8`/`no_nul`/`ascii_only`) AND that predicate holds for the
/// literal's compile-time bytes. Reuses the shared comptime byte-predicate
/// primitives (`super::grants`); the subject must be the literal itself (an
/// expression-rooted place with no field/index segments), not a derived place.
/// A subslice `base[a..b]` satisfies a `requires <arg> in D` domain obligation
/// when D's byte-predicate fact is SUBSLICE-PRESERVING (such as
/// `no_nul`/`ascii_only`) and the `base` is itself provably in a domain implying
/// D. Sound because a contiguous subslice's bytes are a subset of the whole's, so
/// any per-byte character class the whole satisfies, the subslice does too. This
/// is the subslice analog of the concat-domain grant (`value_proves_domain`);
/// `base`'s membership is matched against the entry-context domain facts exactly
/// as the concat/param discharge does. (Correctly EXCLUDES `valid_utf8` — a
/// subslice can cut a multi-byte scalar — and `non_empty` — `base[a..a]` is empty.)
fn subslice_grants_domain(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    entry_contexts: &[psi_facts::FactContextHandle],
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return false;
    };
    let FactPlace::Place(place_handle) = place else {
        return false;
    };
    if !crate::field_domain::domain_is_subslice_preserving(program, domain_symbol) {
        return false;
    }
    // A subslice place is `base` followed by a trailing `Index` segment whose
    // expression is a Range (`base[a..b]`). The base = root + the preceding
    // segments.
    let resolved = *facts.semantic.places.get(place_handle);
    let segments: Vec<PlaceSegment> = facts
        .semantic
        .place_segments
        .span_or_empty(resolved.segments)
        .to_vec();
    let Some((PlaceSegment::Index { expression }, base_segments)) = segments.split_last() else {
        return false;
    };
    if !matches!(
        program.expression_table.expression(*expression),
        ExpressionNode::Range(_)
    ) {
        return false;
    }
    // The BASE place (root + base_segments) must be provably in a domain implying
    // `domain_symbol` -- match it against the entry-context domain facts (the same
    // discharge `value_proves_domain` uses for a domained param/field carried in).
    entry_contexts.iter().any(|&context_handle| {
        let context = facts.semantic.contexts.get(context_handle);
        facts.semantic.context_view(context).facts().any(|fact| {
            let fact_domain = match fact.payload {
                FactPayload::DomainMembership { domain_symbol, .. }
                | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
                _ => return false,
            };
            if !facts.semantic.domain_implies(fact_domain, domain_symbol)
                && !crate::field_domain::domain_membership_implies(
                    program,
                    fact_domain,
                    domain_symbol,
                )
            {
                return false;
            }
            let FactPlace::Place(fact_place) = fact.place else {
                return false;
            };
            let base = facts.semantic.places.get(fact_place);
            base.root == resolved.root
                && facts.semantic.place_segments.span_or_empty(base.segments) == base_segments
        })
    })
}

/// A SHARED (`&`, non-`mut`) parameter's DECLARED domain is invariant for the
/// state's lifetime: any interleaved call receives that parameter as a shared
/// borrow (or not at all) and cannot mutate its bytes -- Omega's
/// shared-XOR-mutable borrow discipline keeps any aliased backing frozen while
/// the shared borrow is live -- so the parameter's declared domain still holds
/// EVEN after an interleaved `&mut self` call whose conservative fact
/// invalidation dropped the flow-tracked `<param> in <declared>` fact (e.g. an
/// empty-mutation-summary helper that reduces to the blunt "wipe every context"
/// path). A `requires <param> in D` obligation is therefore satisfied whenever
/// the parameter's declared domain implies `D`. This is the same trust basis as
/// a declared param/return domain at a use site (`value_call_return_domain_grants`);
/// the subject must be the PARAMETER ITSELF (a symbol-rooted place with no
/// derived segments -- a derived place `param.field`/`param[i]` may carry a
/// different domain). Restricted to a non-mutable, non-self parameter: a
/// `&mut`-borrowed parameter's bytes CAN change, so its domain is not invariant.
fn parameter_domain_grants(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return false;
    };
    let FactPlace::Place(place_handle) = place else {
        return false;
    };
    let resolved = *facts.semantic.places.get(place_handle);
    if !facts
        .semantic
        .place_segments
        .span_or_empty(resolved.segments)
        .is_empty()
    {
        return false;
    }
    let PlaceRoot::Symbol(root_symbol) = resolved.root else {
        return false;
    };
    let Some(state) = crate::find_state(program, state_flow.state_symbol) else {
        return false;
    };
    let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root_symbol)
    else {
        return false;
    };
    if parameter.is_mutable || parameter.is_self {
        return false;
    }
    crate::field_domain::domain_constraint_symbols(program, parameter.type_reference)
        .into_iter()
        .any(|param_domain| {
            facts.semantic.domain_implies(param_domain, domain_symbol)
                || crate::field_domain::domain_membership_implies(
                    program,
                    param_domain,
                    domain_symbol,
                )
        })
}

fn string_literal_grants_domain(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return false;
    };
    let FactPlace::Place(place_handle) = place else {
        return false;
    };
    let resolved = semantic.places.get(place_handle);
    if !resolved.segments.is_empty() {
        return false;
    }
    let PlaceRoot::Expression(expression) = resolved.root else {
        return false;
    };
    crate::field_domain::string_literal_expression_grants_domain(program, expression, domain_symbol)
}

/// #66 return-domain forwarding: a `requires <arg> in D` obligation whose argument
/// is a VALUE CALL (`self.direction_command(direction)`) is satisfied when the
/// callee's DECLARED return type carries a domain implying `D`. This is the same
/// trust basis as a declared param domain at a call site (the signature's domain
/// is trusted at use sites; the callee's return body is enforced separately, the
/// deferred returns-domain check) -- and the call-argument analog of the field
/// write that already trusts a declared return domain (checks/contracts/writes.rs
/// `value_call_return_domain_implies`). The subject must be the call expression
/// itself (an expression-rooted place with no field/index segments).
fn value_call_return_domain_grants(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return false;
    };
    let FactPlace::Place(place_handle) = place else {
        return false;
    };
    let resolved = semantic.places.get(place_handle);
    if !resolved.segments.is_empty() {
        return false;
    }
    let PlaceRoot::Expression(expression) = resolved.root else {
        return false;
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some(target) = crate::find_state(program, call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    crate::field_domain::predicate_domain_constraint_symbols(program, target.return_type)
        .into_iter()
        .any(|return_domain| {
            semantic.domain_implies(return_domain, domain_symbol)
                || crate::field_domain::domain_membership_implies(
                    program,
                    return_domain,
                    domain_symbol,
                )
        })
}

/// Clear "needs fact X here" guidance for a proof-backed operator/contract that
/// is missing a required boolean fact (for example an index bound or a
/// domain-sensitive operator precondition). The caller has not established the
/// fact in the entry context, so name exactly what must hold before the call.
fn explain_missing_boolean_fact(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    let fact = program.expression_table.display_name(expression);
    if fact.is_empty() {
        return None;
    }
    Some(format!(
        "needs fact `{fact}` here; establish it before the call via a prior \
         contract guarantee, domain membership, or guard"
    ))
}

fn explain_domain_requirement_failure(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    required_place: psi_facts::PlaceHandle,
    required_domain: SymbolHandle,
) -> Option<String> {
    let mut detail = None;
    for invalidation in facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
    {
        let fact = facts.semantic.facts.get(invalidation.fact);
        let (fact_domain, fact_place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    continue;
                };
                (domain_symbol, place)
            }
            _ => continue,
        };

        if (!facts.semantic.domain_implies(fact_domain, required_domain)
            && !crate::field_domain::domain_membership_implies(
                program,
                fact_domain,
                required_domain,
            ))
            || !facts
                .semantic
                .places_match(program, fact_place, required_place)
        {
            continue;
        }

        let fact_place = facts.semantic.places.get(fact_place);
        let dependency_segments = facts
            .flow
            .invalidations
            .segments
            .span_or_empty(invalidation.dependency_segments);
        let invalidated =
            joined_place_label(program, &facts.semantic, fact_place, dependency_segments);
        let mutated = canonical_place_label_from_parts(
            program,
            invalidation.mutated_root,
            facts
                .flow
                .invalidations
                .segments
                .span_or_empty(invalidation.mutated_segments),
        );
        detail = Some(format!(
            "invalidated by prior mutation of {mutated}; {invalidated} is part of {}",
            symbol_name(program, required_domain)
        ));
    }

    detail
}

/// A caller-state incoming guard (non-negated, dominating, rewrite-fenced by
/// the ranges walk-back) whose spelling matches the requires expression --
/// exactly (`self.a <= self.b`), as an `&&` conjunct, or through the
/// multi-arm `(subject) == true` desugar. The caller state itself must also
/// preserve every field the expression names (whole-state: any assignment
/// mentioning one, or any call statement, defeats the route).
fn incoming_guard_proves_requires(
    program: &psi_typed_trees::TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: psi_typed_trees::expression::ExpressionHandle,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return false;
    };
    let Some(call_site) = crate::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_parameters) = crate::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };
    let required_label = super::labels::instantiate_call_contract_expression_label(
        program,
        state_flow.state_symbol,
        call_flow.statement_index,
        &call_site,
        target_parameters,
        expression,
    );
    let guard_matches = incoming
        .iter()
        .filter(|guard| guard.holds_at(state_flow.state_symbol))
        .any(|guard| {
            guard_conjunct_matches(program, guard.guard(), &required_label)
                || guard.direct_arguments().is_some_and(|arguments| {
                    let instantiated = instantiate_state_parameter_label(
                        program,
                        state,
                        arguments,
                        &required_label,
                    );
                    guard_conjunct_matches(program, guard.guard(), &instantiated)
                })
        });
    if !guard_matches {
        return false;
    }
    let mut fields: Vec<psi_typed_trees::name::Identifier> = Vec::new();
    collect_expression_self_fields(program, expression, &mut fields);
    fields
        .iter()
        .all(|field| caller_state_preserves_field(program, state, field))
}

/// Rebind a contract label already instantiated in `state` through the
/// immediate incoming transition arguments. Contract comparison is currently
/// label-based, so substitute only whole, unqualified parameter identifiers:
/// a field token in `self.count` must not be mistaken for the state parameter
/// `count`.
fn instantiate_state_parameter_label(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    arguments: psi_arena::HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
    label: &str,
) -> String {
    let arguments = program.statement_table.expression_handles(arguments);
    let mut replacements: Vec<(&str, String)> = Vec::new();
    let mut argument_index = 0usize;
    for parameter in program.state_parameters(state) {
        if parameter.is_self {
            continue;
        }
        let Some(argument) = arguments.get(argument_index) else {
            break;
        };
        replacements.push((
            parameter.name.as_str(),
            program.expression_table.display_name(*argument),
        ));
        argument_index = argument_index.saturating_add(1);
    }
    replace_unqualified_identifiers(label, &replacements)
}

fn replace_unqualified_identifiers(label: &str, replacements: &[(&str, String)]) -> String {
    let mut result = String::with_capacity(label.len());
    let mut cursor = 0usize;
    while cursor < label.len() {
        let Some(ch) = label[cursor..].chars().next() else {
            break;
        };
        if ch == '_' || ch.is_alphabetic() {
            let start = cursor;
            cursor += ch.len_utf8();
            while cursor < label.len() {
                let Some(next) = label[cursor..].chars().next() else {
                    break;
                };
                if next == '_' || next.is_alphanumeric() {
                    cursor += next.len_utf8();
                } else {
                    break;
                }
            }
            let identifier = &label[start..cursor];
            let qualified =
                start > 0 && matches!(label.as_bytes().get(start - 1), Some(b'.' | b':'));
            if !qualified
                && let Some((_, replacement)) =
                    replacements.iter().find(|(name, _)| *name == identifier)
            {
                result.push_str(replacement);
            } else {
                result.push_str(identifier);
            }
        } else {
            result.push(ch);
            cursor += ch.len_utf8();
        }
    }
    result
}

/// A named transition is itself an incoming edge. Its taken-arm guard may
/// establish the target state's arrival requirement after positional
/// substitution (`value > 0` becomes `self.value > 0`). Ordinary call entry
/// contexts are statement-entry facts and therefore deliberately do not assume
/// that guard; discharge it explicitly for the transition target only.
fn transition_guard_proves_requires(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &psi_facts::Fact,
) -> bool {
    let Some(call_site) = crate::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    if !matches!(call_site, crate::CallSite::TransitionNamed { .. }) {
        return false;
    }
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
    let Some(caller_state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return false;
    };
    let Some(StatementNode::Transition(transition)) = program
        .statement_table
        .statements(caller_state.statement_nodes)
        .get(call_flow.statement_index)
    else {
        return false;
    };
    let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        return false;
    };
    let Some(target_parameters) = crate::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };
    let required_label = match fact.payload {
        FactPayload::ContractBooleanExpression { expression, .. } => {
            super::labels::instantiate_call_contract_expression_label(
                program,
                state_flow.state_symbol,
                call_flow.statement_index,
                &call_site,
                target_parameters,
                expression,
            )
        }
        FactPayload::ContractDomainMembership { .. } => {
            semantic_fact_requirement_label(program, &facts.semantic, fact)
        }
        _ => return false,
    };
    guard_conjunct_matches(program, guard, &required_label)
}

pub(super) fn guard_conjunct_matches(
    program: &psi_typed_trees::TypedTrees,
    guard: psi_typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(guard) == required_label {
        return true;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return false;
    };
    match binary.operator {
        psi_typed_trees::expression::BinaryOperator::And => {
            guard_conjunct_matches(program, binary.left, required_label)
                || guard_conjunct_matches(program, binary.right, required_label)
        }
        psi_typed_trees::expression::BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_conjunct_matches(program, binary.left, required_label)
        }
        _ => false,
    }
}

fn collect_expression_self_fields(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
    fields: &mut Vec<psi_typed_trees::name::Identifier>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            fields.push(member.member.clone());
            collect_expression_self_fields(program, member.receiver, fields);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_self_fields(program, binary.left, fields);
            collect_expression_self_fields(program, binary.right, fields);
        }
        ExpressionNode::Borrow(inner) => {
            collect_expression_self_fields(program, inner.target, fields)
        }
        ExpressionNode::Cast(cast) => collect_expression_self_fields(program, cast.value, fields),
        _ => {}
    }
}

fn caller_state_preserves_field(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    field: &psi_typed_trees::name::Identifier,
) -> bool {
    use psi_typed_trees::statement::StatementNode;
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::Assignment(assignment) = statement
            && assignment_target_mentions_field(program, assignment.target, field)
        {
            return false;
        }
    }
    true
}

fn assignment_target_mentions_field(
    program: &psi_typed_trees::TypedTrees,
    target: psi_typed_trees::expression::ExpressionHandle,
    field: &psi_typed_trees::name::Identifier,
) -> bool {
    if !target.is_valid() {
        return false;
    }
    match program.expression_table.expression(target) {
        ExpressionNode::Member(member) => {
            member.member.as_str() == field.as_str()
                || assignment_target_mentions_field(program, member.receiver, field)
        }
        ExpressionNode::Borrow(inner) => {
            assignment_target_mentions_field(program, inner.target, field)
        }
        ExpressionNode::Indexed(indexed) => {
            assignment_target_mentions_field(program, indexed.collection, field)
        }
        _ => false,
    }
}
