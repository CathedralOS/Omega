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
//! provenance retires changed results. Predicate inputs must be places or
//! literals (including constructor fields) and preserve their read footprints:
//! expression-root dependencies alone do
//! not establish that a computed actual still denotes its captured value.

use crate::flow::CanonicalPlace;
use crate::semantic::calls::CallSite;
use checked_trees::{CheckedOperatorFacts, FlowCallFact, FlowStateFact};
use facts::{FactPayload, FactPlace, FactPlan, PlaceRoot};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;

pub(in crate::checks) mod arithmetic;
pub(in crate::checks) mod callable;
use callable::Callable;
mod availability;
pub(in crate::checks) use availability::{AvailableGuarantee, available};

struct Invocation<'program> {
    site: CallSite<'program>,
    callable: Callable<'program>,
    caller_state: SymbolHandle,
    statement: usize,
    ordinal: usize,
}

/// Read surviving normal guarantees as formulas over the caller's current
/// storage. Availability owns declaration identity, actual capture and write
/// preservation; this join only changes the formula's observation namespace.
/// Cleanup can then retire those observations after each selected hook using
/// the same mutation law as the rest of source flow.
pub(crate) fn current_predicates(
    program: &TypedTrees,
    facts: &checked_trees::CheckFacts,
    caller: &FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    frames: &validation::CallFrameResolver<'_>,
) -> Vec<crate::facts::boolean_predicates::BoundPredicate> {
    available(program, facts, caller, statement, contexts, frames)
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
            let mut predicate = crate::facts::boolean_predicates::bind_parameters(
                program,
                invocation.callable.owner_symbol(),
                invocation.callable.target_symbol(),
                parameters,
                expression,
            )?;
            let arguments =
                crate::semantic::calls::call_site_argument_expressions(program, &invocation.site);
            for binding in &mut predicate.bindings {
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

pub(super) fn proves(
    program: &TypedTrees,
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
    if !stable_predicate_inputs(program, &required, expression)
        || !builtin_predicate(program, operators, &required, expression)
    {
        return false;
    }
    let mut arithmetic_hypotheses = Vec::new();
    let exact = available(
        program,
        facts,
        caller,
        call.statement_index,
        contexts,
        frames,
    )
    .into_iter()
    .any(|guarantee| {
        if let Some(proposition) = arithmetic::at_call(
            program,
            facts,
            caller,
            &guarantee.invocation,
            guarantee.expression,
            frames,
        ) {
            arithmetic_hypotheses.push(validation::ScopedArithmeticHypothesis {
                proposition,
                holds: true,
            });
        }
        predicates_match(
            program,
            semantic,
            contexts,
            &required,
            expression,
            &guarantee.invocation,
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
    program: &TypedTrees,
    borrow: &checked_trees::BorrowFacts,
    caller: &Machine,
    supplied: &Invocation<'_>,
    frame: facts::NormalizedWriteFrame,
    guarantee: ExpressionHandle,
    frames: &validation::CallFrameResolver<'_>,
) -> bool {
    let Some(mut writes) = crate::flow::frame_storage_writes(
        program,
        caller.symbol,
        supplied.caller_state,
        supplied.statement,
        &frame,
        Some(frames),
    ) else {
        return false;
    };
    let Some(state) = borrow.states.iter().map(|(_, state)| state).find(|state| {
        state.machine_symbol == caller.symbol && state.state_symbol == supplied.caller_state
    }) else {
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
    // Ordinary body frames do not yet summarize every selected operator.
    // Reuse the independent signature/authority ceiling instead of trusting
    // an omitted dispatch or introducing another transitive body scanner.
    let Some(ceiling) = crate::flow::signature_ceiling_places(
        program,
        caller.symbol,
        supplied.caller_state,
        call,
        Some(frames),
    ) else {
        return false;
    };
    writes.extend(ceiling);
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
        // An operand bound through an exclusive-borrow actual is written by the
        // callee's own contract — that write is the establishment the
        // guarantee describes, not capture.
        let Some((actual, _)) = actual_projection(program, supplied, occurrence) else {
            return false;
        };
        matches!(
            program.expression_table.expression(actual),
            ExpressionNode::Borrow(borrow) if borrow.access.is_exclusive()
        )
    })
}

fn invocation<'program>(
    program: &'program TypedTrees,
    caller: &FlowStateFact,
    statement: usize,
    ordinal: usize,
) -> Option<Invocation<'program>> {
    let site = crate::semantic::calls::find_call_site(
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
    let callable = match &site {
        CallSite::TransitionNamed { .. } => {
            crate::semantic::calls::find_state_with_machine(program, target)
                .filter(|(machine, _)| machine.symbol == caller.machine_symbol)
                .map(|(machine, state)| Callable::Machine { machine, state })
                .or_else(|| Callable::resolve(program, target))?
        }
        _ => Callable::resolve(program, target)?,
    };
    Some(Invocation {
        site,
        callable,
        caller_state: caller.state_symbol,
        statement,
        ordinal,
    })
}

fn ordinary_arguments(program: &TypedTrees, invocation: &Invocation<'_>) -> bool {
    let parameters = invocation.callable.parameters(program);
    let arguments =
        crate::semantic::calls::call_site_argument_expressions(program, &invocation.site);
    // Receiver, static and evidence substitution retain their own owners.
    let ordinary = match &invocation.site {
        CallSite::Expression { call, .. } => {
            call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
        }
        CallSite::Statement(call) => {
            call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && call.static_requirement_dispatch.is_none()
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
    program: &TypedTrees,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> bool {
    if !ordinary_arguments(program, invocation) {
        return false;
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
        actual_projection(program, invocation, occurrence)
            .is_some_and(|(actual, _)| stable_value(program, actual, &mut Vec::new()))
    })
}

fn stable_value(
    program: &TypedTrees,
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
        super::scalars::literal(program, expression).is_some()
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
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        return None;
    };
    if !borrow.access.is_exclusive() {
        return None;
    }
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    (matches!(place.root, PlaceRoot::Symbol(symbol) if symbol.is_valid())
        && !place.segments.iter().any(|segment| {
            matches!(segment, facts::PlaceSegment::Index { .. })
                || crate::flow::place_segment_has_unresolved_identity(*segment)
        }))
    .then_some(place)
}

pub(in crate::checks) fn direct_place(
    program: &TypedTrees,
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
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> bool {
    super::has_builtin_operators(program, operators, expression)
        && invocation.callable.builtin_meaning(program, expression)
}

fn predicates_match(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    required: &Invocation<'_>,
    left: ExpressionHandle,
    supplied: &Invocation<'_>,
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
                    required,
                    left_binary.left,
                    supplied,
                    right_binary.left,
                )
                && same_scalar_type(
                    program,
                    required,
                    left_binary.right,
                    supplied,
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
                    required,
                    left_unary.operand,
                    supplied,
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
            same_scalar_type(program, required, left, supplied, right)
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
        _ => {
            match (
                bound_literal(program, required, left),
                bound_literal(program, supplied, right),
            ) {
                (Some(left), Some(right)) => left == right,
                _ => bound_place(program, required, left)
                    .zip(bound_place(program, supplied, right))
                    .is_some_and(|(left, right)| {
                        captured_place(program, semantic, contexts, left)
                            .zip(captured_place(program, semantic, contexts, right))
                            .is_some_and(|(left, right)| left == right)
                    }),
            }
        }
    };
    pending.pop();
    matched
}

fn same_scalar_type(
    program: &TypedTrees,
    left_owner: &Invocation<'_>,
    left: ExpressionHandle,
    right_owner: &Invocation<'_>,
    right: ExpressionHandle,
) -> bool {
    let reference =
        |owner: &Invocation<'_>, expression| owner.callable.scalar_reference(program, expression);
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
    program: &TypedTrees,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, Vec<facts::PlaceSegment>)> {
    direct_place(program, expression)?;
    crate::semantic::places::call_contract_argument_projection(
        program,
        invocation.callable.parameters(program),
        crate::semantic::calls::call_site_argument_expressions(program, &invocation.site),
        expression,
    )
}

fn bound_literal(
    program: &TypedTrees,
    invocation: &Invocation<'_>,
    expression: ExpressionHandle,
) -> Option<facts::ScalarValue> {
    super::scalars::literal(program, expression).or_else(|| {
        let (value, remaining) = actual_projection(program, invocation, expression)?;
        if !remaining.is_empty() {
            return None;
        }
        super::scalars::literal(program, value)
    })
}

fn bound_place(
    program: &TypedTrees,
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
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        invocation.caller_state,
        invocation.statement,
        actual,
    )?;
    place.segments.extend(remaining);
    Some(place)
}

fn captured_place(
    program: &TypedTrees,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    // Flow copies this provenance with a whole-value copy and removes it on
    // overlapping writes. Its current statement need not be the original call.
    let mut source = None;
    let mut non_call_value = false;
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        if !matches!(
            fact.payload,
            FactPayload::AssignedValue { .. } | FactPayload::AssignedScalarValue { .. }
        ) {
            continue;
        }
        let FactPlace::Place(destination) = fact.place else {
            continue;
        };
        let destination = semantic.places.get(destination);
        if destination.root != place.root || !destination.segments.is_empty() {
            continue;
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
    }
    Some(place)
}
