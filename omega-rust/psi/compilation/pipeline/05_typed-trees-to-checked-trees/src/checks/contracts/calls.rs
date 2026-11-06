mod boolean_requirements;
mod captured_boolean;
mod contradictory_edge;
mod guard_observations;
mod labels;
mod predicate_domains;
mod state_preservation;
mod transition_guards;
use labels::instantiate_state_parameter_label;
use predicate_domains::predicate_only_domain_predicates;
use state_preservation::{
    caller_state_preserves_field, caller_state_preserves_label_names,
    caller_state_preserves_self_fields_against_call_writes, collect_expression_self_fields,
};
use transition_guards::{
    guard_conjunct_matches, negated_guard_conjunct_matches, transition_guard_proves_requires,
};
pub(super) use transition_guards::{
    selected_guard_predicate, transition_guard_proves_boolean_requirement,
};

use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use diagnostics::Diagnostic;
use facts::{FactPayload, FactPlace};
use symbols::SymbolHandle;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::published::labels::{
    call_target_label, joined_place_label, machine_name, semantic_fact_requirement_label,
    symbol_name,
};

/// Whole-contract answers may depend on a completed alternative's exact
/// contexts. Share repeated questions only within that same captured premise
/// roster; never reuse one arm's positive result to qualify another arm.
struct WholeContractCache {
    contexts: Vec<facts::FactContextHandle>,
    answers: Vec<(arena::Handle<checked_trees::ContractProofFact>, bool)>,
}

pub(super) fn check_call_requires(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    nominal_requirements: &super::nominal_inputs::DeclaredFieldRequirements,
    incoming_guards: &[crate::checks::ranges::incoming_guards::IncomingGuard],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    constant_obligations: &mut Vec<checked_trees::ConstantPredicateObligation>,
    recipe_obligations: &mut Vec<checked_trees::ConstantRecipeObligation>,
    value_qualifications: &mut arena::Arena<checked_trees::CheckedValueQualification>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The state-parameter domain facts (origin StateParameterDomain, surfaced at
    // ProgramPoint::State by append_state_parameter_domain_facts) are folded into
    // the state entry and reach every call -- INCLUDING guarded-transition
    // fallthrough arms -- through the flow's context threading, now that a
    // transition's branch-taken exit context is no longer leaked onto its sibling
    // fallthrough (see flow/statements.rs). No direct-context consultation needed.
    let entry_contexts: Vec<_> = phase_profile::scope(
        "psi.05.check_checked_facts.check_flow_call_contracts.entry_contexts",
        || {
            facts
                .flow
                .state_call_entry_semantic_contexts(
                    state_flow,
                    call_flow.statement_index,
                    call_flow.call_ordinal,
                    call_flow.target_symbol,
                    call_flow.receiver_symbol,
                )
                .collect()
        },
    );
    phase_profile::scope(
        "psi.05.check_checked_facts.check_flow_call_contracts.contextual_cleanup",
        || {
            super::contextual_cleanup::check(
                program,
                facts,
                state_flow,
                call_flow,
                &entry_contexts,
                call_frames,
                diagnostics,
            );
        },
    );
    phase_profile::scope(
        "psi.05.check_checked_facts.check_flow_call_contracts.nominal_inputs",
        || {
            super::nominal_inputs::check(
                program,
                facts,
                state_flow,
                call_flow,
                nominal_requirements,
                diagnostics,
            );
        },
    );
    phase_profile::scope(
        "psi.05.check_checked_facts.check_flow_call_contracts.lender_domains",
        || {
            check_lender_domain_restoration(program, facts, state_flow, call_flow, diagnostics);
        },
    );
    let mut whole_contract_caches = Vec::<WholeContractCache>::new();
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let membership = phase_profile::scope(
                "psi.05.check_checked_facts.check_flow_call_contracts.membership",
                || {
                    super::call_values::membership_evidence(
                        program, facts, state_flow, call_flow, fact,
                    )
                },
            );
            let mut proves_case = |case: Option<&super::call_values::MembershipCase>| {
                let requirement_contexts =
                    case.map_or(entry_contexts.as_slice(), |case| case.contexts.as_slice());
                if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some()
                    && let Some((_, _, domain, _)) = fact.payload.domain_membership()
                    && let Some(place) = fact.place.as_place()
                {
                    trace_requirement_contexts(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        if case.is_some() {
                            "captured-case"
                        } else {
                            "entry"
                        },
                        requirement_contexts,
                        place,
                        domain,
                    );
                }
                let cache_index = whole_contract_caches
                    .iter()
                    .position(|cache| cache.contexts.as_slice() == requirement_contexts)
                    .unwrap_or_else(|| {
                        whole_contract_caches.push(WholeContractCache {
                            contexts: requirement_contexts.to_vec(),
                            answers: Vec::new(),
                        });
                        whole_contract_caches.len() - 1
                    });
                let captured_judgment = phase_profile::scope(
                    "psi.05.check_checked_facts.check_flow_call_contracts.captured_boolean",
                    || {
                        captured_boolean::judgment(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            requirement_contexts,
                            fact,
                            call_frames,
                            &mut whole_contract_caches[cache_index].answers,
                        )
                    },
                );
                let satisfied = phase_profile::scope(
                    "psi.05.check_checked_facts.check_flow_call_contracts.proves_core",
                    || {
                        selected_evidence_proves_requirement(facts, state_flow, call_flow, fact)
                            || captured_judgment.unwrap_or_else(|| match fact.payload {
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
                                        facts,
                                        facts::ProgramPoint::Statement {
                                            machine_symbol: state_flow.machine_symbol,
                                            state_symbol: state_flow.state_symbol,
                                            statement_index: call_flow.statement_index,
                                        },
                                        requirement_contexts,
                                        fact,
                                    ) || transparent_requirement_proves(
                                        program, facts, state_flow, call_flow, fact,
                                    )
                                }
                            })
                    },
                );
                // ch8 construction-grant: a string literal whose compile-time bytes
                // satisfy a domain's declared byte-predicate fact grants that
                // domain without a validating boundary call -- this is how a literal
                // flows into a `&[u8] in Utf8` target.
                let satisfied = satisfied
                    || phase_profile::scope(
                        "psi.05.check_checked_facts.check_flow_call_contracts.domain_grants",
                        || {
                            (captured_judgment.is_none()
                                && proven_predicates_grant_domain(
                                    program,
                                    facts,
                                    state_flow,
                                    call_flow,
                                    requirement_contexts,
                                    fact,
                                    call_frames,
                                ))
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
                                    ) || qualified_argument_grants_domain(
                                        program,
                                        facts,
                                        requirement_contexts,
                                        fact,
                                        membership
                                            .as_ref()
                                            .and_then(|evidence| evidence.whole_argument),
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
                                        case.and_then(|case| case.referents.as_deref()),
                                        call_frames,
                                    )))
                        },
                    );

                let satisfied = satisfied
                    || crate::checks::termination::profile_requirement_covered(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        fact,
                        call_frames,
                    );
                let accepted = satisfied
                    && super::prover::contract_membership_place_is_accessible(
                        program,
                        &facts.semantic,
                        requirement_contexts,
                        fact,
                    );
                if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some()
                    && fact.payload.domain_membership().is_some()
                {
                    eprintln!(
                        "CALL_WRITE membership-case-verdict machine={:?} state={:?} stmt={} ordinal={} target={:?} requirement={:?} place={:?} contexts={requirement_contexts:?} captured_judgment={captured_judgment:?} satisfied={satisfied} access_checked={satisfied} final_result={accepted}",
                        state_flow.machine_symbol,
                        state_flow.state_symbol,
                        call_flow.statement_index,
                        call_flow.call_ordinal,
                        call_flow.target_symbol,
                        fact.payload,
                        fact.place
                    );
                }
                accepted
            };
            // Every possible completed actual must establish the requirement
            // independently. Combining arm facts would invent a value that no
            // execution captured; one successful arm cannot qualify the join.
            let satisfied = match membership.as_ref() {
                None => proves_case(None),
                Some(evidence) => {
                    !evidence.cases.is_empty()
                        && evidence.cases.iter().all(|case| proves_case(Some(case)))
                }
            };
            // Preserve the successful judgment at the captured operand, not
            // on its source home or every later use of the same expression.
            // Terminal publication consumes this ordinary value-formation row;
            // the receiving signature alone never supplies membership.
            if satisfied
                && let Some(expression) = membership
                    .as_ref()
                    .and_then(|evidence| evidence.whole_argument)
                && let Some((_, _, domain_symbol, semantic_domain)) =
                    fact.payload.domain_membership()
                && let Ok(statement_index) = u32::try_from(call_flow.statement_index)
            {
                value_qualifications.append(checked_trees::CheckedValueQualification {
                    machine: state_flow.machine_symbol,
                    state: state_flow.state_symbol,
                    statement_index,
                    expression,
                    semantic_domain,
                    domain_symbol,
                    structural_value: checked_trees::CheckedStructuralValueHandle::invalid(),
                });
            }
            if !satisfied {
                if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some()
                    && let Some((_, _, domain, _)) = fact.payload.domain_membership()
                    && let Some(place) = fact.place.as_place()
                {
                    trace_requirement_contexts(
                        program,
                        facts,
                        state_flow,
                        call_flow,
                        "failing-call-entry",
                        &entry_contexts,
                        place,
                        domain,
                    );
                    if let Some(evidence) = membership.as_ref() {
                        for (index, case) in evidence.cases.iter().enumerate() {
                            eprintln!(
                                "CALL_WRITE membership-case-inventory machine={:?} state={:?} stmt={} ordinal={} target={:?} requirement={:?} place={:?} index={index} referents={:?}",
                                state_flow.machine_symbol,
                                state_flow.state_symbol,
                                call_flow.statement_index,
                                call_flow.call_ordinal,
                                call_flow.target_symbol,
                                fact.payload,
                                fact.place,
                                case.referents
                            );
                            trace_requirement_contexts(
                                program,
                                facts,
                                state_flow,
                                call_flow,
                                "failing-captured-case-inventory",
                                &case.contexts,
                                place,
                                domain,
                            );
                        }
                    }
                }
                if let FactPayload::ContractBooleanExpression { expression, .. } = fact.payload
                    && super::prover::has_builtin_operators(program, &facts.operators, expression)
                    && let Some(parameters) =
                        validation::call_target_parameters(program, call_flow.target_symbol)
                    && let Some(arguments) = super::call_values::captured_argument_expressions(
                        program, facts, state_flow, call_flow,
                    )
                    && parameters.len() == arguments.len()
                    && let Some(obligations) = super::argument_intervals::recipe_requirements(
                        program,
                        state_flow.machine_symbol,
                        program
                            .machine_by_own_or_state_symbol(call_flow.target_symbol)
                            .map_or(call_flow.target_symbol, |machine| machine.symbol),
                        expression,
                        &|subject| {
                            let name = program.expression_table.name_path_node(subject)?;
                            if !program.expression_table.name_path_is_single_member(name) {
                                return None;
                            }
                            let position = parameters
                                .iter()
                                .position(|parameter| parameter.symbol == name.symbol)?;
                            program
                                .primitive_type_reference(parameters[position].type_reference)
                                .filter(|primitive| primitive.accepts_integer_literal())?;
                            arguments.get(position).copied()
                        },
                        &format!(
                            "requires contract for call {} from {} state {} statement {}",
                            call_target_label(program, call_flow.target_symbol),
                            machine_name(program, state_flow.machine_symbol),
                            symbol_name(program, state_flow.state_symbol),
                            call_flow.statement_index,
                        ),
                    )
                {
                    recipe_obligations.extend(obligations);
                    continue;
                }
                if let FactPayload::ContractBooleanExpression { expression, .. } = fact.payload
                    && let Some((target_machine, target_state)) =
                        validation::find_state(program, call_flow.target_symbol)
                    && let Some(obligation) = checked_trees::ConstantPredicateObligation::retain(
                        program,
                        expression,
                        target_machine.symbol,
                        state_flow.machine_symbol,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
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
                let requirement = present_captured_parameter(&semantic_fact_requirement_label(
                    program,
                    &facts.semantic,
                    fact,
                ));
                let authored_requirement = match fact.payload {
                    FactPayload::ContractBooleanExpression { expression, .. } => {
                        authored_requirement_label(program, state_flow, call_flow, expression)
                            .filter(|authored| authored != &requirement)
                    }
                    _ => None,
                };
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
                let diagnostic = Diagnostic::error(format!(
                    "cannot prove requires contract for call {} from {}: {}{} in state `{}`{}",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    requirement,
                    detail
                        .map(|message| format!(" ({message})"))
                        .unwrap_or_default(),
                    symbol_name(program, state_flow.state_symbol),
                    authored_requirement
                        .map(|authored| format!("; authored requirement `{authored}`"))
                        .unwrap_or_default()
                ));
                diagnostics.push(match call_source_span(program, state_flow, call_flow) {
                    Some(span) => diagnostic.with_source_span(span),
                    None => diagnostic,
                });
            }
        }
    }
}

/// A captured generic parameter is a member of a saved subject. The authored
/// requirement names that parameter. The capture carrier is not part of it.
fn present_captured_parameter(label: &str) -> String {
    let marker = "<saved subject #";
    let mut rest = label;
    let mut presented = String::new();
    while let Some(start) = rest.find(marker) {
        presented.push_str(&rest[..start]);
        let after_marker = &rest[start + marker.len()..];
        let Some(close) = after_marker.find('>') else {
            presented.push_str(&rest[start..]);
            return presented;
        };
        let identity = &after_marker[..close];
        let tail = &after_marker[close + 1..];
        if identity
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b':')
            && let Some(stripped) = tail.strip_prefix('.')
        {
            rest = stripped;
            continue;
        }
        presented.push_str(&rest[start..start + marker.len() + close + 1]);
        rest = tail;
    }
    presented.push_str(rest);
    presented
}

/// The authored invocation a requires diagnostic should point at: the call
/// expression the traversal captured, else the authored span of the statement
/// at the call's coordinate (a call statement or named transition, which have
/// no expression handle). Compiler-generated occurrences keep the default
/// span, so they yield `None` rather than an invented location.
fn call_source_span(
    program: &dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<source::SourceSpan> {
    if call_flow.authored_expression.is_valid() {
        return program
            .expression_table
            .try_source_span(call_flow.authored_expression)
            .filter(|span| *span != source::SourceSpan::default());
    }
    let (_, state) = validation::find_state(program, state_flow.state_symbol)?;
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(call_flow.statement_index)?;
    validation::statement_source_span(program, statement)
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
    let Some(subject) = crate::flow::canonical_place_from_semantic_place(
        program,
        &facts.semantic,
        facts.semantic.places.get(place_handle),
    ) else {
        return false;
    };
    super::prover::prove_domain_at_place(
        program,
        &facts.semantic,
        entry_contexts,
        &subject,
        domain_symbol,
    )
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
    let (_, _, domain_symbol, _) = payload.domain_membership()?;
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

/// Every requirement route's static coordinates for one call occurrence: the
/// caller's own machine and state, the exact invocation site inside it, and
/// the callee's declared parameters. The routes used to spell this lookup
/// chain themselves, one gate at a time, so one resolver keeps them agreeing
/// about which lookups must succeed before a proof may run.
fn call_coordinates<'p>(
    program: &'p dyn ::validation::CheckingInput,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
) -> Option<(
    &'p typed_trees::machine::Machine,
    &'p typed_trees::state::State,
    validation::CallSite<'p>,
    &'p [typed_trees::signature::StateParameter],
)> {
    let (machine, state) = validation::machine_state_by_symbols(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    )?;
    let call_site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )?;
    let target_parameters = validation::call_target_parameters(program, call_flow.target_symbol)?;
    Some((machine, state, call_site, target_parameters))
}

/// Diagnostics retain the caller's operand spelling beside the canonical
/// backing label. Normalizing a loan to its backing is useful to the prover,
/// but must not hide which authored view owes the current-content obligation.
/// This rendering supplies no evidence and never participates in proof.
fn authored_requirement_label(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    call: &FlowCallFact,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    use typed_trees::proposition::ProofSubstitutions;
    let (_, _, site, parameters) = call_coordinates(program, state, call)?;
    let arguments = validation::call_site_argument_expressions(program, &site);
    if arguments.len() != parameters.len() {
        return None;
    }
    let substitutions: Vec<_> = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, &argument)| {
            let argument = match program.expression_table.expression(argument) {
                typed_trees::expression::ExpressionNode::Borrow(borrow)
                    if validation::reference_shell(program, parameter.type_reference).is_some() =>
                {
                    borrow.target
                }
                _ => argument,
            };
            (
                parameter.symbol,
                parameter.name.to_string(),
                program.render_proof_expression(argument, ProofSubstitutions::None),
            )
        })
        .collect();
    Some(
        program
            .render_proof_expression(expression, ProofSubstitutions::ByParameter(&substitutions)),
    )
}

fn proven_predicates_grant_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let facts::FactPayload::ContractDomainMembership {
        value,
        domain_symbol,
        semantic_domain,
        ..
    } = fact.payload
    else {
        return false;
    };
    let Some(domain) = program.domain_definition_for_instance(domain_symbol, semantic_domain)
    else {
        return false;
    };
    let Some(predicates) = predicate_only_domain_predicates(program, domain) else {
        return false;
    };
    let Some(subject) = validation::canonical_place_from_expression(program, value) else {
        return false;
    };
    // The declaration supplies goals, never premises. Bind its subject to the
    // exact formal, then use the same captured actuals and observation epochs
    // as an ordinary Boolean contract. Display labels cannot identify storage
    // or transport a named state's parameters through a constructor.
    let Some(goals) = crate::published::facts::boolean_predicates::domain_predicates_for_subject(
        program,
        &facts.operators,
        domain,
        &subject,
    ) else {
        return false;
    };
    goals.len() == predicates.len()
        && goals.iter().all(|goal| {
            super::call_values::proves_bound_predicate(
                program,
                facts,
                state_flow,
                call_flow,
                entry_contexts,
                goal,
                call_frames,
                &|observation| {
                    super::call_values::observation_actual(
                        program,
                        state_flow,
                        call_flow,
                        observation,
                    )
                },
            )
        })
}

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

// A past removal may have been followed by a valid restoration. Show the
// actual premise roster at this check, separately for each captured operand.
fn trace_requirement_contexts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    role: &str,
    contexts: &[facts::FactContextHandle],
    required_place: facts::PlaceHandle,
    required_domain: SymbolHandle,
) {
    eprintln!(
        "CALL_WRITE membership-cut machine={:?} state={:?} stmt={} ordinal={} target={:?} role={role} required_place={required_place:?} required_domain={required_domain:?} contexts={contexts:?}",
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
        call.target_symbol
    );
    for handle in contexts {
        let context = facts.semantic.contexts.get(*handle);
        let references = facts.semantic.refs.span_or_empty(context.facts);
        let roster: Vec<_> = references.iter().map(|reference| reference.fact).collect();
        eprintln!(
            "CALL_WRITE membership-context handle={handle:?} point={:?} roster={roster:?}",
            context.point
        );
        for reference in references {
            let current = facts.semantic.facts.get(reference.fact);
            let Some((_, _, domain, _)) = current.payload.domain_membership() else {
                continue;
            };
            let Some(place) = current.place.as_place() else {
                continue;
            };
            let same_place = facts.semantic.places_match(program, place, required_place);
            let same_domain = facts.semantic.domain_implies(domain, required_domain)
                || validation::field_domains::domain_membership_implies(
                    program,
                    domain,
                    required_domain,
                );
            eprintln!(
                "CALL_WRITE membership-current machine={:?} state={:?} stmt={} ordinal={} target={:?} role={role} required_place={required_place:?} required_domain={required_domain:?} context={handle:?} fact={:?} point={:?} place={:?} payload={:?} same_place={same_place} same_domain={same_domain}",
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
                call.target_symbol,
                reference.fact,
                current.point,
                current.place,
                current.payload
            );
        }
    }
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
            facts::domain_membership_pattern!(domain_symbol) => {
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

        if std::env::var_os("OMEGA_CALL_WRITE_TRACE").is_some() {
            eprintln!(
                "CALL_WRITE membership-historical-removal machine={:?} state={:?} stmt={} ordinal={} target={:?} required_place={required_place:?} required_domain={required_domain:?} source={:?} context={:?} context_point={:?} fact={:?} fact_point={:?} fact_place={:?} payload={:?}",
                state_flow.machine_symbol,
                state_flow.state_symbol,
                call_flow.statement_index,
                call_flow.call_ordinal,
                call_flow.target_symbol,
                invalidation.source,
                invalidation.context,
                facts.semantic.contexts.get(invalidation.context).point,
                invalidation.fact,
                fact.point,
                fact.place,
                fact.payload
            );
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
    let Some((_machine, state, call_site, target_parameters)) =
        call_coordinates(program, state_flow, call_flow)
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

/// A selected Type witness already inhabited this exact requires lane after
/// completed-operand substitution in `evidence`. Re-rendering its subject as a
/// current storage label would discard the call snapshot and reject preserved
/// copies (or accept a later write). This consumes only the bound lane of this
/// exact call; it imports no unconditional proposition or runtime authority.
fn selected_evidence_proves_requirement(
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &facts::Fact,
) -> bool {
    let FactPayload::ContractPropositionApplication { fact, .. } = requirement.payload else {
        return false;
    };
    facts.proof.contract_calls.iter().any(|(_, candidate)| {
        candidate.caller_machine_symbol == state.machine_symbol
            && candidate.caller_state_symbol == state.state_symbol
            && candidate.statement_index == call.statement_index
            && candidate.call_ordinal == call.call_ordinal
            && candidate.target_state_symbol == call.target_symbol
            && facts
                .proof
                .contract_evidence_arguments
                .span_or_empty(candidate.evidence_arguments)
                .iter()
                .any(|binding| {
                    facts.proof.outcome_specific_arms.iter().any(|(_, arm)| {
                        arm.rows
                            .iter()
                            .any(|row| row.selected_term == Some(binding.source))
                    }) && facts
                        .proof
                        .contract_fact_refs
                        .span_or_empty(candidate.requires)
                        .iter()
                        .any(|reference| {
                            let contract = facts.proof.contract_facts.get(reference.fact);
                            contract.fact == fact
                                && contract.evidence_term == Some(binding.parameter)
                        })
                })
    })
}

/// An argument written `x as T in D` names its operand's storage place, while
/// flow publishes the checked qualification's membership on the cast itself.
/// That membership establishes the formal's requirement for this exact value.
fn qualified_argument_grants_domain(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    contexts: &[facts::FactContextHandle],
    requirement: &facts::Fact,
    argument: Option<typed_trees::expression::ExpressionHandle>,
) -> bool {
    let Some(argument) = argument else {
        return false;
    };
    let Some((_, _, domain_symbol, semantic_domain)) = requirement.payload.domain_membership()
    else {
        return false;
    };
    let qualified = matches!(
        program.expression_table.expression(argument),
        typed_trees::expression::ExpressionNode::Cast(cast) if !cast.semantic_domain.is_empty()
    );
    qualified
        && super::exact_scalar_membership(
            program,
            facts,
            contexts,
            &validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(argument),
                segments: Vec::new(),
            },
            domain_symbol,
            semantic_domain,
        )
}

/// A local declared with a domain qualification (`let mut bytes: [u8; 2] in Ascii`)
/// retains that obligation across exclusive loans (`&mut`, `&write`).
/// If a callee mutates the lent storage without an `ensures` clause restoring
/// the declared domain, the call statement's exit contexts cannot prove the
/// declared domain for that storage, rendering the call inadmissible.
fn check_lender_domain_restoration(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parameters) = validation::call_target_parameters(program, call_flow.target_symbol)
    else {
        return;
    };
    if !parameters.iter().any(|p| {
        validation::reference_shell(program, p.type_reference)
            .is_some_and(|shell| shell.access.is_exclusive())
    }) {
        return;
    }
    let Some(site) = validation::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return;
    };
    let arguments = validation::call_site_argument_expressions(program, &site);
    let receiver_offset = usize::from(
        matches!(&site, validation::CallSite::Expression { call, .. } if call.receiver.is_valid()),
    );
    let Some(state) = validation::find_state_in_machine(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
    ) else {
        return;
    };
    let exit_contexts: Vec<_> = {
        let call_contexts: Vec<_> = facts
            .flow
            .contexts
            .semantic_context_refs
            .span_or_empty(call_flow.exit_semantic_contexts)
            .iter()
            .map(|c| c.context)
            .collect();
        if !call_contexts.is_empty() {
            call_contexts
        } else {
            facts
                .flow
                .state_statement(state_flow, call_flow.statement_index)
                .map(|s| {
                    facts
                        .flow
                        .contexts
                        .semantic_context_refs
                        .span_or_empty(s.exit_semantic_contexts)
                        .iter()
                        .map(|c| c.context)
                        .collect()
                })
                .unwrap_or_default()
        }
    };

    let mut checked_obligations = Vec::new();
    for (position, parameter) in parameters.iter().enumerate() {
        if !validation::reference_shell(program, parameter.type_reference)
            .is_some_and(|shell| shell.access.is_exclusive())
        {
            continue;
        }
        let argument = arguments.get(position).copied();
        let operand_position = position + receiver_offset;
        let actual = super::call_values::Actual {
            parameter,
            argument,
            path: Vec::new(),
            operand_position,
        };
        let Some(captured_values) =
            super::call_values::captured_values(program, facts, state_flow, call_flow, &actual)
        else {
            continue;
        };
        for captured in captured_values {
            let mut places = Vec::new();
            if let Some(obs_places) =
                crate::flow::captured_observation_places(program, &facts.flow, &captured)
            {
                places.extend(obs_places);
            }
            if let Some(val_places) =
                crate::flow::captured_value_places(program, &facts.flow, &captured)
            {
                places.extend(val_places);
            }
            for place in places {
                let Some(symbol) = place.storage.root.as_symbol() else {
                    continue;
                };
                let Some(local_type) = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .find_map(|s| match s {
                        typed_trees::statement::StatementNode::LocalData(l)
                            if l.symbol == symbol =>
                        {
                            Some(l.type_reference)
                        }
                        _ => None,
                    })
                else {
                    continue;
                };
                for row in validation::field_domains::declared_type_domain_rows(program, local_type)
                {
                    if row.transfer {
                        continue;
                    }
                    let overlaps = row.path.starts_with(&place.storage.segments)
                        || place.storage.segments.starts_with(&row.path);
                    if !overlaps {
                        continue;
                    }
                    let domain_symbol = row.domain_symbol;
                    let semantic_domain = row.semantic_domain;
                    let required_place = validation::CanonicalPlace {
                        root: place.storage.root,
                        segments: row.path.clone(),
                    };
                    if checked_obligations.contains(&(required_place.clone(), domain_symbol)) {
                        continue;
                    }
                    checked_obligations.push((required_place.clone(), domain_symbol));

                    let has_membership = exit_contexts.iter().any(|&context_handle| {
                        let context = facts.semantic.contexts.get(context_handle);
                        facts.semantic.context_view(context).facts().any(|fact| {
                            let (
                                FactPlace::Place(fact_place),
                                FactPayload::DomainMembership {
                                    domain_symbol: f_dom,
                                    semantic_domain: f_id,
                                    ..
                                },
                            ) = (fact.place, fact.payload)
                            else {
                                return false;
                            };
                            if f_dom != domain_symbol {
                                return false;
                            }
                            if semantic_domain.is_valid() && f_id != semantic_domain {
                                return false;
                            }
                            let fact_place = facts.semantic.places.get(fact_place);
                            crate::flow::canonical_place_from_semantic_place(
                                program,
                                &facts.semantic,
                                fact_place,
                            )
                            .as_ref()
                                == Some(&required_place)
                        })
                    });
                    let satisfied = has_membership
                        || if semantic_domain.is_valid() {
                            super::prover::prove_indexed_domain_at_place(
                                program,
                                &facts.semantic,
                                &exit_contexts,
                                &required_place,
                                domain_symbol,
                                semantic_domain,
                            )
                        } else {
                            super::prover::prove_domain_at_place(
                                program,
                                &facts.semantic,
                                &exit_contexts,
                                &required_place,
                                domain_symbol,
                            )
                        };
                    if !satisfied {
                        let diagnostic = Diagnostic::error(format!(
                            "cannot prove default-domain field requirement for call {} from {}::{}: parameter {} requires {}",
                            call_target_label(program, call_flow.target_symbol),
                            machine_name(program, state_flow.machine_symbol),
                            symbol_name(program, state_flow.state_symbol),
                            facts::canonical_place_label_from_parts(
                                program,
                                facts::PlaceRoot::Symbol(parameter.symbol),
                                &row.path,
                            ),
                            symbol_name(program, domain_symbol),
                        ));
                        diagnostics.push(match call_source_span(program, state_flow, call_flow) {
                            Some(span) => diagnostic.with_source_span(span),
                            None => diagnostic,
                        });
                    }
                }
            }
        }
    }
}
