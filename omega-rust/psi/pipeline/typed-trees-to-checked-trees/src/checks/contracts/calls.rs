use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use language_core::is_self_receiver;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

use super::places::expression_is_boolean_place_like;
use super::prover::{
    call_entry_contexts_prove_boolean_contract_expression, closed_boolean_value,
    semantic_contexts_prove_contract_fact,
};
use crate::labels::{
    call_target_label, joined_place_label, machine_name, semantic_fact_requirement_label,
    symbol_name,
};

pub(super) fn check_call_requires(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    nominal_requirements: &super::nominal_inputs::DeclaredFieldRequirements,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
    super::nominal_inputs::check(
        program,
        facts,
        state_flow,
        call_flow,
        nominal_requirements,
        &entry_contexts,
        call_frames,
        diagnostics,
    );
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = match fact.payload {
                FactPayload::ContractBooleanExpression { expression, .. } => {
                    if std::env::var_os("OMEGA_DEBUG_ROUTES").is_some() {
                        eprintln!(
                            "ROUTE target={} closed={} proves={} entry_ctx={} in_ctx={} structural={}",
                            crate::labels::symbol_name(program, call_flow.target_symbol),
                            closed_boolean_value(program, &facts.operators, expression)
                                == Some(true),
                            super::call_bounds::proves(
                                program, facts, state_flow, call_flow, expression
                            ),
                            call_entry_contexts_prove_boolean_contract_expression(
                                program,
                                facts,
                                state_flow,
                                call_flow,
                                &entry_contexts,
                                expression,
                                call_frames
                            ),
                            super::call_bounds::proves_in_context(
                                program,
                                facts,
                                state_flow,
                                call_flow,
                                &entry_contexts,
                                expression,
                                call_frames
                            ),
                            super::entailment::structural_call_requirement(
                                program,
                                facts,
                                state_flow,
                                call_flow,
                                expression,
                                call_frames
                            ),
                        );
                    }
                    closed_boolean_value(program, &facts.operators, expression) == Some(true)
                        || super::call_bounds::proves(
                            program, facts, state_flow, call_flow, expression,
                        )
                        || call_entry_contexts_prove_boolean_contract_expression(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            &entry_contexts,
                            expression,
                            call_frames,
                        )
                        || super::call_bounds::proves_in_context(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            &entry_contexts,
                            expression,
                            call_frames,
                        )
                        || super::entailment::structural_call_requirement(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            expression,
                            call_frames,
                        )
                        || if !expression_is_boolean_place_like(program, expression) {
                            // R1: a DOMINATING incoming-arm guard establishes a
                            // boolean requires fact -- the ranges machinery's
                            // IncomingGuard walk-back is the soundness gate
                            // (single-predecessor edges, rewrite-fenced across
                            // intermediate states), and the caller state's OWN
                            // statements must preserve the named fields up to
                            // the call (conservative whole-state scan).
                            incoming_guard_proves_requires(
                                program,
                                facts,
                                state_flow,
                                call_flow,
                                expression,
                                incoming_guards,
                                call_frames,
                            )
                        } else {
                            // The actual-aware proof above already checked
                            // this formal. A raw callee name is not a caller
                            // fact, even when both parameters spell `flag`.
                            false
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
                || (!super::prover::indexed_membership(program, fact.payload)
                    && (transition_guard_proves_requires(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        fact,
                        call_frames,
                    ) || string_literal_grants_domain(
                        program,
                        &facts.semantic,
                        fact.payload,
                        fact.place,
                    ) || value_call_return_domain_grants(
                        program,
                        &facts.semantic,
                        fact.payload,
                        fact.place,
                    ) || subslice_grants_domain(
                        program,
                        facts,
                        &entry_contexts,
                        fact.payload,
                        fact.place,
                    ) || parameter_domain_grants(
                        program,
                        facts,
                        state_flow,
                        fact.payload,
                        fact.place,
                    ) || super::reference_domains::proves(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        &entry_contexts,
                        fact,
                        call_frames,
                    )));

            let satisfied = satisfied
                && super::prover::contract_membership_place_is_accessible(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    fact,
                );
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
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    entry_contexts: &[facts::FactContextHandle],
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return false;
    };
    let FactPlace::Place(place_handle) = place else {
        return false;
    };
    if !crate::facts::field_domain::domain_is_subslice_preserving(program, domain_symbol) {
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
                && !crate::facts::field_domain::domain_membership_implies(
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
    program: &typed_trees::TypedTrees,
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
    let Some(state) = crate::semantic::calls::find_state(program, state_flow.state_symbol) else {
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
    crate::facts::field_domain::domain_constraint_symbols(program, parameter.type_reference)
        .into_iter()
        .any(|param_domain| {
            facts.semantic.domain_implies(param_domain, domain_symbol)
                || crate::facts::field_domain::domain_membership_implies(
                    program,
                    param_domain,
                    domain_symbol,
                )
        })
}

fn string_literal_grants_domain(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
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
    crate::facts::field_domain::string_literal_expression_grants_domain(
        program,
        expression,
        domain_symbol,
    )
}

/// #66 return-domain forwarding: a `requires <arg> in D` obligation whose argument
/// is a VALUE CALL (`self.direction_command(direction)`) is satisfied when the
/// callee's DECLARED return type carries a domain implying `D`. This is the same
/// trust basis as a declared param domain at a call site (the signature's domain
/// is trusted at use sites; scalar result domains are enforced independently at
/// ordinary exits in exits/result_domains.rs) -- and the call-argument analog
/// of the field write that trusts a declared return domain (checks/contracts/writes.rs
/// `value_call_return_domain_implies`). The subject must be the call expression
/// itself (an expression-rooted place with no field/index segments).
fn value_call_return_domain_grants(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
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
    let Some(target) = crate::semantic::calls::find_state(program, call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    crate::facts::field_domain::predicate_domain_constraint_symbols(program, target.return_type)
        .into_iter()
        .any(|return_domain| {
            semantic.domain_implies(return_domain, domain_symbol)
                || crate::facts::field_domain::domain_membership_implies(
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
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
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
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    required_place: facts::PlaceHandle,
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
            && !crate::facts::field_domain::domain_membership_implies(
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
        let mutated = facts::canonical_place_label_from_parts(
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
/// mentioning one, or any call statement, defeats the route) and every
/// unqualified operand name the instantiated requirement spells (a rebinding
/// such as `limit = 0` leaves the label match quoting a stale premise), and
/// the jump's own earlier operands must leave the requirement's reads
/// unwritten (`guard_operands`).
fn incoming_guard_proves_requires(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_flow.state_symbol)
    else {
        return false;
    };
    let Some(call_site) = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_parameters) =
        crate::semantic::calls::call_target_parameters(program, call_flow.target_symbol)
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
    let mut fields: Vec<typed_trees::name::Identifier> = Vec::new();
    collect_expression_self_fields(program, expression, &mut fields);
    fields
        .iter()
        .all(|field| caller_state_preserves_field(program, state, field))
        && caller_state_preserves_label_names(
            program,
            facts,
            state_flow,
            call_flow,
            state,
            &required_label,
        )
        // The statement scans above stop at the call; the jump's own operands
        // run after them and before the requirement's read.
        && super::guard_operands::requirement_reads_survive_earlier_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            crate::semantic::calls::call_site_argument_expressions(program, &call_site),
            target_parameters,
            &super::guard_operands::boolean_requirement_mentions(program, expression),
            call_frames,
        )
}

/// The guard match above compares display labels, so it stands only while
/// every unqualified name the instantiated requirement spells still refers to
/// the caller value the guard was evaluated against. Rebinding such a name
/// before the call -- `limit = 0`, a shadowing `let`, or a mutation through a
/// call's write frame (recorded as a pre-call invalidation) -- makes the label
/// quote a stale premise, and this route must refuse; the remaining provers
/// may still establish the fact from current evidence. Qualified `self.field`
/// operands are covered by the field walk above, so `self` itself is skipped.
fn caller_state_preserves_label_names(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    state: &typed_trees::state::State,
    required_label: &str,
) -> bool {
    let names = unqualified_label_identifiers(required_label);
    if names.is_empty() {
        return true;
    }
    let rebound_before_call = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(call_flow.statement_index)
        .any(|statement| match statement {
            StatementNode::Assignment(assignment) => names
                .iter()
                .any(|name| assignment_target_mentions_name(program, assignment.target, name)),
            StatementNode::LocalData(local) => names
                .iter()
                .any(|name| local.name.as_str() == name.as_str()),
            _ => false,
        });
    if rebound_before_call {
        return false;
    }
    !facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
        .any(|invalidation| {
            let PlaceRoot::Symbol(symbol) = invalidation.mutated_root else {
                return false;
            };
            names
                .iter()
                .any(|name| name.as_str() == symbol_name(program, symbol))
        })
}

/// Unqualified identifier tokens in a display label, skipping `self` and any
/// token reached through `.`/`::` (those are member or namespace paths, not
/// caller-local names).
fn unqualified_label_identifiers(label: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = 0usize;
    while cursor < label.len() {
        let Some(character) = label[cursor..].chars().next() else {
            break;
        };
        if character == '_' || character.is_alphabetic() {
            let start = cursor;
            cursor += character.len_utf8();
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
                && !is_self_receiver(identifier)
                && !names.iter().any(|name: &String| name == identifier)
            {
                names.push(identifier.to_owned());
            }
        } else {
            cursor += character.len_utf8();
        }
    }
    names
}

fn assignment_target_mentions_name(
    program: &typed_trees::TypedTrees,
    target: typed_trees::expression::ExpressionHandle,
    name: &str,
) -> bool {
    if !target.is_valid() {
        return false;
    }
    match program.expression_table.expression(target) {
        ExpressionNode::Name(_) => program.expression_table.display_name(target) == name,
        ExpressionNode::Member(member) => {
            assignment_target_mentions_name(program, member.receiver, name)
        }
        ExpressionNode::Borrow(inner) => {
            assignment_target_mentions_name(program, inner.target, name)
        }
        ExpressionNode::Indexed(indexed) => {
            assignment_target_mentions_name(program, indexed.collection, name)
        }
        _ => false,
    }
}

/// Rebind a contract label already instantiated in `state` through the
/// immediate incoming transition arguments. Contract comparison is currently
/// label-based, so substitute only whole, unqualified parameter identifiers:
/// a field token in `self.count` must not be mistaken for the state parameter
/// `count`.
fn instantiate_state_parameter_label(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    arguments: arena::HandleSpan<typed_trees::expression::ExpressionHandle>,
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
        let Some(character) = label[cursor..].chars().next() else {
            break;
        };
        if character == '_' || character.is_alphabetic() {
            let start = cursor;
            cursor += character.len_utf8();
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
            result.push(character);
            cursor += character.len_utf8();
        }
    }
    result
}

/// A named transition is itself an incoming edge. Its taken-arm guard may
/// establish the target state's arrival requirement after positional
/// substitution (`value > 0` becomes `self.value > 0`). Ordinary call entry
/// contexts are statement-entry facts and therefore deliberately do not assume
/// that guard; discharge it explicitly for the transition target only, and
/// only while the arm's own operands, evaluated after the guard and before
/// the jump, leave the guard's storage unwritten (`guard_operands`).
fn transition_guard_proves_requires(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(call_site) = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(machine) = crate::lookup::machine_by_symbol(program, state_flow.machine_symbol) else {
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
    let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard else {
        return false;
    };
    // The arm this guard selects, in either spelling. A bare named target is
    // one; so is `-> (callee(..))`, which `ac52bc4114` requires of an attached
    // machine and which a receiver-qualified call always uses. Only the
    // SELECTED arm's own target qualifies: a call inside the guard, nested
    // inside the target expression, or on the continuation arm -- which holds
    // the guard's negation -- is not this edge.
    let selects_this_call = match &call_site {
        crate::semantic::calls::CallSite::TransitionNamed { .. } => true,
        crate::semantic::calls::CallSite::Expression { expression, .. } => {
            matches!(
                program.statement_table.transition_target(transition.target),
                typed_trees::statement::TransitionTargetNode::Value(value) if value == expression
            )
        }
        crate::semantic::calls::CallSite::Statement(_) => false,
    };
    if !selects_this_call {
        return false;
    }
    let Some(target_parameters) =
        crate::semantic::calls::call_target_parameters(program, call_flow.target_symbol)
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
    let guard_establishes = guard_conjunct_matches(program, guard, &required_label)
        || predicate_only_domain_labels(program, facts, fact).is_some_and(|labels| {
            labels
                .iter()
                .all(|label| guard_conjunct_matches(program, guard, label))
        })
        // The spellings need not agree when both state a closed interval:
        // `index < 16` establishes `requires self <= 15`. Comparing the
        // intervals rather than the text is what lets a bound move to a domain
        // without the guard being rewritten to match the predicate.
        || predicate_only_domain_interval(program, facts, fact).is_some_and(
            |(subject_label, required_low, required_high)| {
                let contains = |(low, high): (numerics::bignum::BigInt, numerics::bignum::BigInt)| {
                    low >= required_low && high <= required_high
                };
                super::intervals::guard_interval_for_label(program, guard, &subject_label).is_some_and(contains)
                    // The subject may be an EXPRESSION rather than a name:
                    // `walk(remaining - 1)` states its bound about the
                    // subtraction, which no guard spells. Read the argument
                    // the requirement is about and shift its interval.
                    || requirement_subject_expression(facts, fact)
                        .and_then(|argument| {
                            super::intervals::expression_interval(
                                program,
                                machine,
                                caller_state,
                                &[guard],
                                argument,
                            )
                        })
                        .is_some_and(contains)
            },
        );
    if !guard_establishes {
        return false;
    }
    // The guard was read before the arm's operands ran. An operand evaluated
    // ahead of the requirement's read may have written the storage that read
    // names; the label match alone cannot see that, so the operand write
    // frames decide whether the quoted premise still describes the delivered
    // value.
    super::guard_operands::requirement_reads_survive_earlier_operand_writes(
        program,
        facts,
        state_flow,
        call_flow,
        crate::semantic::calls::call_site_argument_expressions(program, &call_site),
        target_parameters,
        &super::guard_operands::fact_requirement_mentions(program, facts, fact),
        call_frames,
    )
}

pub(super) fn guard_conjunct_matches(
    program: &typed_trees::TypedTrees,
    guard: typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(guard) == required_label {
        return true;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return false;
    };
    match binary.operator {
        typed_trees::expression::BinaryOperator::And => {
            guard_conjunct_matches(program, binary.left, required_label)
                || guard_conjunct_matches(program, binary.right, required_label)
        }
        typed_trees::expression::BinaryOperator::Equal
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
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    fields: &mut Vec<typed_trees::name::Identifier>,
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
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    field: &typed_trees::name::Identifier,
) -> bool {
    use typed_trees::statement::StatementNode;
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
    program: &typed_trees::TypedTrees,
    target: typed_trees::expression::ExpressionHandle,
    field: &typed_trees::name::Identifier,
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

#[cfg(test)]
mod prerequisite_roster_probes {
    //! Adversarial probes for the prerequisite-side helpers this file runs
    //! before an incoming guard or assignment may stand in for a proven
    //! contract clause: label tokenization never treats a qualified member or
    //! namespace path as a caller-local name (`unqualified_label_identifiers`,
    //! `replace_unqualified_identifiers`), guard conjunctions decompose `&&`
    //! and unwrap `x == true` but never `||` (`guard_conjunct_matches`), and
    //! assignment targets reach through member/index/borrow receivers only to
    //! the spelled root (`assignment_target_mentions_name`,
    //! `assignment_target_mentions_field`). Each pin names the helper it
    //! exercises.

    use super::{
        assignment_target_mentions_field, assignment_target_mentions_name, guard_conjunct_matches,
        replace_unqualified_identifiers, unqualified_label_identifiers,
    };
    use crate::tests::front_end::typed_program;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::name::Identifier;
    use typed_trees::statement::{StatementNode, TransitionGuardNode};

    const SOURCE: &str = r#"
        data Main {
            value: i64;
            flag: bool;
            primed: bool;
            cells: [u64; 4];
        }

        machine Main::main(&mut self, k: u64) -> u64 {
            self.value = 3;
            self.cells[0] = k;
            transition self.flag && self.primed {
                true -> 1
                false -> 0
            }
            state equal(&mut self) -> u64 {
                transition self.flag == true {
                    true -> 1
                    false -> 0
                }
            }
            state fork(&mut self, a: bool, b: bool) -> u64 {
                transition a || b {
                    true -> 0
                    false -> 1
                }
            }
        }
    "#;

    fn program() -> typed_trees::TypedTrees {
        typed_program(SOURCE)
    }

    fn state_statement<'program>(
        program: &'program typed_trees::TypedTrees,
        state_name: &str,
        index: usize,
    ) -> &'program StatementNode {
        // Machine names keep their qualified diagnostic spelling
        // (`Main::main`); a leaf name matches its final member.
        let machine = program
            .machines()
            .iter()
            .find(|machine| {
                let spelled = machine.name.as_str();
                spelled == "main" || spelled.ends_with("::main")
            })
            .expect("main machine");
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| {
                let spelled = state.name.as_str();
                spelled == state_name || spelled.ends_with(&format!("::{state_name}"))
            })
            .unwrap_or_else(|| panic!("state {state_name}"));
        &program.statement_table.statements(state.statement_nodes)[index]
    }

    fn state_guard(
        program: &typed_trees::TypedTrees,
        state_name: &str,
        transition_index: usize,
    ) -> ExpressionHandle {
        let StatementNode::Transition(transition) =
            state_statement(program, state_name, transition_index)
        else {
            panic!("statement {transition_index} of {state_name} is not a transition");
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            panic!("transition {transition_index} of {state_name} is unguarded");
        };
        guard
    }

    fn assignment_target(program: &typed_trees::TypedTrees, index: usize) -> ExpressionHandle {
        let StatementNode::Assignment(assignment) = state_statement(program, "main", index) else {
            panic!("statement {index} is not an assignment");
        };
        assignment.target
    }

    fn label(program: &typed_trees::TypedTrees, expression: ExpressionHandle) -> String {
        program.expression_table.display_name(expression)
    }

    /// Lowering wraps every authored guard in an implicit `== true`; the
    /// authored expression sits on its left.
    fn authored_guard(
        program: &typed_trees::TypedTrees,
        guard: ExpressionHandle,
    ) -> ExpressionHandle {
        let ExpressionNode::Binary(wrapper) = program.expression_table.expression(guard) else {
            panic!("guard is not a binary expression");
        };
        assert_eq!(wrapper.operator, BinaryOperator::Equal);
        wrapper.left
    }

    fn and_operands(
        program: &typed_trees::TypedTrees,
        guard: ExpressionHandle,
    ) -> (ExpressionHandle, ExpressionHandle) {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
            panic!("guard is not a binary expression");
        };
        assert_eq!(binary.operator, BinaryOperator::And);
        (binary.left, binary.right)
    }

    #[test]
    fn unqualified_label_identifiers_skip_qualified_member_and_namespace_tokens() {
        // `count` behind `.` is a member path; `self` itself never lists.
        assert!(unqualified_label_identifiers("self.count > 0").is_empty());
        // Bare roots list, deduplicated.
        assert_eq!(unqualified_label_identifiers("x + x"), ["x"]);
        // `Pkg` is an unqualified root; `limit` behind `::` is not.
        assert_eq!(
            unqualified_label_identifiers("Pkg::limit + count"),
            ["Pkg", "count"]
        );
        // Index payloads are ordinary unqualified names.
        assert_eq!(unqualified_label_identifiers("a.b[c]"), ["a", "c"]);
        // `_` prefixes and trailing digits stay inside the token.
        assert_eq!(
            unqualified_label_identifiers("_hidden + tail_2"),
            ["_hidden", "tail_2"]
        );
        assert!(unqualified_label_identifiers("").is_empty());
    }

    #[test]
    fn replace_unqualified_identifiers_never_rewrites_qualified_tokens() {
        let owned = |name: &str| name.to_owned();
        assert_eq!(
            replace_unqualified_identifiers("self.count + a::count", &[("count", owned("w"))]),
            "self.count + a::count"
        );
        // Whole-token matching: `country` is not `count`.
        assert_eq!(
            replace_unqualified_identifiers("count + country", &[("count", owned("w"))]),
            "w + country"
        );
        // A root occurrence rewrites while its qualified twin stays.
        assert_eq!(
            replace_unqualified_identifiers("count.count", &[("count", owned("w"))]),
            "w.count"
        );
        // `self` is an ordinary unqualified token to this helper; callers keep
        // it out of the replacement roster instead.
        assert_eq!(
            replace_unqualified_identifiers("self.value", &[("self", owned("r"))]),
            "r.value"
        );
    }

    #[test]
    fn guard_conjunct_matches_walks_and_unwraps_but_not_or() {
        let program = program();
        // Every authored guard arrives wrapped: `G` lowers to `G == true`.
        let main_guard = state_guard(&program, "main", 2);
        let authored_and = authored_guard(&program, main_guard);
        let (left, right) = and_operands(&program, authored_and);
        // The whole lowered guard is its own conjunct.
        assert!(guard_conjunct_matches(
            &program,
            main_guard,
            &label(&program, main_guard)
        ));
        // The `== true` wrapper unwraps, exposing the authored `&&`.
        assert!(guard_conjunct_matches(
            &program,
            main_guard,
            &label(&program, authored_and)
        ));
        // `&&` decomposes on either side beneath the wrapper.
        assert!(guard_conjunct_matches(
            &program,
            main_guard,
            &label(&program, left)
        ));
        assert!(guard_conjunct_matches(
            &program,
            main_guard,
            &label(&program, right)
        ));
        // `x == true` is itself unwrap-transparent: `self.flag` inside
        // `(self.flag == true) == true` is still a conjunct.
        let equal_guard = state_guard(&program, "equal", 0);
        let authored_equal = authored_guard(&program, equal_guard);
        let ExpressionNode::Binary(equality) = program.expression_table.expression(authored_equal)
        else {
            panic!("equal guard is not `flag == true`");
        };
        assert_eq!(equality.operator, BinaryOperator::Equal);
        assert!(guard_conjunct_matches(
            &program,
            equal_guard,
            &label(&program, equality.left)
        ));
        // A label present nowhere in the guard cannot match.
        assert!(!guard_conjunct_matches(
            &program,
            main_guard,
            "self.cells > 0"
        ));
        // `||` does not decompose: `a` under `a || b` is not a conjunct.
        let fork_guard = state_guard(&program, "fork", 0);
        let authored_or = authored_guard(&program, fork_guard);
        let ExpressionNode::Binary(disjunction) = program.expression_table.expression(authored_or)
        else {
            panic!("fork guard is not a binary expression");
        };
        assert_eq!(disjunction.operator, BinaryOperator::Or);
        assert!(guard_conjunct_matches(
            &program,
            fork_guard,
            &label(&program, fork_guard)
        ));
        assert!(guard_conjunct_matches(
            &program,
            fork_guard,
            &label(&program, authored_or)
        ));
        assert!(!guard_conjunct_matches(
            &program,
            fork_guard,
            &label(&program, disjunction.left)
        ));
    }

    #[test]
    fn assignment_targets_reach_only_the_spelled_root() {
        let program = program();
        // `self.value = 3`: the root `self` is mentioned; the field name is not.
        let member = assignment_target(&program, 0);
        assert!(assignment_target_mentions_name(&program, member, "self"));
        assert!(!assignment_target_mentions_name(&program, member, "value"));
        assert!(assignment_target_mentions_field(
            &program,
            member,
            &Identifier::from("value")
        ));
        assert!(!assignment_target_mentions_field(
            &program,
            member,
            &Identifier::from("cells")
        ));
        // `self.cells[0] = k`: indexing keeps the collection receiver, so the
        // root and the `cells` field are still reachable through the target.
        let indexed = assignment_target(&program, 1);
        assert!(assignment_target_mentions_name(&program, indexed, "self"));
        assert!(!assignment_target_mentions_name(&program, indexed, "0"));
        assert!(assignment_target_mentions_field(
            &program,
            indexed,
            &Identifier::from("cells")
        ));
        assert!(!assignment_target_mentions_field(
            &program,
            indexed,
            &Identifier::from("value")
        ));
    }
}

#[cfg(test)]
mod transition_arm_guard_probes {
    //! `transition_guard_proves_requires` lets a transition's taken-arm guard
    //! discharge the target's `requires`. Two things decide whether that is
    //! sound and complete: WHICH ARM the call sits on, and how the arm target
    //! is SPELLED.

    use crate::tests::front_end::checked_program_result;

    fn accepted(source: &str) -> bool {
        checked_program_result(source).is_ok()
    }

    const CALLEE: &str = "machine check(value: u64) -> u64
        requires
            value > 0
        { value }";

    /// The taken arm establishes the guard, so the arrival requirement is
    /// discharged; the CONTINUATION arm establishes its negation, so the same
    /// call there must stay rejected. Both are free machines, which keep the
    /// bare named-target spelling.
    #[test]
    fn only_the_taken_arm_discharges_the_arrival_requirement() {
        assert!(
            accepted(&format!(
                "{CALLEE}
                machine run(value: u64) -> u64 {{
                    transition value > 0 {{ true -> check(value) false -> (0) }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the taken arm's guard must discharge `requires value > 0`"
        );
        assert!(
            !accepted(&format!(
                "{CALLEE}
                machine run(value: u64) -> u64 {{
                    transition value > 0 {{ true -> (0) false -> check(value) }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the continuation arm holds `!(value > 0)`, which cannot discharge \
             `requires value > 0`"
        );
    }

    /// A RECEIVER-QUALIFIED callee always arrives as a value call, and its
    /// requirement is discharged by the same taken-arm guard. This is the case
    /// the `CallSite::TransitionNamed` gate actually blocked: `self.read(index)`
    /// on the taken arm of `transition index <= 15` could not prove
    /// `requires index <= 15`, while the identical edge to a FREE callee could,
    /// because a different route reaches that one first.
    #[test]
    fn a_receiver_qualified_taken_arm_call_discharges_its_requirement() {
        const READER: &str = "data Store { arr: [u64; 16]; }
            machine Store::read(&mut self, index: u64) -> u64
            requires
                index <= 15;
            { transition { _ -> (self.arr[index]) } }";
        assert!(
            accepted(&format!(
                "{READER}
                machine Store::scan(&mut self, index: u64) -> u64 {{
                    transition index <= 15 {{ true -> (self.read(index)) false -> (0) }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the taken arm's guard must discharge a receiver-qualified callee's requirement"
        );
        assert!(
            !accepted(&format!(
                "{READER}
                machine Store::scan(&mut self, index: u64) -> u64 {{
                    transition index <= 15 {{ true -> (0) false -> (self.read(index)) }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the continuation arm holds the guard's negation and cannot discharge it"
        );
    }

    /// A PREDICATE-ONLY domain is established by proving its predicates.
    /// wiki/spec/language/domains.md: "Predicates alone establish
    /// predicate-only membership", and a ROUTED domain is the case that
    /// "additionally needs exact authorized provenance". Both controls are the
    /// point: the routed twin carries the SAME predicate and the SAME guard and
    /// must still reject, and a two-predicate domain whose guard proves only
    /// one of them must reject, because membership is every obligation rather
    /// than any of them.
    #[test]
    fn a_predicate_only_domain_is_established_by_proving_its_predicates() {
        let program = |domain: &str| {
            format!(
                "{domain}
                data Main {{}}
                machine Main::take(&mut self, index: u64 in Slot16) -> u64 {{
                    transition {{ _ -> (0) }}
                }}
                machine Main::scan(&mut self, index: u64) -> u64 {{
                    transition index <= 15 {{ true -> (self.take(index)) false -> (0) }}
                }}
                machine Main::main(&mut self) {{}}"
            )
        };
        assert!(
            accepted(&program("domain u64::Slot16 requires self <= 15;")),
            "a guard proving the whole predicate must establish predicate-only membership"
        );
        assert!(
            !accepted(&program(
                "pub boundary trait Granter { machine grant(v: u64) -> u64 in Slot16; }
                 pub domain u64::Slot16 requires self <= 15 established by Granter::grant;"
            )),
            "a ROUTED domain still needs provenance, however well its predicate is proved"
        );
        assert!(
            !accepted(&program(
                "domain u64::Slot16 requires self <= 15 && self >= 4;"
            )),
            "a guard proving one of two predicates establishes neither membership"
        );
    }

    /// The guard and the predicate need not agree in SPELLING when both state a
    /// closed interval: `index < 16` establishes `requires self <= 15`. The
    /// control is containment — a guard admitting more than the domain does
    /// establishes nothing, however similar the two look.
    #[test]
    fn a_guard_interval_inside_the_domains_establishes_membership() {
        let program = |guard: &str| {
            format!(
                "domain u64::Slot16 requires self <= 15;
                data Main {{}}
                machine Main::take(&mut self, index: u64 in Slot16) -> u64 {{
                    transition {{ _ -> (0) }}
                }}
                machine Main::scan(&mut self, index: u64) -> u64 {{
                    transition {guard} {{ true -> (self.take(index)) false -> (0) }}
                }}
                machine Main::main(&mut self) {{}}"
            )
        };
        assert!(
            accepted(&program("index < 16")),
            "`index < 16` is the same interval as `self <= 15`"
        );
        assert!(
            accepted(&program("index <= 15 && index >= 2")),
            "a narrower guard is inside the domain's interval"
        );
        assert!(
            !accepted(&program("index < 32")),
            "a guard admitting 16..=31 does not establish `self <= 15`"
        );
        assert!(
            !accepted(&program("index >= 2")),
            "a guard with no upper bound cannot reach a bounded domain"
        );
    }

    /// A membership requirement whose subject is an EXPRESSION is discharged
    /// by reading that expression's interval: `walk(remaining - 1)` states its
    /// obligation about the subtraction, which no guard spells. The control is
    /// direction — the same shift the other way leaves the interval outside
    /// the domain and must reject.
    #[test]
    fn an_expression_argument_carries_its_shifted_interval_into_the_domain() {
        let program = |argument: &str| {
            format!(
                "domain u64::Fuel requires self <= 5;
                data Main {{}}
                machine Main::take(&mut self, value: u64 in Fuel) -> u64 {{
                    transition {{ _ -> (0) }}
                }}
                machine Main::walk(&mut self, remaining: u64 in Fuel) -> u64 {{
                    transition remaining > 0 {{
                        true -> (self.take({argument}))
                        false -> (0)
                    }}
                }}
                machine Main::main(&mut self) {{}}"
            )
        };
        assert!(
            accepted(&program("remaining - 1")),
            "`remaining` is in 0..=5 and guarded above 0, so `remaining - 1` is in 0..=4"
        );
        assert!(
            !accepted(&program("remaining + 1")),
            "`remaining + 1` reaches 6 and leaves the domain"
        );
    }

    /// The FACT route carries numeric implication -- `index < 16` discharges
    /// `index <= 15` -- where the transition-guard route only compares
    /// spellings. It reached a free callee but not a receiver-qualified one,
    /// because a call whose receiver is a runtime place was not an "ordinary
    /// call". It is: the goal is substituted THROUGH the receiver before it is
    /// compared, which the control below is what proves.
    #[test]
    fn a_receiver_qualified_call_carries_numeric_implication_to_its_own_object() {
        assert!(
            accepted(
                "data Store { arr: [u64; 16]; }
                machine Store::read(&mut self, index: u64) -> u64
                requires
                    index <= 15;
                { transition { _ -> (self.arr[index]) } }
                machine Store::scan(&mut self, index: u64) -> u64 {
                    transition index < 16 { true -> (self.read(index)) false -> (0) }
                }
                data Main {}
                machine Main::main(&mut self) {}"
            ),
            "`index < 16` must discharge `requires index <= 15` through a receiver call"
        );

        // A requirement ABOUT THE RECEIVER is substituted to the object the
        // call names, so a guard about a DIFFERENT object cannot discharge it
        // even though both spell `self.limit`.
        const PEER: &str = "data Store { limit: u64; }
            machine Store::read(&mut self) -> u64
            requires
                self.limit <= 15;
            { transition { _ -> (self.limit) } }
            data Main { limit: u64; peer: Store; }";
        assert!(
            accepted(&format!(
                "{PEER}
                machine Main::go(&mut self) -> u64 {{
                    transition self.peer.limit <= 15 {{ true -> (self.peer.read()) false -> (0) }}
                }}
                machine Main::main(&mut self) {{}}"
            )),
            "a guard naming the receiver's own field must discharge the requirement"
        );
        assert!(
            !accepted(&format!(
                "{PEER}
                machine Main::go(&mut self) -> u64 {{
                    transition self.limit <= 15 {{ true -> (self.peer.read()) false -> (0) }}
                }}
                machine Main::main(&mut self) {{}}"
            )),
            "the caller's own `self.limit` is not the receiver's, and must not discharge it"
        );
    }

    /// An ATTACHED machine must spell a foreign tail call with the value-call
    /// parentheses, so the same taken-arm edge arrives as a value call. It is
    /// the same edge and the guard is evaluated in the same place, so it must
    /// discharge the same requirement.
    #[test]
    fn a_parenthesized_taken_arm_target_discharges_what_its_bare_form_does() {
        assert!(
            accepted(&format!(
                "{CALLEE}
                data Runner {{}}
                machine Runner::run(&mut self, value: u64) -> u64 {{
                    transition value > 0 {{ true -> (check(value)) false -> (0) }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the parenthesized taken-arm target is the same arrival as the bare one"
        );
    }
}

/// The predicates a PREDICATE-ONLY domain membership reduces to, each rendered
/// at the subject, or `None` when the domain is not predicate-only.
///
/// [Domains](wiki/spec/language/domains.md#declaration-and-membership) settles
/// the rule: "Predicates alone establish predicate-only membership", and a
/// ROUTED domain is the case that "additionally needs exact authorized
/// provenance". So a routed, aliased or indexed domain returns `None` here and
/// keeps its provenance obligation; only a domain whose whole content is
/// predicates over `self` reduces, and then EVERY predicate must be
/// established, never a subset.
///
/// The rendering is `instantiate_domain_expression_label`, the same
/// substitution `contracts::domains` already runs in the MEMBERSHIP ->
/// PREDICATE direction. This is that rendering read the other way.
fn predicate_only_domain_labels(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    fact: &facts::Fact,
) -> Option<Vec<String>> {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = fact.payload else {
        return None;
    };
    let FactPlace::Place(place_handle) = fact.place else {
        return None;
    };
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    // The same list `domain_byte_predicate` enforces, so one definition governs
    // both grants: an alias expands to constituent requirements, index binders
    // carry an identity this substitution cannot supply, and `established by`
    // routes restrict who may mint membership at all.
    if domain.alias.is_some()
        || !domain.index_arguments.is_empty()
        || !domain.establishment_routes.is_empty()
        || !typed_trees::domain::index_parameters(program, domain).is_empty()
    {
        return None;
    }
    let declared = program.proof_facts(domain);
    if declared.is_empty() {
        // A predicate-free domain adds no membership obligation predicates
        // could discharge; whatever it asks for is not this.
        return None;
    }
    let subject_label = facts.semantic.place_label(program, place_handle);
    declared
        .iter()
        .map(|declared_fact| match declared_fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                Some(super::labels::instantiate_domain_expression_label(
                    program,
                    *expression,
                    &subject_label,
                ))
            }
            // A nested membership or a proposition needs more than this
            // substitution gives; refuse the whole domain rather than
            // establish a subset of its obligations.
            _ => None,
        })
        .collect()
}

/// The closed interval a PREDICATE-ONLY domain requires of its subject, paired
/// with that subject's label, or `None` when the domain states no interval.
fn predicate_only_domain_interval(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    fact: &facts::Fact,
) -> Option<(String, numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = fact.payload else {
        return None;
    };
    let FactPlace::Place(place_handle) = fact.place else {
        return None;
    };
    let domain = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
    if domain.alias.is_some()
        || !domain.index_arguments.is_empty()
        || !domain.establishment_routes.is_empty()
        || !typed_trees::domain::index_parameters(program, domain).is_empty()
    {
        return None;
    }
    let constraint = typed_trees::types::DomainConstraint {
        symbol: domain_symbol,
        ..Default::default()
    };
    let (minimum, maximum) = validation::declared_domain_predicate_bounds(program, &constraint)?;
    Some((
        facts.semantic.place_label(program, place_handle),
        minimum,
        maximum,
    ))
}

/// The caller expression a membership requirement is about.
///
/// A call-entry membership fact is already substituted at the caller, so its
/// place is rooted at the ACTUAL: `walk(remaining - 1)` states its obligation
/// about the subtraction itself. A projected place (`self.field`) is a
/// different subject and is not read here.
fn requirement_subject_expression(
    facts: &CheckFacts,
    fact: &facts::Fact,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let FactPlace::Place(place_handle) = fact.place else {
        return None;
    };
    let place = facts.semantic.places.get(place_handle);
    if !facts
        .semantic
        .place_segments
        .span_or_empty(place.segments)
        .is_empty()
    {
        return None;
    }
    match place.root {
        PlaceRoot::Expression(expression) => Some(expression),
        _ => None,
    }
}
