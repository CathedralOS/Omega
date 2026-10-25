//! A call guarantee and a local result binding have separate lifetimes. Join
//! the surviving guarantee to surviving AssignedValue provenance, then compare
//! predicates under exact declaration/call substitution. Never replay a local
//! initializer or identify repeated calls by their printed arguments.
//! Machine entries, named state arrivals, and requirement signatures share this
//! substitution; `callable` retains their distinct parameter/result scopes.
//! A public requirement guarantee is not a proof of its provider's body.
//! A named arrival substitutes only its own state contract; it cannot revive
//! machine-entry assumptions. Every input substitutes through its explicit
//! argument position and exact declaration-owned projection.
//!
//! This consumes the existing flow evidence rather than copying predicates at
//! each assignment. Storage dependencies retire changed inputs; assignment
//! provenance retires changed results. Inputs retain their exact completed
//! operand and declared carrier. Detached stable values can name opaque
//! computations without replay; live views still owe storage preservation.
//! Expression-root dependencies alone do not establish referent identity.

use crate::facts::boolean_predicates::{self, BoundPredicate};

use validation::CallSite;
use validation::CanonicalPlace;

use checked_trees::{CheckedOperatorFacts, FlowCallFact, FlowStateFact};
use facts::{FactPayload, FactPlan, PlaceRoot};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

pub(in crate::checks) mod arithmetic;
pub(in crate::checks) mod callable;
use callable::Callable;
mod availability;
mod static_operands;
pub(crate) use availability::GuaranteeUse;
pub(in crate::checks) use availability::{AvailableGuarantee, available};

struct Invocation<'program> {
    site: CallSite<'program>,
    callable: Callable<'program>,
    caller_state: SymbolHandle,
    statement: usize,
    ordinal: usize,
}

fn invocation_fact<'facts>(
    facts: &'facts checked_trees::CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
) -> Option<&'facts checked_trees::FlowCallFact> {
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(caller.calls)
        .iter()
        .filter(|call| {
            call.statement_index == invocation.statement && call.call_ordinal == invocation.ordinal
        });
    let call = calls.next()?;
    calls.next().is_none().then_some(call)
}

/// Declaration meaning and observation custody are independent. Invocation
/// and exit consumers supply exact value bindings to the same matcher.
struct PredicateScope<'program, 'bindings> {
    callable: Callable<'program>,
    literal: &'bindings dyn Fn(ExpressionHandle) -> Option<facts::ScalarValue>,
    place: &'bindings dyn Fn(ExpressionHandle) -> Option<CanonicalPlace>,
}

/// Match a current-storage obligation with an independently available normal
/// guarantee. The caller authenticates its declaration projections; the common
/// matcher joins them through surviving completed-value provenance.
pub(in crate::checks) fn proves_current(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
    callable: Callable<'_>,
    expression: ExpressionHandle,
    place: &dyn Fn(ExpressionHandle) -> Option<CanonicalPlace>,
) -> bool {
    if !super::has_builtin_operators(program, &facts.operators, expression)
        || !callable.builtin_meaning(program, expression)
    {
        return false;
    }
    let required = PredicateScope {
        callable,
        literal: &|expression| crate::flow::literal(program, expression),
        place,
    };
    let guarantees = available(
        program,
        facts,
        caller,
        GuaranteeUse::StatementEntry(statement),
        contexts,
        frames,
    );
    guarantees.into_iter().any(|guarantee| {
        let supplied = PredicateScope {
            callable: guarantee.invocation.callable,
            literal: &|expression| bound_literal(program, &guarantee.invocation, expression),
            place: &|expression| bound_place(program, &guarantee.invocation, expression),
        };
        predicates_match(
            program,
            &facts.semantic,
            contexts,
            &required,
            expression,
            &supplied,
            guarantee.expression,
            &mut Vec::new(),
        )
    })
}

/// Read surviving normal guarantees in their authenticated caller namespace.
/// Parameters observe current storage; results name completed occurrences.
/// Availability owns declaration identity, actual capture and write preservation.
/// Keep storage bindings intact for write retirement; completed-result joins
/// belong to proof scratch after each consumer has selected its live contexts.
pub(crate) fn current_predicates(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    use_point: GuaranteeUse,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
) -> Vec<BoundPredicate> {
    available(program, facts, caller, use_point, contexts, frames)
        .into_iter()
        .filter_map(|guarantee| {
            let invocation = &guarantee.invocation;
            let parameters = invocation.callable.parameters(program);
            let expression = crate::values::lower_callable_scalar_contract_predicate(
                program,
                &facts.operators,
                invocation.callable.owner_symbol(),
                parameters,
                invocation.callable.result_type(),
                guarantee.expression,
                true,
                &mut 4096,
            )?;
            let result = guarantee
                .call_expression()
                .map(|expression| CanonicalPlace {
                    root: PlaceRoot::Expression(expression),
                    segments: Vec::new(),
                });
            let mut predicate = boolean_predicates::bind_parameters(
                program,
                invocation.callable.owner_symbol(),
                invocation.callable.target_symbol(),
                parameters,
                result
                    .as_ref()
                    .map(|place| (place, invocation.callable.result_type())),
                expression,
            )?;
            let arguments = validation::call_site_argument_expressions(program, &invocation.site);
            for binding in &mut predicate.bindings {
                // The normal result is already in the caller's completed-value
                // namespace. Only this exact invocation root bypasses substitution;
                // a parameter or another call must still establish its own capture.
                if result
                    .as_ref()
                    .is_some_and(|result| binding.place.root == result.root)
                {
                    continue;
                }
                let position = parameters.iter().position(|parameter| {
                    binding.place.root == PlaceRoot::Symbol(parameter.symbol)
                })?;
                let actual = *arguments.get(position)?;
                let place = crate::semantic::facts::projected_argument_place(
                    program,
                    caller.state_symbol,
                    invocation.statement,
                    actual,
                    parameters[position].type_reference,
                    &binding.place.segments,
                )?;
                binding.place = crate::flow::rebase_exact_local_place(
                    program,
                    caller.state_symbol,
                    invocation.statement,
                    place,
                    Some(frames),
                )?;
            }
            Some(predicate)
        })
        .collect()
}

/// Join whole checked formulas through surviving completed-value provenance.
/// A named receiver and its producer's result are the same observation only
/// while the current contexts retain that exact assignment. This is proof
/// scratch: stored predicates keep their storage namespace so later writes
/// still invalidate them. In particular a replacement's incoming result cannot
/// become the pre-store destination, and a retired assignment is never replayed.
pub(crate) fn proves_captured_predicates(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    predicates: &[BoundPredicate],
    required: &BoundPredicate,
    mut ordinary: impl FnMut(&CanonicalPlace) -> Option<facts::ScalarValue>,
) -> bool {
    fn retain_value(
        values: &mut Vec<(CanonicalPlace, Option<facts::ScalarValue>)>,
        place: &CanonicalPlace,
        value: facts::ScalarValue,
    ) {
        if let Some((_, known)) = values.iter_mut().find(|(known, _)| known == place) {
            if *known != Some(value) {
                *known = None;
            }
        } else {
            values.push((place.clone(), Some(value)));
        }
    }

    let mut values: Vec<(CanonicalPlace, Option<facts::ScalarValue>)> = Vec::new();
    let mut bind = |predicate: &BoundPredicate| {
        let mut predicate = predicate.clone();
        for binding in &mut predicate.bindings {
            // Resolve before rebasing: cleanup's write fence names actual
            // storage, not the historical result occurrence. Conflicting
            // snapshots supply no ordinary value to the common algebra.
            let value = ordinary(&binding.place);
            if let Some(place) = captured_place(program, semantic, contexts, binding.place.clone())
            {
                binding.place = place;
            }
            if let Some(value) = value {
                retain_value(&mut values, &binding.place, value);
            }
        }
        predicate
    };
    let predicates = predicates.iter().map(&mut bind).collect::<Vec<_>>();
    let required = bind(required);
    // Boolean closure keeps integer comparisons as whole propositions. Use the
    // existing positive equation reader to compose authenticated conclusions
    // such as supplied.code == 7 and result.code == supplied.code. Every round
    // reads a frozen map; only previously unknown, already bound observations
    // become known. The required goal supplies no equation, and an unseeded
    // cycle establishes nothing. Any disagreement rejects the complete query
    // so descendants of an inconsistent value cannot survive as evidence.
    if values.iter().any(|(_, value)| value.is_none()) {
        return false;
    }
    let binding_count = predicates
        .iter()
        .map(|predicate| predicate.bindings.len())
        .sum::<usize>();
    for _ in 0..=binding_count {
        let prior = values.clone();
        for predicate in &predicates {
            for binding in &predicate.bindings {
                if let Some(value) = predicate.constant_at_place(&binding.place, |place| {
                    prior
                        .iter()
                        .find(|(known, _)| known == place)
                        .and_then(|(_, value)| value.clone())
                }) {
                    retain_value(&mut values, &binding.place, value);
                    if values.iter().any(|(_, value)| value.is_none()) {
                        return false;
                    }
                }
            }
        }
        if values.len() == prior.len() {
            return boolean_predicates::proves(&predicates, &required, |place| {
                values
                    .iter()
                    .find(|(known, _)| known == place)
                    .and_then(|(_, value)| value.clone())
            });
        }
    }
    false
}

/// Read an exact scalar conclusion after authenticating its declaration,
/// argument substitution and surviving capture. This does not turn a raw
/// `ensures` expression into a caller fact or choose a side of a disjunction.
pub(in crate::checks) fn scalar_at_place(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
    place: &CanonicalPlace,
) -> Option<facts::ScalarValue> {
    current_predicates(
        program,
        facts,
        caller,
        GuaranteeUse::StatementEntry(statement),
        contexts,
        frames,
    )
    .iter()
    .find_map(|predicate| {
        predicate.constant_at_place(place, |operand| {
            crate::values::scalar_value_at_place(
                program,
                &facts.semantic,
                contexts
                    .iter()
                    .map(|context| facts.semantic.contexts.get(*context)),
                operand,
            )
        })
    })
}
pub(in crate::checks::contracts) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let operators = &facts.operators;
    let semantic = &facts.semantic;
    let Some(required) = invocation(program, caller, call.statement_index, call.call_ordinal)
    else {
        return false;
    };
    let Some(frames) = frames else {
        return false;
    };
    if !stable_predicate_inputs(program, facts, caller, &required, expression)
        || !builtin_predicate(program, operators, &required, expression)
    {
        return false;
    }
    let mut arithmetic_hypotheses = Vec::new();
    let exact = available(
        program,
        facts,
        caller,
        // These are the exact call-entry contexts after its operands, not
        // statement entry. Context membership excludes this call's own and
        // later guarantees while admitting completed nested operands.
        GuaranteeUse::EvaluatedContext(call.statement_index),
        contexts,
        frames,
    )
    .into_iter()
    .any(|guarantee| {
        if let Some(post_return_writes) = guarantee.post_return_writes.as_deref()
            && let Some(proposition) = arithmetic::at_invocation(
                program,
                facts,
                caller,
                &guarantee.invocation,
                guarantee.expression,
                post_return_writes,
                frames,
            )
        {
            arithmetic_hypotheses.push(validation::ScopedArithmeticHypothesis {
                proposition,
                holds: true,
            });
        }
        let required_scope = PredicateScope {
            callable: required.callable,
            literal: &|expression| bound_literal(program, &required, expression),
            place: &|expression| {
                let place = bound_place(program, &required, expression)?;
                let machine = program.machine_by_symbol(caller.machine_symbol)?;
                // A syntactic match still owes operand chronology. In
                // demand(value, set(&mut value)), the first copy cannot name
                // the setter's post-state value. The shared prefix judge
                // declines that storage identity; arithmetic may separately
                // prove the retained copy at its own captured epoch.
                crate::flow::local_reference_storage_at_call(
                    program,
                    frames,
                    machine,
                    &facts.flow,
                    caller,
                    call,
                    place,
                    crate::flow::ValueObservation::Contents,
                )
            },
        };
        let supplied_scope = PredicateScope {
            callable: guarantee.invocation.callable,
            literal: &|expression| bound_literal(program, &guarantee.invocation, expression),
            place: &|expression| bound_place(program, &guarantee.invocation, expression),
        };
        predicates_match(
            program,
            semantic,
            contexts,
            &required_scope,
            expression,
            &supplied_scope,
            guarantee.expression,
            &mut Vec::new(),
        )
    });
    exact
        || arithmetic::proves(
            program,
            facts,
            caller,
            contexts,
            &required,
            expression,
            arithmetic_hypotheses,
            frames,
        )
}

fn capture_preserved(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    borrow: &checked_trees::BorrowFacts,
    caller: &Machine,
    supplied: &Invocation<'_>,
    guarantee: ExpressionHandle,
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    let Some((_, state)) = borrow.first_state_borrow(caller.symbol, supplied.caller_state) else {
        return false;
    };
    let Some(call) = borrow.calls.span_or_empty(state.calls).iter().find(|call| {
        call.statement_index == supplied.statement
            && call.call_ordinal == supplied.ordinal
            && (call.target_symbol == supplied.callable.target_symbol()
                || call.target_symbol == supplied.callable.owner_symbol())
    }) else {
        return false;
    };
    // Every caller-visible write needs exclusive authority named by the
    // signature. This complete ceiling owns capture preservation, including
    // callees whose instruction bodies have no precise write summary.
    let Some(writes) = crate::flow::signature_ceiling_places(
        program,
        caller.symbol,
        supplied.caller_state,
        None,
        call,
        Some(frames),
    ) else {
        return false;
    };
    if let Some(reads) = logical_formal_reads(program, operators, supplied, guarantee) {
        let parameters = supplied.callable.parameters(program);
        let arguments = validation::call_site_argument_expressions(program, &supplied.site);
        return reads.iter().all(|formal| {
            let Some(position) = parameters
                .iter()
                .position(|parameter| formal.root == PlaceRoot::Symbol(parameter.symbol))
            else {
                return false;
            };
            let Some(actual) = arguments.get(position) else {
                return false;
            };
            if formal.segments.is_empty() && crate::flow::literal(program, *actual).is_some() {
                return true;
            }
            let Some(place) = crate::semantic::facts::projected_argument_place(
                program,
                supplied.caller_state,
                supplied.statement,
                *actual,
                parameters[position].type_reference,
                &formal.segments,
            ) else {
                return false;
            };
            let Some(place) = crate::flow::rebase_exact_local_place(
                program,
                supplied.caller_state,
                supplied.statement,
                place,
                Some(frames),
            ) else {
                return false;
            };
            !writes.iter().any(|write| {
                write.root == place.root
                    && crate::flow::canonical_place_segments_may_overlap(
                        program,
                        &write.segments,
                        &place.segments,
                    )
            }) || crate::values::exclusive_reference(program, parameters[position].type_reference)
        });
    }
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        guarantee,
        &mut occurrences,
    );
    occurrences.into_iter().all(|occurrence| {
        if validation::reserved_result_place(program, occurrence)
            .is_some_and(|result| result.machine_symbol == supplied.callable.owner_symbol())
            || bound_literal(program, supplied, occurrence).is_some()
            || static_operands::immutable(program, occurrence)
        {
            return true;
        }
        let Some(place) = bound_place(program, supplied, occurrence) else {
            return false;
        };
        let Some(place) = crate::flow::rebase_exact_local_place(
            program,
            supplied.caller_state,
            supplied.statement,
            place,
            Some(frames),
        ) else {
            return false;
        };
        if !writes.iter().any(|write| {
            (write.root) == (place.root)
                && crate::flow::canonical_place_segments_may_overlap(
                    program,
                    &write.segments,
                    &place.segments,
                )
        }) {
            return true;
        }
        // An exclusive formal describes post-call storage regardless of
        // whether its actual spells a borrow or forwards an existing loan.
        // The callee's write establishes that guarantee, not an old snapshot.
        let Some(formal) = formal_place(program, supplied, occurrence) else {
            return false;
        };
        supplied
            .callable
            .parameters(program)
            .iter()
            .any(|parameter| {
                formal.root == PlaceRoot::Symbol(parameter.symbol)
                    && crate::values::exclusive_reference(program, parameter.type_reference)
            })
    })
}

/// Bind one declaration-scoped scalar guarantee through its authentic call
/// actuals. By-value inputs retain operand completion; an exclusive-reference
/// projection describes normal-return storage under the existing frame law.
/// The observer supplies value versions, never a replay of the callee body.
pub(in crate::checks) fn guarantee_scalar<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts checked_trees::CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    expression: ExpressionHandle,
    projection: &[facts::PlaceSegment],
    primitive: typed_trees::types::PrimitiveType,
    frames: &validation::CallFrameResolver<'_>,
    observe: &mut impl FnMut(
        &'facts checked_trees::FlowOperandFact,
        &[facts::PlaceSegment],
        typed_trees::types::PrimitiveType,
        bool,
    ) -> Option<validation::ScopedArithmeticValue>,
) -> Option<validation::ScopedArithmeticValue> {
    let supplied = invocation(program, caller, call.statement_index, call.call_ordinal)?;
    let bind = |occurrence,
                extra: &[facts::PlaceSegment],
                primitive,
                observe: &mut dyn FnMut(
        &'facts checked_trees::FlowOperandFact,
        &[facts::PlaceSegment],
        typed_trees::types::PrimitiveType,
        bool,
    ) -> Option<validation::ScopedArithmeticValue>| {
        let (actual, mut path, post_state) =
            if let Some(result) = validation::reserved_result_place(program, occurrence) {
                if result.machine_symbol != supplied.callable.owner_symbol() {
                    return None;
                }
                let CallSite::Expression { expression, .. } = supplied.site else {
                    return None;
                };
                (expression, result.segments, false)
            } else {
                let formal = formal_place(program, &supplied, occurrence)?;
                let post_state = supplied
                    .callable
                    .parameters(program)
                    .iter()
                    .any(|parameter| {
                        formal.root == PlaceRoot::Symbol(parameter.symbol)
                            && crate::values::exclusive_reference(program, parameter.type_reference)
                    });
                let (actual, path) = actual_projection(program, &supplied, occurrence)?;
                (actual, path, post_state)
            };
        path.extend_from_slice(extra);
        if post_state {
            let machine = program.machine_by_symbol(caller.machine_symbol)?;
            if matches!(supplied.site, CallSite::TransitionNamed { .. }) {
                return None;
            }
            let stable = stable_predicate_inputs(program, facts, caller, &supplied, occurrence);
            let preserved = capture_preserved(
                program,
                &facts.operators,
                &facts.borrow,
                machine,
                &supplied,
                occurrence,
                frames,
            );
            if !stable || !preserved {
                return None;
            }
        }
        let operand =
            captured_actual_operand(program, facts, caller, call, actual, &path, post_state)?;
        observe(operand, &path, primitive, post_state)
    };
    if projection.is_empty() && primitive == typed_trees::types::PrimitiveType::Bool {
        if let Some(facts::ScalarValue::Boolean(value)) = crate::flow::literal(program, expression)
        {
            return Some(validation::ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_u128(u128::from(value)),
            ));
        }
        return bind(expression, projection, primitive, observe);
    }
    if projection.is_empty() {
        arithmetic::scalar_term(
            program,
            facts,
            &supplied.callable,
            expression,
            |occurrence, primitive| bind(occurrence, &[], primitive, observe),
        )
    } else {
        bind(expression, projection, primitive, observe)
    }
}

/// Bind a declaration-owned place to the actual captured by this exact call.
/// Reference effect queries consume the address, not the referent's numeric
/// contents. Result projections and exclusive post-state values remain with
/// their existing guarantee judgments.
pub(super) fn guarantee_input<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts checked_trees::CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    expression: ExpressionHandle,
    suffix: &[facts::PlaceSegment],
) -> Option<(
    &'facts checked_trees::FlowOperandFact,
    Vec<facts::PlaceSegment>,
)> {
    let supplied = invocation(program, caller, call.statement_index, call.call_ordinal)?;
    let formal = formal_place(program, &supplied, expression)?;
    let parameter = supplied
        .callable
        .parameters(program)
        .iter()
        .find(|parameter| formal.root == PlaceRoot::Symbol(parameter.symbol))?;
    // The existing post-state binding judge handles exclusive parameters;
    // borrowing their pre-call address cannot establish an exit descriptor.
    if crate::values::exclusive_reference(program, parameter.type_reference) {
        return None;
    }
    let (actual, mut path) = actual_projection(program, &supplied, expression)?;
    path.extend_from_slice(suffix);
    let operand = captured_actual_operand(program, facts, caller, call, actual, &path, false)?;
    Some((operand, path))
}
fn captured_actual_operand<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts checked_trees::CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    actual: ExpressionHandle,
    path: &[facts::PlaceSegment],
    post_state: bool,
) -> Option<&'facts checked_trees::FlowOperandFact> {
    let mut operands = facts
        .flow
        .control
        .operands
        .iter()
        .map(|(_, operand)| operand)
        .filter(|operand| {
            operand.state_symbol == caller.state_symbol
                && operand.statement_index == call.statement_index
                && operand.expression == actual
        });
    let operand = operands.next()?;
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        caller.state_symbol,
        call.statement_index,
        actual,
    )
    .unwrap_or(CanonicalPlace {
        root: PlaceRoot::Expression(actual),
        segments: Vec::new(),
    });
    place.segments.extend_from_slice(&path);
    if !operands.all(|other| {
        if post_state {
            crate::flow::operand_bindings_agree(program, &facts.flow, operand, other, &place)
        } else {
            crate::flow::operand_observations_agree(program, &facts.flow, operand, other, &place)
        }
    }) {
        return None;
    }
    Some(operand)
}

fn invocation<'program>(
    program: &'program dyn ::validation::CheckingInput,
    caller: &FlowStateFact,
    statement: usize,
    ordinal: usize,
) -> Option<Invocation<'program>> {
    let site = validation::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        statement,
        ordinal,
    )?;
    let target = match &site {
        CallSite::Expression { call, .. } => call.target_symbol,
        CallSite::Statement(call) => call.target_symbol,
        CallSite::TransitionNamed { path, .. } => path.symbol,
    };
    let requirement = site.static_requirement_dispatch().is_some()
        || validation::selected_requirement_signature(program, target).is_some();
    let callable = if requirement {
        let (_, signature) =
            validation::requirement_contract_signature(program, caller.machine_symbol, &site)?;
        Callable::Requirement { signature }
    } else {
        match &site {
            CallSite::TransitionNamed { .. } => validation::find_state(program, target)
                .filter(|(machine, _)| machine.symbol == caller.machine_symbol)
                .map(|(machine, state)| Callable::Machine { machine, state })
                .or_else(|| Callable::resolve(program, target))?,
            _ => Callable::resolve(program, target)?,
        }
    };
    Some(Invocation {
        site,
        callable,
        caller_state: caller.state_symbol,
        statement,
        ordinal,
    })
}

fn ordinary_arguments(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
) -> bool {
    let parameters = invocation.callable.parameters(program);
    let arguments = validation::call_site_argument_expressions(program, &invocation.site);
    // Receiver, static and evidence substitution retain their own owners.
    let ordinary = match &invocation.site {
        CallSite::Expression { call, .. } => {
            static_operands::admitted(program, invocation)
                && call.evidence_arguments.is_empty()
                && (call.static_requirement_dispatch.is_none()
                    || matches!(invocation.callable, Callable::Requirement { .. }))
        }
        CallSite::Statement(call) => {
            static_operands::admitted(program, invocation)
                && call.evidence_arguments.is_empty()
                && (call.static_requirement_dispatch.is_none()
                    || matches!(invocation.callable, Callable::Requirement { .. }))
        }
        CallSite::TransitionNamed {
            evidence_arguments, ..
        } => evidence_arguments.is_empty(),
    };
    ordinary
        && parameters.len() == arguments.len()
        && parameters.iter().all(|parameter| !parameter.is_const)
}

/// A proof query consumes only the actuals its predicate names. Requiring every
/// sibling actual to be a place rejected `cell(grid, y, x + 1)` even for the
/// independent clause `y <= grid.max_y`. Effects of every operand still run
/// through the call-entry contexts and capture-preservation checks; this is
/// only admission to the exact-value vocabulary, not an effect exemption.
fn stable_predicate_inputs(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> bool {
    if !ordinary_arguments(program, invocation) {
        return false;
    }
    if let Some(reads) = logical_formal_reads(program, &facts.operators, invocation, expression) {
        let parameters = invocation.callable.parameters(program);
        let arguments = validation::call_site_argument_expressions(program, &invocation.site);
        return reads.iter().all(|place| {
            let Some(position) = parameters
                .iter()
                .position(|formal| place.root == PlaceRoot::Symbol(formal.symbol))
            else {
                return false;
            };
            let Some(actual) = arguments.get(position) else {
                return false;
            };
            stable_captured_projection(
                program,
                facts,
                caller,
                invocation,
                position,
                *actual,
                &place.segments,
            )
        });
    }
    let mut occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    occurrences.into_iter().all(|occurrence| {
        if let Some(result) = validation::reserved_result_place(program, occurrence) {
            return result.machine_symbol == invocation.callable.owner_symbol();
        }
        if static_operands::immutable(program, occurrence) {
            return true;
        }
        let Some((actual, path)) = actual_projection(program, invocation, occurrence) else {
            return false;
        };
        let Some(formal) = formal_place(program, invocation, occurrence) else {
            return false;
        };
        let Some(position) = invocation
            .callable
            .parameters(program)
            .iter()
            .position(|parameter| formal.root == PlaceRoot::Symbol(parameter.symbol))
        else {
            return false;
        };
        stable_captured_projection(program, facts, caller, invocation, position, actual, &path)
    })
}

/// Logical matches carry their subject binding inside this exact contract.
/// Return only the formal observations admitted by the common contract owner;
/// ambient captures and unsupported patterns cannot enter this route.
fn logical_formal_reads(
    program: &dyn validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<Vec<CanonicalPlace>> {
    let mut tree = Vec::new();
    validation::expression_traversal::collect_expression_tree(program, expression, &mut tree);
    if !tree.iter().any(|node| {
        matches!(
            program.expression_table.expression(*node),
            ExpressionNode::Match(_)
        )
    }) || !validation::expression_has_closed_value_captures(program, expression)
    {
        return None;
    }
    let parameters = invocation.callable.parameters(program);
    let predicate = crate::values::lower_callable_scalar_contract_predicate(
        program,
        operators,
        invocation.callable.owner_symbol(),
        parameters,
        invocation.callable.result_type(),
        expression,
        true,
        &mut 4096,
    )?;
    // A reserved result's synthetic query root only excludes it from the input
    // roster. It is not a runtime operand or a published result observation.
    let result = CanonicalPlace {
        root: PlaceRoot::Expression(expression),
        segments: Vec::new(),
    };
    let bound = boolean_predicates::bind_parameters(
        program,
        invocation.callable.owner_symbol(),
        invocation.callable.target_symbol(),
        parameters,
        Some((&result, invocation.callable.result_type())),
        predicate,
    )?;
    Some(
        bound
            .bindings
            .into_iter()
            .filter_map(|binding| (binding.place.root != result.root).then_some(binding.place))
            .collect(),
    )
}

fn stable_captured_projection(
    program: &dyn ::validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    invocation: &Invocation<'_>,
    position: usize,
    actual: ExpressionHandle,
    path: &[facts::PlaceSegment],
) -> bool {
    let Some(call) = invocation_fact(facts, caller, invocation) else {
        return false;
    };
    let Some((operands, receiver_offset)) =
        crate::flow::captured_call_operands(program, &facts.flow, caller, call)
    else {
        return false;
    };
    let Some(operand) = operands.get(receiver_offset + position) else {
        return false;
    };
    if operand.expression != actual
        || validation::project_type_reference_from_segments(program, operand.type_reference, path)
            .is_none()
    {
        return false;
    }
    // Reference prefixes remain live storage; preserve the existing exact-place
    // or detached-copy judgment rather than reconstructing an input value.
    stable_owned_projection(program, operand.type_reference, path)
        || stable_value(program, actual, &mut Vec::new())
}

fn stable_owned_projection(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
    path: &[facts::PlaceSegment],
) -> bool {
    let mut selected = reference;
    for length in 0..=path.len() {
        let Some(projected) =
            validation::project_type_reference_from_segments(program, reference, &path[..length])
        else {
            return false;
        };
        if crate::flow::operand_binds_live_view(program, projected) {
            return false;
        }
        selected = projected;
    }
    crate::flow::operand_binds_stable_copy(program, selected)
}

fn stable_value(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    pending: &mut Vec<ExpressionHandle>,
) -> bool {
    if pending.contains(&expression) {
        return false;
    }
    pending.push(expression);
    let stable = if let ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
    {
        program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| stable_value(program, field.value, pending))
    } else {
        crate::flow::literal(program, expression).is_some()
            || direct_place(program, expression)
                .or_else(|| exclusive_borrow_place(program, expression))
                .is_some_and(|place| matches!(place.root, PlaceRoot::Symbol(_)))
    };
    pending.pop();
    stable
}

/// An exclusive-borrow actual resolves through canonical place spelling: the
/// attached-field identity the write frame carries, which the authored member
/// gate in `checked_place` deliberately does not keep.
fn exclusive_borrow_place(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let borrow = program.expression_table.borrow_expression(expression)?;
    if !borrow.access.is_exclusive() {
        return None;
    }
    let place = validation::canonical_place_from_expression(program, expression)?;
    (matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && !place.segments.iter().any(|segment| {
            matches!(segment, facts::PlaceSegment::Index { .. })
                || crate::flow::place_segment_has_unresolved_identity(*segment)
        }))
    .then_some(place)
}

pub(in crate::checks) fn direct_place(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let mut current = expression;
    let mut visited = Vec::new();
    loop {
        if visited.contains(&current) {
            return None;
        }
        visited.push(current);
        current = match program.expression_table.expression(current) {
            ExpressionNode::Name(_) => break,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            // Indexing needs its own selected-operation and selector-capture
            // evidence. Canonical storage spelling alone supplies neither.
            _ => return None,
        };
    }
    super::field_actuals::checked_place(program, expression)
}

fn builtin_predicate(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> bool {
    super::has_builtin_operators(program, operators, expression)
        && invocation.callable.builtin_meaning(program, expression)
}

fn predicates_match(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    required: &PredicateScope<'_, '_>,
    left: ExpressionHandle,
    supplied: &PredicateScope<'_, '_>,
    right: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, ExpressionHandle)>,
) -> bool {
    if pending.contains(&(left, right)) {
        return false;
    }
    pending.push((left, right));
    let matched = match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Binary(left_binary), ExpressionNode::Binary(right_binary)) => {
            left_binary.operator == right_binary.operator
                && same_scalar_type(
                    program,
                    &required.callable,
                    left_binary.left,
                    &supplied.callable,
                    right_binary.left,
                )
                && same_scalar_type(
                    program,
                    &required.callable,
                    left_binary.right,
                    &supplied.callable,
                    right_binary.right,
                )
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_binary.left,
                    supplied,
                    right_binary.left,
                    pending,
                )
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_binary.right,
                    supplied,
                    right_binary.right,
                    pending,
                )
        }
        (ExpressionNode::Unary(left_unary), ExpressionNode::Unary(right_unary)) => {
            left_unary.operator == right_unary.operator
                && same_scalar_type(
                    program,
                    &required.callable,
                    left_unary.operand,
                    &supplied.callable,
                    right_unary.operand,
                )
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_unary.operand,
                    supplied,
                    right_unary.operand,
                    pending,
                )
        }
        (ExpressionNode::Cast(left_cast), ExpressionNode::Cast(right_cast)) => {
            same_scalar_type(program, &required.callable, left, &supplied.callable, right)
                && predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left_cast.value,
                    supplied,
                    right_cast.value,
                    pending,
                )
        }
        _ => match ((required.literal)(left), (supplied.literal)(right)) {
            (Some(left), Some(right)) => left == right,
            _ => (required.place)(left)
                .zip((supplied.place)(right))
                .is_some_and(|(left, right)| {
                    captured_place(program, semantic, contexts, left)
                        .zip(captured_place(program, semantic, contexts, right))
                        .is_some_and(|(left, right)| left == right)
                }),
        },
    };
    // A surviving conjunction establishes each constituent proposition. This
    // is logical decomposition, not another source arrangement recognizer.
    let matched = matched
        || program
            .expression_table
            .binary_node(right)
            .filter(|binary| binary.operator == typed_trees::expression::BinaryOperator::And)
            .is_some_and(|binary| {
                predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left,
                    supplied,
                    binary.left,
                    pending,
                ) || predicates_match(
                    program,
                    semantic,
                    contexts,
                    required,
                    left,
                    supplied,
                    binary.right,
                    pending,
                )
            });
    pending.pop();
    matched
}

fn same_scalar_type(
    program: &dyn ::validation::CheckingInput,
    left_owner: &Callable<'_>,
    left: ExpressionHandle,
    right_owner: &Callable<'_>,
    right: ExpressionHandle,
) -> bool {
    let reference = |owner: &Callable<'_>, expression| owner.scalar_reference(program, expression);
    // Contextual literals need not retain a standalone carrier reference.
    // The enclosing operation was independently selected and its observed
    // operands still owe identical carriers and policies. Compare exact
    // literal values rather than inventing an expected type for them.
    if let (Some(left), Some(right)) = (
        crate::flow::literal(program, left),
        crate::flow::literal(program, right),
    ) {
        return left == right;
    }
    match (reference(left_owner, left), reference(right_owner, right)) {
        (Some(left), Some(right)) => {
            program.primitive_type_reference(left).is_some()
                && program.primitive_type_reference(left) == program.primitive_type_reference(right)
                && program.arithmetic_domain_for_type_reference(left)
                    == program.arithmetic_domain_for_type_reference(right)
        }
        _ => false,
    }
}

fn actual_projection(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
    formal_place(program, invocation, expression)?;
    crate::semantic::places::call_contract_argument_projection(
        program,
        invocation.callable.parameters(program),
        validation::call_site_argument_expressions(program, &invocation.site),
        expression,
    )
}

/// A contract projection belongs to its declaration and selected builtin
/// operations. Literal indexes have exact coordinates; runtime selectors need
/// capture evidence this static projection cannot supply.
fn formal_place(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let parameters = invocation.callable.parameters(program);
    let (position, path, _) =
        crate::values::structural_parameter_place(program, parameters, expression)?;
    let (root, segments, _, _) =
        crate::values::resolve_structural_parameter_path(program, parameters, position, &path)?;
    Some(CanonicalPlace {
        root: PlaceRoot::Symbol(root),
        segments,
    })
}

fn bound_literal(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<facts::ScalarValue> {
    crate::flow::literal(program, expression).or_else(|| {
        let (value, remaining) = actual_projection(program, invocation, expression)?;
        if !remaining.is_empty() {
            return None;
        }
        crate::flow::literal(program, value)
    })
}

fn bound_place(
    program: &dyn ::validation::CheckingInput,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if let Some(result) = validation::reserved_result_place(program, expression) {
        if result.machine_symbol != invocation.callable.owner_symbol() {
            return None;
        }
        let CallSite::Expression { expression, .. } = invocation.site else {
            return None;
        };
        return Some(CanonicalPlace {
            root: PlaceRoot::Expression(expression),
            segments: result.segments,
        });
    }
    let (actual, remaining) = actual_projection(program, invocation, expression)?;
    let mut place = validation::canonical_place_from_expression_in_state(
        program,
        invocation.caller_state,
        invocation.statement,
        actual,
    )?;
    place.segments.extend(remaining);
    Some(place)
}

pub(crate) fn captured_place(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    // A completed value can be stored at any exact projection. Its surviving
    // provenance rebases the remaining path onto that occurrence. The deepest
    // matching assignment wins; a newer non-call value cannot revive an older
    // ancestor's call. Flow retires these rows on overlapping writes.
    let mut source = None;
    let mut non_call_value = false;
    let mut prefix_length = None;
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        if !matches!(
            fact.payload,
            FactPayload::AssignedValue { .. }
                | FactPayload::AssignedScalarValue { .. }
                | FactPayload::AssignedValueCapture { .. }
        ) {
            continue;
        }
        let Some(destination) = fact.place.as_place() else {
            continue;
        };
        let destination = semantic.places.get(destination);
        let segments = semantic.place_segments.span_or_empty(destination.segments);
        if destination.root != place.root || !place.segments.starts_with(segments) {
            continue;
        }
        if prefix_length.is_some_and(|previous| previous > segments.len()) {
            continue;
        }
        if prefix_length != Some(segments.len()) {
            prefix_length = Some(segments.len());
            source = None;
            non_call_value = false;
        }
        match fact.payload {
            FactPayload::AssignedValue { value }
                if matches!(
                    program.expression_table.expression(value),
                    ExpressionNode::Call(_)
                ) =>
            {
                if source.is_some_and(|prior| prior != value) {
                    return None;
                }
                source = Some(value);
            }
            _ => non_call_value = true,
        }
    }
    if let Some(source) = source {
        if non_call_value {
            return None;
        }
        place.root = PlaceRoot::Expression(source);
        place.segments.drain(..prefix_length?);
    }
    Some(place)
}
