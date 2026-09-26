use crate::checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use crate::fact_plan::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use diagnostics::Diagnostic;
use language_core::is_self_receiver;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;
use symbols::SymbolHandle;

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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    nominal_requirements: &super::nominal_inputs::DeclaredFieldRequirements,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&crate::validation::CallFrameResolver<'_>>,
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
                    ) || proven_predicates_grant_domain(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        &entry_contexts,
                        incoming_guards,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    entry_contexts: &[crate::fact_plan::FactContextHandle],
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
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

/// PREDICATE -> MEMBERSHIP: a predicate-only domain's whole content is the
/// `requires self ...` proof facts it declares, so a caller that has already
/// proven every one of them AT THE SUBJECT has established membership without
/// a validating boundary call. This is the direction opposite to the
/// membership -> predicate reading `contracts::domains` runs at use sites,
/// and the same discharge `transition_guard_proves_requires` applies to a
/// call on the guarded arm itself -- extended here to the premises an
/// ordinary statement call can see. `predicate_only_domain_labels` refuses
/// routed, aliased or indexed domains, so no provenance obligation is ever
/// bypassed, and `predicate_only_domain_interval` declines a domain whose
/// predicates are not all readable bounds, so interval containment never
/// stands in for an unrecognized conjunct.
///
/// Two premise shapes qualify:
/// - a LIVE entry-context boolean fact (an authored `requires`, a `where`, a
///   transported call guarantee). Flow's mutation invalidation has already
///   retired anything a prior write could have staled, so what the context
///   states is what the call receives: an exact label, an `&&` clause, or the
///   subject's proven interval landing inside the domain's own.
/// - a DOMINATING incoming guard the ranges walk-back reconstructed at this
///   state's entry. The walk fences intermediate-state writes but not the
///   caller's OWN statements, so a guard premise additionally owes the gates
///   `incoming_guard_proves_requires` applies, re-expressed over the
///   substituted labels and the subject place -- never over the domain
///   predicate's bare `self`: every field the subject and the instantiated
///   predicates read preserved by `caller_state_preserves_field`, every name
///   the labels spell preserved by `caller_state_preserves_label_names`.
/// Whatever the premise, the jump's own earlier operands must leave the
/// membership's callee-formal reads unwritten (`guard_operands`).
fn proven_predicates_grant_domain(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[crate::fact_plan::FactContextHandle],
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    fact: &crate::fact_plan::Fact,
    call_frames: Option<&crate::validation::CallFrameResolver<'_>>,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = fact.payload else {
        return false;
    };
    let Some(predicate_expressions) = predicate_only_domain_predicates(program, domain_symbol)
    else {
        return false;
    };
    let Some(labels) = predicate_only_domain_labels(program, facts, fact) else {
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
    // Every instantiated predicate clause must be established; splitting `&&`
    // lets each piece come from a different premise, and a missing piece fails
    // the whole membership.
    let required_labels: Vec<String> = labels
        .iter()
        .flat_map(|label| {
            split_label_conjuncts(label)
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect();
    // The caller-terms boolean premises the live entry contexts carry. An
    // instantiated contract fact keeps only its label -- its raw expression
    // names the producer's scope, not this one's -- while a declaration-shaped
    // fact still owns the authored handle for conjunct and interval reads.
    let context_premises: Vec<(
        Option<symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle>,
        String,
    )> = entry_contexts
        .iter()
        .flat_map(|&handle| {
            let context = facts.semantic.contexts.get(handle);
            facts
                .semantic
                .context_view(context)
                .facts()
                .filter_map(|candidate| {
                    let label = crate::labels::semantic_boolean_fact_label(
                        program,
                        &facts.semantic,
                        candidate,
                    )
                    .or_else(|| {
                        facts
                            .semantic
                            .proposition_fact_label(program, candidate)
                            .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
                    })?;
                    let expression = match candidate.payload {
                        FactPayload::BooleanExpression(expression) => Some(expression),
                        FactPayload::ContractBooleanExpression {
                            expression,
                            instantiated,
                            ..
                        } if !instantiated.is_valid() => Some(expression),
                        _ => None,
                    };
                    Some((expression, label))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    // The domain's interval is whole-domain evidence: when every predicate is
    // a recognized bound, proving the subject inside it establishes all of
    // them at once, and no per-label match is needed.
    let interval = predicate_only_domain_interval(program, facts, fact).map(
        |(subject_label, required_low, required_high)| {
            let contains = |(low, high): (numerics::bignum::BigInt, numerics::bignum::BigInt)| {
                low >= required_low && high <= required_high
            };
            let by_context = context_premises.iter().any(|(expression, label)| {
                expression.is_some_and(|expression| {
                    super::intervals::guard_interval_for_label(program, expression, &subject_label)
                        .is_some_and(contains)
                }) || label_subject_interval(label, &subject_label).is_some_and(contains)
            }) || requirement_subject_expression(facts, fact).is_some_and(
                |argument| {
                    let context_expressions: Vec<symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle> =
                        context_premises
                            .iter()
                            .filter_map(|(expression, _)| *expression)
                            .collect();
                    super::intervals::expression_interval(
                        program,
                        machine,
                        state,
                        &context_expressions,
                        argument,
                    )
                    .is_some_and(contains)
                },
            );
            let by_guard = !by_context
                && incoming
                    .iter()
                    .filter(|guard| guard.holds_at(state_flow.state_symbol))
                    .any(|guard| {
                        super::intervals::guard_interval_for_label(
                            program,
                            guard.guard(),
                            &subject_label,
                        )
                        .is_some_and(contains)
                            || guard.direct_arguments().is_some_and(|arguments| {
                                let renamed = instantiate_state_parameter_label(
                                    program,
                                    state,
                                    arguments,
                                    &subject_label,
                                );
                                super::intervals::guard_interval_for_label(
                                    program,
                                    guard.guard(),
                                    &renamed,
                                )
                                .is_some_and(contains)
                            })
                    });
            (by_context, by_guard)
        },
    );
    let mut premise_used_guard = interval.as_ref().is_some_and(|(_, by_guard)| *by_guard);
    let proven = interval
        .as_ref()
        .is_some_and(|(by_context, by_guard)| *by_context || *by_guard)
        || required_labels.iter().all(|required| {
            context_premises.iter().any(|(expression, label)| {
                label_conjunct_matches(label, required)
                    || expression.is_some_and(|expression| {
                        guard_conjunct_matches(program, expression, required)
                    })
            }) || {
                let proven = incoming
                    .iter()
                    .filter(|guard| guard.holds_at(state_flow.state_symbol))
                    .any(|guard| {
                        guard_conjunct_matches(program, guard.guard(), required)
                            || guard.direct_arguments().is_some_and(|arguments| {
                                let renamed = instantiate_state_parameter_label(
                                    program, state, arguments, required,
                                );
                                guard_conjunct_matches(program, guard.guard(), &renamed)
                            })
                    });
                premise_used_guard |= proven;
                proven
            }
        });
    proven
        // Only the guard half needs caller-state gates: the walk-back has
        // already fenced intermediate-state writes, so what remains is this
        // state's own statements up to the call.
        && (!premise_used_guard
            || caller_state_preserves_predicate_premise(
                program,
                facts,
                state_flow,
                call_flow,
                state,
                fact,
                &predicate_expressions,
                &required_labels,
            ))
        // The statement scans above stop where the statement begins; the
        // jump's own operands run after them and before the membership's read.
        && super::guard_operands::requirement_reads_survive_earlier_operand_writes(
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

/// Whole-caller-state preservation for a premise established BEFORE this
/// state's statements ran. `caller_state_preserves_field` matches member names
/// anywhere in a write target, so `pair.cap = ...` defeats a predicate's
/// `self.cap` read at a `pair` subject exactly as `self.cap = ...` does; the
/// field set is the subject place's own field segments plus every member the
/// domain predicates and a subject expression name. The label-name scan keeps
/// any unqualified token -- the subject's root, a rebound local -- quoting a
/// live premise rather than a stale one.
#[allow(clippy::too_many_arguments)]
fn caller_state_preserves_predicate_premise(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    fact: &crate::fact_plan::Fact,
    predicate_expressions: &[symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle],
    required_labels: &[String],
) -> bool {
    let mut fields: Vec<symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier> =
        Vec::new();
    for expression in predicate_expressions {
        collect_expression_self_fields(program, *expression, &mut fields);
    }
    if let Some(argument) = requirement_subject_expression(facts, fact) {
        collect_expression_self_fields(program, argument, &mut fields);
    }
    if let FactPlace::Place(place_handle) = fact.place {
        let place = facts.semantic.places.get(place_handle);
        for segment in facts.semantic.place_segments.span_or_empty(place.segments) {
            if let PlaceSegment::Field { symbol } = segment {
                fields.push(
                    symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::from(
                        symbol_name(program, *symbol).as_str(),
                    ),
                );
            }
        }
    }
    fields
        .iter()
        .all(|field| caller_state_preserves_field(program, state, field))
        && required_labels.iter().all(|label| {
            caller_state_preserves_label_names(
                program,
                facts,
                state_flow,
                call_flow,
                state,
                label,
            )
        })
        // The name gate cannot see a `self`-rooted write (`self` is skipped
        // as a label token), so a prior CALL writing `self.limit` would leave
        // the guard premise quoting stale storage. Compare the mutated place
        // itself -- root-normalized to the machine's storage identity --
        // against the membership's subject place.
        && !facts
            .flow
            .state_call_prior_invalidations(state_flow, call_flow)
            .any(|invalidation| {
                let FactPlace::Place(place_handle) = fact.place else {
                    return false;
                };
                let subject = facts.semantic.places.get(place_handle);
                crate::flow::normalized_event_place_root(program, invalidation.mutated_root)
                    == crate::flow::normalized_event_place_root(program, subject.root)
                    && crate::flow::canonical_place_segments_may_overlap(
                        program,
                        facts
                            .flow
                            .invalidations
                            .segments
                            .span_or_empty(invalidation.mutated_segments),
                        facts.semantic.place_segments.span_or_empty(subject.segments),
                    )
            })
}

/// `&&`-clause splitting over a display label, depth-aware so an `f(a && b)`
/// argument or a `match` body is never mistaken for a top-level clause.
fn split_label_conjuncts(label: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut cursor = 0usize;
    while cursor < label.len() {
        match label.as_bytes()[cursor] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'&' if depth == 0 && label[cursor..].starts_with(" && ") => {
                parts.push(&label[start..cursor]);
                cursor += " && ".len();
                start = cursor;
                continue;
            }
            _ => {}
        }
        cursor += 1;
    }
    parts.push(&label[start..]);
    parts
}

/// The label analogue of `guard_conjunct_matches`: equality, `x == true`
/// unwrapping, and `&&` decomposition on the CANDIDATE only -- an `||` never
/// discharges the conjunct the requirement states.
fn label_conjunct_matches(candidate_label: &str, required_label: &str) -> bool {
    let candidate = candidate_label
        .strip_suffix(" == true")
        .unwrap_or(candidate_label);
    if candidate == required_label {
        return true;
    }
    let conjuncts = split_label_conjuncts(candidate);
    conjuncts.len() > 1
        && conjuncts
            .iter()
            .any(|part| label_conjunct_matches(part, required_label))
}

/// The interval an instantiated boolean label states for `subject_label`,
/// accumulating every `&&` clause that is a closed comparison over that exact
/// spelling. The label analogue of `intervals::guard_interval_for_label`, for
/// contract facts whose caller-terms survive only as text.
fn label_subject_interval(
    label: &str,
    subject_label: &str,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let mut bounds: Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> = None;
    for conjunct in split_label_conjuncts(label) {
        let Some((low, high)) = comparison_label_bound(conjunct, subject_label) else {
            continue;
        };
        bounds = Some(match bounds {
            Some((prior_low, prior_high)) => (prior_low.max(low), prior_high.min(high)),
            None => (low, high),
        });
    }
    bounds
}

/// One clause's interval contribution: `{subject} <op> <literal>` or the
/// mirrored `{literal} <op> <subject>`, whitespace-checked so `index` never
/// prefix-matches `index2`.
fn comparison_label_bound(
    conjunct: &str,
    subject_label: &str,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    if let Some(rest) = conjunct
        .strip_prefix(subject_label)
        .and_then(|rest| rest.strip_prefix(' '))
        && let Some((operator, literal)) = rest.split_once(' ')
    {
        return comparison_label_endpoints(operator, literal, true);
    }
    if let Some(rest) = conjunct
        .strip_suffix(subject_label)
        .and_then(|rest| rest.strip_suffix(' '))
        && let Some((literal, operator)) = rest.split_once(' ')
    {
        return comparison_label_endpoints(operator, literal, false);
    }
    None
}

/// The interval `subject OP value` states -- the label rendering of
/// `intervals`' comparison reader, with the unconstrained side at the
/// carrier-independent extreme.
fn comparison_label_endpoints(
    operator: &str,
    literal: &str,
    subject_on_left: bool,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::bignum::BigInt;
    let value = BigInt::from_decimal_str(literal)?;
    let one = BigInt::from_i64(1);
    let (low, high) = match (operator, subject_on_left) {
        ("<=", true) | (">=", false) => (None, Some(value)),
        ("<", true) | (">", false) => (None, Some(value.sub(&one))),
        (">=", true) | ("<=", false) => (Some(value), None),
        (">", true) | ("<", false) => (Some(value.add(&one)), None),
        ("==", _) => (Some(value.clone()), Some(value)),
        _ => return None,
    };
    Some((
        low.unwrap_or_else(|| BigInt::from_i64(i64::MIN)),
        high.unwrap_or_else(|| BigInt::from_i64(i64::MAX)),
    ))
}

/// Clear "needs fact X here" guidance for a proof-backed operator/contract that
/// is missing a required boolean fact (for example an index bound or a
/// domain-sensitive operator precondition). The caller has not established the
/// fact in the entry context, so name exactly what must hold before the call.
fn explain_missing_boolean_fact(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    required_place: crate::fact_plan::PlaceHandle,
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
        let mutated = crate::fact_plan::canonical_place_label_from_parts(
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&crate::validation::CallFrameResolver<'_>>,
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
    let mut fields: Vec<symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier> =
        Vec::new();
    collect_expression_self_fields(program, expression, &mut fields);
    fields
        .iter()
        .all(|field| caller_state_preserves_field(program, state, field))
        && caller_state_preserves_self_fields_against_call_writes(
            program, facts, state_flow, call_flow, &fields,
        )
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    target: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    arguments: arena::HandleSpan<
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    >,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &crate::fact_plan::Fact,
    call_frames: Option<&crate::validation::CallFrameResolver<'_>>,
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
    let symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::When(
        guard,
    ) = transition.guard
    else {
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
                symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Value(value) if value == expression
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    guard: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    required_label: &str,
) -> bool {
    if program.expression_table.display_name(guard) == required_label {
        return true;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return false;
    };
    match binary.operator {
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator::And => {
            guard_conjunct_matches(program, binary.left, required_label)
                || guard_conjunct_matches(program, binary.right, required_label)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::BinaryOperator::Equal
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    fields: &mut Vec<symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier>,
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
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    state: &symbol_resolved_trees_to_typed_trees::typed_trees::state::State,
    field: &symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier,
) -> bool {
    use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;
    for statement in program.statement_table.statements(state.statement_nodes) {
        if let StatementNode::Assignment(assignment) = statement
            && assignment_target_mentions_field(program, assignment.target, field)
        {
            return false;
        }
    }
    true
}

/// The field walk above sees ASSIGNMENTS only: `self.clear()` is a call whose
/// write lands in the state's prior-invalidation list instead. A mutation
/// rooted at this machine's own `self` storage and naming one of the fields
/// the premise reads stales that premise. Writes through a non-`self` root
/// stay with `caller_state_preserves_label_names`, which matches the root's
/// caller-visible name.
fn caller_state_preserves_self_fields_against_call_writes(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fields: &[symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier],
) -> bool {
    !facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
        .any(|invalidation| {
            if crate::flow::normalized_event_place_root(program, invalidation.mutated_root)
                != PlaceRoot::Symbol(state_flow.machine_symbol)
            {
                return false;
            }
            let segments = facts
                .flow
                .invalidations
                .segments
                .span_or_empty(invalidation.mutated_segments);
            // A whole-`self` write, or a segment that is not a plain field,
            // cannot be ruled out by the field names.
            segments.is_empty()
                || segments.iter().any(|segment| match segment {
                    PlaceSegment::Field { symbol } => fields
                        .iter()
                        .any(|field| field.as_str() == symbol_name(program, *symbol).as_str()),
                    _ => true,
                })
        })
}

fn assignment_target_mentions_field(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    target: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    field: &symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier,
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
    use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode,
    };
    use symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier;
    use symbol_resolved_trees_to_typed_trees::typed_trees::statement::{
        StatementNode, TransitionGuardNode,
    };

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

    fn program() -> symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees {
        typed_program(SOURCE)
    }

    fn state_statement<'program>(
        program: &'program symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
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
        program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
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

    fn assignment_target(
        program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        index: usize,
    ) -> ExpressionHandle {
        let StatementNode::Assignment(assignment) = state_statement(program, "main", index) else {
            panic!("statement {index} is not an assignment");
        };
        assignment.target
    }

    fn label(
        program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        expression: ExpressionHandle,
    ) -> String {
        program.expression_table.display_name(expression)
    }

    /// Lowering wraps every authored guard in an implicit `== true`; the
    /// authored expression sits on its left.
    fn authored_guard(
        program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
        guard: ExpressionHandle,
    ) -> ExpressionHandle {
        let ExpressionNode::Binary(wrapper) = program.expression_table.expression(guard) else {
            panic!("guard is not a binary expression");
        };
        assert_eq!(wrapper.operator, BinaryOperator::Equal);
        wrapper.left
    }

    fn and_operands(
        program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
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

#[cfg(test)]
mod predicate_only_domain_statement_probes {
    //! `proven_predicates_grant_domain` discharges a predicate-only domain
    //! membership for ORDINARY statement calls — the premises a transition
    //! arm's guard never reaches: a live `requires` fact already in the
    //! entry context, and a dominating incoming guard reconstructed at this
    //! state's entry. The gates are what keep both honest: a premise quoted
    //! from before a write is no premise at all.

    use crate::tests::front_end::checked_program_result;

    fn accepted(source: &str) -> bool {
        checked_program_result(source).is_ok()
    }

    const STORE: &str = "domain u64::Slot16 requires self <= 15;
        data Store { arr: [u64; 16]; }
        machine Store::read(&mut self, index: u64 in Slot16) -> u64 {
            transition { _ -> (self.arr[index]) }
        }";

    /// A live `requires` fact states the predicate at the actual subject; the
    /// statement call discharges membership from it. Strict and non-strict
    /// spellings of the same interval both qualify, a wider premise does not,
    /// and a domain carrying a second predicate the premise cannot state does
    /// not.
    #[test]
    fn a_requires_fact_establishes_predicate_only_membership_at_a_statement_call() {
        let program = |clause: &str| {
            format!(
                "{STORE}
                machine Store::scan(&mut self, index: u64) -> u64
                    requires {clause};
                {{
                    let v: u64 = self.read(index);
                    transition {{ _ -> v }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )
        };
        assert!(
            accepted(&program("index <= 15")),
            "requires `index <= 15` is the domain's whole content at `index`"
        );
        assert!(
            accepted(&program("index < 16")),
            "`index < 16` is the same closed interval"
        );
        assert!(
            !accepted(&program("index < 32")),
            "`index < 32` admits 16..=31, outside the domain"
        );
        assert!(
            !accepted(&format!(
                "domain u64::NotSeven requires self <= 15 && self != 7;
                data Store {{ arr: [u64; 16]; }}
                machine Store::read(&mut self, index: u64 in NotSeven) -> u64 {{
                    transition {{ _ -> (self.arr[index]) }}
                }}
                machine Store::scan(&mut self, index: u64) -> u64
                    requires index <= 15;
                {{
                    let v: u64 = self.read(index);
                    transition {{ _ -> v }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the interval proves `self <= 15` but cannot state `self != 7`"
        );
    }

    /// A ROUTED domain keeps its provenance obligation at a statement call
    /// exactly as at a transition arm: proving the predicate is not the same
    /// as being authorized to mint membership.
    #[test]
    fn a_routed_domain_still_needs_its_established_by_route() {
        assert!(
            !accepted(
                "pub boundary trait Granter { machine grant(v: u64) -> u64 in Slot16; }
                pub domain u64::Slot16 requires self <= 15 established by Granter::grant;
                data Store { arr: [u64; 16]; }
                machine Store::read(&mut self, index: u64 in Slot16) -> u64 {
                    transition { _ -> (self.arr[index]) }
                }
                machine Store::scan(&mut self, index: u64) -> u64
                    requires index <= 15;
                {
                    let v: u64 = self.read(index);
                    transition { _ -> v }
                }
                data Main {}
                machine Main::main(&mut self) {}"
            ),
            "a `requires` fact must never mint membership a route owns"
        );
    }

    /// A dominating incoming guard is a premise the state's own statements
    /// must preserve: untouched it discharges, an ASSIGNMENT to the field
    /// stales it, and so does a CALL whose write frame covers the field —
    /// the prior-invalidation list, not the statement scan, is what sees it.
    #[test]
    fn an_incoming_guard_premise_must_survive_the_states_own_statements() {
        const SETUP: &str = "domain u64::Limit requires self <= 8;
            data Store { limit: u64; }
            machine Store::clear(&mut self) -> u64 {
                self.limit = 99;
                transition { _ -> (0) }
            }
            machine Store::done(&mut self, a: u64, v: u64 in Limit) -> u64 {
                transition { _ -> v }
            }";
        let program = |inner_body: &str| {
            format!(
                "{SETUP}
                machine Store::scan(&mut self) -> u64 {{
                    transition self.limit <= 8 {{
                        true -> inner()
                        false -> (0)
                    }}
                    state inner(&mut self) -> u64 {{
                        {inner_body}
                        let v: u64 = self.done(0, self.limit);
                        transition {{ _ -> v }}
                    }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )
        };
        assert!(
            accepted(&program("")),
            "the edge guard must reach the call in an untouched state"
        );
        assert!(
            !accepted(&program("self.limit = 99;")),
            "an assignment rewriting the field stales the quoted guard"
        );
        assert!(
            !accepted(&program("let a: u64 = self.clear();")),
            "a call whose write frame covers the field stales it the same way"
        );
    }

    /// The same self-field write through a prior CALL stales a quoted guard
    /// for a plain boolean `requires` too; the membership route is not the
    /// only one that had to learn to look at prior invalidations.
    #[test]
    fn a_boolean_requires_cannot_quote_a_guard_staled_by_a_call() {
        assert!(
            !accepted(
                "data Store { limit: u64; }
                machine Store::clear(&mut self) -> u64 {
                    self.limit = 99;
                    transition { _ -> (0) }
                }
                machine Store::done(&mut self, a: u64) -> u64
                    requires self.limit <= 8;
                {
                    transition { _ -> a }
                }
                machine Store::scan(&mut self) -> u64 {
                    transition self.limit <= 8 {
                        true -> inner()
                        false -> (0)
                    }
                    state inner(&mut self) -> u64 {
                        let a: u64 = self.clear();
                        let v: u64 = self.done(a);
                        transition { _ -> v }
                    }
                }
                data Main {}
                machine Main::main(&mut self) {}"
            ),
            "`self.clear()` wrote the premise the guard spelled"
        );
    }

    /// The jump's own operands evaluate between the guard and the callee's
    /// read of the membership subject: `self.clear()` rewrites `self.limit`
    /// before `done` reads it, so the guard's `self.limit <= 8` no longer
    /// describes the delivered value. Ordering decides: the same pair with
    /// the read FIRST is the pre-write value and remains sound, and a
    /// non-writing operand never disturbs it.
    #[test]
    fn an_earlier_operand_write_defeats_the_guard_premise_it_follows() {
        const SETUP: &str = "domain u64::Limit requires self <= 8;
            data Store { limit: u64; }
            machine Store::clear(&mut self) -> u64 {
                self.limit = 99;
                transition { _ -> (0) }
            }
            machine Store::noop(&mut self) -> u64 {
                transition { _ -> (0) }
            }";
        assert!(
            !accepted(&format!(
                "{SETUP}
                machine Store::done(&mut self, a: u64, v: u64 in Limit) -> u64 {{
                    transition {{ _ -> v }}
                }}
                machine Store::scan(&mut self) -> u64 {{
                    transition self.limit <= 8 {{
                        true -> (self.done(self.clear(), self.limit))
                        false -> (0)
                    }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "`clear()` wrote the subject between the guard and the read"
        );
        assert!(
            accepted(&format!(
                "{SETUP}
                machine Store::done(&mut self, v: u64 in Limit, a: u64) -> u64 {{
                    transition {{ _ -> v }}
                }}
                machine Store::scan(&mut self) -> u64 {{
                    transition self.limit <= 8 {{
                        true -> (self.done(self.limit, self.clear()))
                        false -> (0)
                    }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "the subject operand is read before `clear()` runs"
        );
        assert!(
            accepted(&format!(
                "{SETUP}
                machine Store::done(&mut self, a: u64, v: u64 in Limit) -> u64 {{
                    transition {{ _ -> v }}
                }}
                machine Store::scan(&mut self) -> u64 {{
                    transition self.limit <= 8 {{
                        true -> (self.done(self.noop(), self.limit))
                        false -> (0)
                    }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )),
            "an operand that writes nothing leaves the premise intact"
        );
    }

    /// An edge argument renamed at the transition still grounds the premise:
    /// `walk(remaining)` arrives at `remaining <= 5` and the domain is read at
    /// the callee's own spelling. The control keeps both directions honest:
    /// the constraint belongs to the delivered argument, not the name.
    #[test]
    fn a_renamed_edge_argument_carries_the_premise_to_its_parameter() {
        let program = |guard: &str| {
            format!(
                "domain u64::Fuel requires self <= 5;
                data Store {{}}
                machine Store::take(&mut self, value: u64 in Fuel) -> u64 {{
                    transition {{ _ -> (0) }}
                }}
                machine Store::scan(&mut self, n: u64) -> u64 {{
                    transition {guard} {{
                        true -> inner(n)
                        false -> (0)
                    }}
                    state inner(&mut self, remaining: u64) -> u64 {{
                        let v: u64 = self.take(remaining);
                        transition {{ _ -> v }}
                    }}
                }}
                data Main {{}}
                machine Main::main(&mut self) {{}}"
            )
        };
        assert!(
            accepted(&program("n <= 5")),
            "`n <= 5` on the edge is `remaining <= 5` in the state"
        );
        assert!(
            !accepted(&program("n <= 9")),
            "`n <= 9` does not bound `remaining` inside the domain"
        );
    }
}

/// The proof-fact expressions of a PREDICATE-ONLY domain, or `None` when the
/// domain is not predicate-only.
///
/// [Domains](wiki/spec/language/domains.md#declaration-and-membership) settles
/// the rule: "Predicates alone establish predicate-only membership", and a
/// ROUTED domain is the case that "additionally needs exact authorized
/// provenance". So a routed, aliased or indexed domain returns `None` here and
/// keeps its provenance obligation; only a domain whose whole content is
/// predicates over `self` reduces, and then EVERY predicate must be
/// established, never a subset.
fn predicate_only_domain_predicates(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<Vec<symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle>> {
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
        || !symbol_resolved_trees_to_typed_trees::typed_trees::domain::index_parameters(
            program, domain,
        )
        .is_empty()
    {
        return None;
    }
    let declared = program.proof_facts(domain);
    if declared.is_empty() {
        // A predicate-free domain adds no membership obligation predicates
        // could discharge; whatever it asks for is not this.
        return None;
    }
    declared
        .iter()
        .map(|declared_fact| match declared_fact {
            symbol_resolved_trees_to_typed_trees::typed_trees::domain::ProofFact::Expression(
                expression,
            ) => Some(*expression),
            // A nested membership or a proposition needs more than this
            // substitution gives; refuse the whole domain rather than
            // establish a subset of its obligations.
            _ => None,
        })
        .collect()
}

/// The predicates a PREDICATE-ONLY domain membership reduces to, each rendered
/// at the subject, or `None` when the domain is not predicate-only.
///
/// The rendering is `instantiate_domain_expression_label`, the same
/// substitution `contracts::domains` already runs in the MEMBERSHIP ->
/// PREDICATE direction. This is that rendering read the other way.
fn predicate_only_domain_labels(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    fact: &crate::fact_plan::Fact,
) -> Option<Vec<String>> {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = fact.payload else {
        return None;
    };
    let FactPlace::Place(place_handle) = fact.place else {
        return None;
    };
    let subject_label = facts.semantic.place_label(program, place_handle);
    Some(
        predicate_only_domain_predicates(program, domain_symbol)?
            .iter()
            .map(|expression| {
                super::labels::instantiate_domain_expression_label(
                    program,
                    *expression,
                    &subject_label,
                )
            })
            .collect(),
    )
}

/// The closed interval a PREDICATE-ONLY domain requires of its subject, paired
/// with that subject's label, or `None` when the domain states no interval.
///
/// Establishment direction, so the interval must be EXACT: every predicate a
/// recognized bound on `self`, or containing the subject inside it would grant
/// membership while a predicate it cannot see -- `self != 7` beside
/// `self <= 15` -- stays unproven.
fn predicate_only_domain_interval(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    facts: &CheckFacts,
    fact: &crate::fact_plan::Fact,
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
        || !symbol_resolved_trees_to_typed_trees::typed_trees::domain::index_parameters(
            program, domain,
        )
        .is_empty()
    {
        return None;
    }
    let constraint = symbol_resolved_trees_to_typed_trees::typed_trees::types::DomainConstraint {
        symbol: domain_symbol,
        ..Default::default()
    };
    let (minimum, maximum) =
        crate::validation::exact_declared_domain_interval(program, &constraint)?;
    Some((
        facts.semantic.place_label(program, place_handle),
        minimum.unwrap_or_else(|| numerics::bignum::BigInt::from_i64(i64::MIN)),
        maximum.unwrap_or_else(|| numerics::bignum::BigInt::from_i64(i64::MAX)),
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
    fact: &crate::fact_plan::Fact,
) -> Option<symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle> {
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
