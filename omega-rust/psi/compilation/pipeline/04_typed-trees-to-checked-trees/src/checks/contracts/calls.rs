mod boolean_requirements;
mod captured_boolean;
mod guard_observations;
mod labels;
mod predicate_domains;
mod state_preservation;
mod transition_guards;
use labels::{
    instantiate_state_parameter_label, label_conjunct_matches, label_subject_interval,
    split_label_conjuncts,
};
use predicate_domains::{
    predicate_only_domain_interval, predicate_only_domain_labels, predicate_only_domain_predicates,
    requirement_subject_expression,
};
use state_preservation::{
    caller_state_preserves_field, caller_state_preserves_label_names,
    caller_state_preserves_predicate_premise,
    caller_state_preserves_self_fields_against_call_writes, collect_expression_self_fields,
};
pub(super) use transition_guards::transition_guard_proves_boolean_requirement;
use transition_guards::{
    guard_conjunct_matches, negated_guard_conjunct_matches, transition_guard_proves_requires,
};

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use facts::{FactPayload, FactPlace, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::labels::{
    call_target_label, joined_place_label, machine_name, semantic_fact_requirement_label,
    symbol_name,
};

pub(super) fn check_call_requires(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    nominal_requirements: &super::nominal_inputs::DeclaredFieldRequirements,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    expression_intervals: &[validation::ExpressionInterval],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    constant_obligations: &mut Vec<checked_trees::ConstantPredicateObligation>,
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
    super::contextual_cleanup::check(
        program,
        facts,
        state_flow,
        call_flow,
        &entry_contexts,
        call_frames,
        diagnostics,
    );
    super::nominal_inputs::check(
        program,
        facts,
        state_flow,
        call_flow,
        nominal_requirements,
        diagnostics,
    );
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let membership = super::call_values::membership_evidence(
                program, facts, state_flow, call_flow, fact,
            );
            let requirement_contexts = membership
                .as_ref()
                .map_or(entry_contexts.as_slice(), |evidence| {
                    evidence.contexts.as_slice()
                });
            let captured_judgment = captured_boolean::judgment(
                program,
                facts,
                state_flow,
                call_flow,
                requirement_contexts,
                fact,
                expression_intervals,
                call_frames,
            );
            let satisfied = captured_judgment.unwrap_or_else(|| match fact.payload {
                FactPayload::ContractBooleanExpression { expression, .. } => {
                    boolean_requirements::proves(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        requirement_contexts,
                        expression,
                        incoming_guards,
                        call_frames,
                    )
                }
                _ => {
                    semantic_contexts_prove_contract_fact(
                        program,
                        &facts.semantic,
                        requirement_contexts,
                        fact,
                    ) || transparent_requirement_proves(program, facts, state_flow, call_flow, fact)
                }
            });
            // ch8 construction-grant: a string literal whose compile-time bytes
            // satisfy a domain's declared byte-predicate fact grants that
            // domain without a validating boundary call -- this is how a literal
            // flows into a `&[u8] in Utf8` target.
            let satisfied = satisfied
                || (captured_judgment.is_none()
                    && !super::prover::indexed_membership(program, fact.payload)
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
                        requirement_contexts,
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
                        requirement_contexts,
                        fact,
                        membership
                            .as_ref()
                            .and_then(|evidence| evidence.referents.as_deref()),
                        call_frames,
                    ) || proven_predicates_grant_domain(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        requirement_contexts,
                        incoming_guards,
                        fact,
                        call_frames,
                        expression_intervals,
                    )));

            let satisfied = satisfied
                && super::prover::contract_membership_place_is_accessible(
                    program,
                    &facts.semantic,
                    requirement_contexts,
                    fact,
                );
            if !satisfied {
                if let FactPayload::ContractBooleanExpression { expression, .. } = fact.payload
                    && let Some((target_machine, target_state)) =
                        validation::find_state(program, call_flow.target_symbol)
                    && let Some(obligation) = checked_trees::ConstantPredicateObligation::retain(
                        program,
                        expression,
                        target_machine.symbol,
                        state_flow.machine_symbol,
                        format!(
                            "requires contract for call {} from {}: {}",
                            call_target_label(program, call_flow.target_symbol),
                            machine_name(program, state_flow.machine_symbol),
                            semantic_fact_requirement_label(program, &facts.semantic, fact)
                        ),
                        &|expression| {
                            validation::has_builtin_binary_expression_meaning(
                                program,
                                target_machine,
                                Some(target_state),
                                expression,
                            )
                        },
                    )
                {
                    constant_obligations.push(obligation);
                    continue;
                }
                let detail = match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. }
                    | FactPayload::DomainMembership { domain_symbol, .. } => {
                        let Some(place) = fact.place.as_place() else {
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
                    "cannot prove requires contract for call {} from {}: {}{} in state `{}`",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                    detail
                        .map(|message| format!(" ({message})"))
                        .unwrap_or_default(),
                    symbol_name(program, state_flow.state_symbol)
                )));
            }
        }
    }
}

/// A transparent proposition requirement holds when the calling state's own
/// premises prove its unfolded definition under the call's argument terms.
/// Those premises describe state entry, so any earlier write in the state
/// withdraws the route.
fn transparent_requirement_proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    fact: &facts::Fact,
) -> bool {
    let FactPayload::ContractPropositionApplication { fact: source, .. } = fact.payload else {
        return false;
    };
    let Some(application) = program.proof_facts.get(source).as_proposition() else {
        return false;
    };
    let Some((caller, caller_state)) = validation::find_state(program, state_flow.state_symbol)
        .filter(|(machine, _)| machine.symbol == state_flow.machine_symbol)
    else {
        return false;
    };
    let Some((callee, callee_state)) = validation::find_state(program, call_flow.target_symbol)
    else {
        return false;
    };
    facts
        .flow
        .state_call_prior_invalidations(state_flow, call_flow)
        .next()
        .is_none()
        && validation::transparent_call_requirement_entailed(
            program,
            caller,
            caller_state,
            callee,
            callee_state,
            facts
                .flow
                .control
                .call_arguments
                .span_or_empty(call_flow.authored_arguments),
            application,
        )
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    entry_contexts: &[facts::FactContextHandle],
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((_, _, domain_symbol, _)) = payload.domain_membership() else {
        return false;
    };
    let Some(place_handle) = place.as_place() else {
        return false;
    };
    if !validation::field_domains::domain_is_subslice_preserving(program, domain_symbol) {
        return false;
    }
    // Place canonicalization turns literal half-open ranges into FixedRange;
    // bounds requiring runtime observations retain Index(Range). Both select
    // a subslice of the same base, whose membership is checked below.
    let resolved = *facts.semantic.places.get(place_handle);
    let segments: Vec<PlaceSegment> = facts
        .semantic
        .place_segments
        .span_or_empty(resolved.segments)
        .to_vec();
    let Some((selection, base_segments)) = segments.split_last() else {
        return false;
    };
    let is_subslice = match selection {
        PlaceSegment::FixedRange { .. } => true,
        PlaceSegment::Index { expression } => matches!(
            program.expression_table.expression(*expression),
            ExpressionNode::Range(_)
        ),
        _ => false,
    };
    if !is_subslice {
        return false;
    }
    // The BASE place (root + base_segments) must be provably in a domain implying
    // `domain_symbol` -- match it against the entry-context domain facts (the same
    // discharge `value_proves_domain` uses for a domained param/field carried in).
    entry_contexts.iter().any(|&context_handle| {
        let context = facts.semantic.contexts.get(context_handle);
        facts.semantic.context_view(context).facts().any(|fact| {
            let fact_domain = match fact.payload {
                facts::domain_membership_symbol_pattern!(domain_symbol) => domain_symbol,
                _ => return false,
            };
            if !facts.semantic.domain_implies(fact_domain, domain_symbol)
                && !validation::field_domains::domain_membership_implies(
                    program,
                    fact_domain,
                    domain_symbol,
                )
            {
                return false;
            }
            let Some(fact_place) = fact.place.as_place() else {
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((_, _, domain_symbol, _)) = payload.domain_membership() else {
        return false;
    };
    let Some(place_handle) = place.as_place() else {
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
    let Some(root_symbol) = resolved.root.as_symbol() else {
        return false;
    };
    let Some(state) = program.state_by_symbol(state_flow.state_symbol) else {
        return false;
    };
    let Some(parameter) = program.state_parameter_by_symbol(state, root_symbol) else {
        return false;
    };
    if parameter.is_mutable {
        return false;
    }
    validation::field_domains::domain_constraint_symbols(program, parameter.type_reference)
        .into_iter()
        .any(|param_domain| {
            facts.semantic.domain_implies(param_domain, domain_symbol)
                || validation::field_domains::domain_membership_implies(
                    program,
                    param_domain,
                    domain_symbol,
                )
        })
}

/// The `domain_symbol` and no-segment expression root a membership `place`
/// names — the shared peel both grant checks apply before their payload
/// tests.
fn expression_rooted_domain_parts(
    semantic: &facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> Option<(
    symbols::SymbolHandle,
    typed_trees::expression::ExpressionHandle,
)> {
    let Some((_, _, domain_symbol, _)) = payload.domain_membership() else {
        return None;
    };
    let place_handle = place.as_place()?;
    let resolved = semantic.places.get(place_handle);
    if !resolved.segments.is_empty() {
        return None;
    }
    let expression = resolved.root.as_expression()?;
    Some((domain_symbol, expression))
}

fn string_literal_grants_domain(
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((domain_symbol, expression)) =
        expression_rooted_domain_parts(semantic, payload, place)
    else {
        return false;
    };
    validation::field_domains::string_literal_expression_grants_domain(
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
    program: &dyn ::validation::CheckingInput,
    semantic: &facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let Some((domain_symbol, expression)) =
        expression_rooted_domain_parts(semantic, payload, place)
    else {
        return false;
    };
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some(target) = program.state_by_symbol(call.target_symbol) else {
        return false;
    };
    if !target.return_type.is_valid() {
        return false;
    }
    validation::field_domains::predicate_domain_constraint_symbols(program, target.return_type)
        .into_iter()
        .any(|return_domain| {
            semantic.domain_implies(return_domain, domain_symbol)
                || validation::field_domains::domain_membership_implies(
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
///
/// Whatever the premise, the jump's own earlier operands must leave the
/// membership's callee-formal reads unwritten (`guard_operands`).

/// The invocation's call site plus its target signature parameters.
fn call_site_target_parameters<'p>(
    program: &'p dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<(
    validation::CallSite<'p>,
    &'p [typed_trees::signature::StateParameter],
)> {
    let call_site = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )?;
    let target_parameters = validation::call_target_parameters(program, call_flow.target_symbol)?;
    Some((call_site, target_parameters))
}

fn proven_predicates_grant_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    expression_intervals: &[validation::ExpressionInterval],
) -> bool {
    let Some((_, _, domain_symbol, _)) = fact.payload.domain_membership() else {
        return false;
    };
    let Some(predicate_expressions) = predicate_only_domain_predicates(program, domain_symbol)
    else {
        return false;
    };
    let Some(labels) = predicate_only_domain_labels(program, facts, fact) else {
        return false;
    };
    let Some((call_site, target_parameters)) =
        call_site_target_parameters(program, state_flow, call_flow)
    else {
        return false;
    };
    let Some((_machine, state)) =
        program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)
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
    let context_premises: Vec<(Option<typed_trees::expression::ExpressionHandle>, String)> =
        entry_contexts
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
                // The arithmetic walk bounded the argument where the call
                // evaluates it, with declared ranges, guards and refuted arms.
                |argument| {
                    validation::expression_interval(expression_intervals, argument).is_some_and(
                        |interval| {
                            contains((
                                numerics::bignum::BigInt::from_i64(
                                    interval.low.unwrap_or(i64::MIN),
                                ),
                                numerics::bignum::BigInt::from_i64(
                                    interval.high.unwrap_or(i64::MAX),
                                ),
                            ))
                        },
                    )
                },
            );
            let by_guard = !by_context
                && incoming
                    .iter()
                    .filter(|guard| guard.applies_at(state_flow.state_symbol))
                    .any(|guard| {
                        let interval = if guard.is_negated() {
                            super::prover::has_builtin_operators(
                                program,
                                &facts.operators,
                                guard.guard(),
                            )
                            .then(|| {
                                super::intervals::negated_guard_interval_for_label(
                                    program,
                                    guard.guard(),
                                    &subject_label,
                                )
                            })
                            .flatten()
                        } else {
                            super::intervals::guard_interval_for_label(
                                program,
                                guard.guard(),
                                &subject_label,
                            )
                        };
                        interval.is_some_and(contains)
                            || guard.direct_arguments().is_some_and(|arguments| {
                                let renamed = instantiate_state_parameter_label(
                                    program,
                                    state,
                                    arguments,
                                    &subject_label,
                                );
                                let interval = if guard.is_negated() {
                                    super::intervals::negated_guard_interval_for_label(
                                        program,
                                        guard.guard(),
                                        &renamed,
                                    )
                                } else {
                                    super::intervals::guard_interval_for_label(
                                        program,
                                        guard.guard(),
                                        &renamed,
                                    )
                                };
                                interval.is_some_and(contains)
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
                    .filter(|guard| guard.applies_at(state_flow.state_symbol))
                    .any(|guard| {
                        if guard.is_negated() {
                            super::prover::has_builtin_operators(
                                program,
                                &facts.operators,
                                guard.guard(),
                            ) && (negated_guard_conjunct_matches(program, guard.guard(), required)
                                || guard.direct_arguments().is_some_and(|arguments| {
                                    let renamed = instantiate_state_parameter_label(
                                        program, state, arguments, required,
                                    );
                                    negated_guard_conjunct_matches(program, guard.guard(), &renamed)
                                }))
                        } else {
                            guard_conjunct_matches(program, guard.guard(), required)
                                || guard.direct_arguments().is_some_and(|arguments| {
                                    let renamed = instantiate_state_parameter_label(
                                        program, state, arguments, required,
                                    );
                                    guard_conjunct_matches(program, guard.guard(), &renamed)
                                })
                        }
                    });
                premise_used_guard |= proven;
                proven
            }
        });
    let arguments = validation::call_site_argument_expressions(program, &call_site);
    let mentioned = super::guard_operands::fact_requirement_mentions(program, facts, fact);
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
                &super::guard_operands::requirement_argument_reads(
                    arguments,
                    target_parameters,
                    &mentioned,
                ),
            ))
        // The statement scans above stop where the statement begins; the
        // jump's own operands run after them and before the membership's read.
        && super::guard_operands::requirement_reads_survive_earlier_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            arguments,
            target_parameters,
            &mentioned,
            call_frames,
        )
}

/// Whole-caller-state preservation for a premise established BEFORE this
/// Clear "needs fact X here" guidance for a proof-backed operator/contract that
/// is missing a required boolean fact (for example an index bound or a
/// domain-sensitive operator precondition). The caller has not established the
/// fact in the entry context, so name exactly what must hold before the call.
fn explain_missing_boolean_fact(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
            facts::domain_membership_symbol_pattern!(domain_symbol) => {
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                (domain_symbol, place)
            }
            _ => continue,
        };

        if (!facts.semantic.domain_implies(fact_domain, required_domain)
            && !validation::field_domains::domain_membership_implies(
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
    incoming: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some((_machine, state)) =
        program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)
    else {
        return false;
    };
    let Some((call_site, target_parameters)) =
        call_site_target_parameters(program, state_flow, call_flow)
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
        .filter(|guard| guard.applies_at(state_flow.state_symbol))
        .any(|guard| {
            if guard.is_negated() {
                // The continuation edge carries `!guard`: comparisons and
                // `||` disjunctions read as their complement, which only
                // holds under builtin operator semantics.
                super::prover::has_builtin_operators(program, &facts.operators, guard.guard())
                    && (negated_guard_conjunct_matches(program, guard.guard(), &required_label)
                        || guard.direct_arguments().is_some_and(|arguments| {
                            let instantiated = instantiate_state_parameter_label(
                                program,
                                state,
                                arguments,
                                &required_label,
                            );
                            negated_guard_conjunct_matches(program, guard.guard(), &instantiated)
                        }))
            } else {
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
            }
        });
    if !guard_matches {
        return false;
    }
    let mut fields: Vec<typed_trees::name::Identifier> = Vec::new();
    collect_expression_self_fields(program, expression, &mut fields);
    let arguments = validation::call_site_argument_expressions(program, &call_site);
    let mentioned = super::guard_operands::boolean_requirement_mentions(program, expression);
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
            &super::guard_operands::requirement_argument_reads(
                arguments,
                target_parameters,
                &mentioned,
            ),
        )
        // The statement scans above stop at the call; the jump's own operands
        // run after them and before the requirement's read.
        && super::guard_operands::requirement_reads_survive_earlier_operand_writes(
            program,
            facts,
            state_flow,
            call_flow,
            arguments,
            target_parameters,
            &mentioned,
            call_frames,
        )
}
