//! Rejoin computed invocations to captured expression occurrences, not spans.
//! Semantic casts are part of that ordering: equal final tags cannot justify
//! skipping an explicit erasure followed by reintroduction. Only casts whose
//! operand is independently known to be bare may be peeled as payload wrappers.
use super::super::{CheckedBooleanExpression, CheckedScalarExpression};
use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, PrimitiveType, StatementNode,
    authored_state, unsupported,
};
use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind};

use crate::expression_preparation::source_custody::borrow_occurrences as borrow_rows;
mod borrowed_nominal_arguments;
pub(crate) mod byte_comparisons;
pub(crate) mod byte_predicates;
mod dispatch;
mod mixed_arguments;
mod operand_scopes;
mod owned_arguments;
pub(crate) mod primitive_arguments;
mod qualifications;
mod view_arguments;

pub(crate) use mixed_arguments::access_occurrences::{
    rejoin as rejoin_call_accesses, replay as replay_call_accesses,
};
pub(crate) use mixed_arguments::{RejoinedComputationArgument, rejoin_computation_call_arguments};

pub(crate) fn validate_computation_calls(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    root: CheckedScalarComputationHandle,
    authored_root: ExpressionHandle,
) -> Result<(), LoweringError> {
    let expressions = authored_expressions(checked, authored_root)?;
    validate_local_availability(checked, state, statement, &expressions)?;
    let executable = expression_membership(checked, authored_root, true)?;
    let plans = &checked.facts.values.scalar_computations;
    let control = &checked.facts.flow.control;
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let source_state = states.next().ok_or(LoweringError::Unsupported(
        "computed invocation has no checked source state",
    ))?;
    if states.next().is_some() {
        return unsupported("computed invocation has ambiguous checked source state");
    }
    let source_calls = control
        .calls
        .span(source_state.calls)
        .ok_or(LoweringError::Unsupported(
            "computed invocation has an invalid source call span",
        ))?;
    let mut pending = vec![(root, false, authored_root)];
    let mut active = Vec::new();
    let mut calls = Vec::new();
    // Discriminant-dispatch selection chains pair each link with the authored
    // match's next classifier arm; the DFS walk visits the head link first.
    let mut case_selection_arms: std::collections::HashMap<ExpressionHandle, usize> =
        std::collections::HashMap::new();
    while let Some((handle, exiting, authored_scope)) = pending.pop() {
        if exiting {
            active.pop();
            continue;
        }
        if !plans.nodes.is_valid(handle) || active.contains(&handle) {
            return unsupported("computed invocation has an invalid or cyclic computation");
        }
        active.push(handle);
        pending.push((handle, true, authored_scope));
        let node = plans.nodes.get(handle);
        let authored_scope = operand_scopes::folded_match_scope(checked, authored_scope)?;
        match &node.kind {
            CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                operand_type,
                source_expression,
                subject,
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                if node.primitive_type != PrimitiveType::Bool {
                    return unsupported("byte content comparison is not Boolean");
                }
                byte_comparisons::validate(
                    checked,
                    state,
                    statement,
                    *source_expression,
                    *operand_type,
                    subject,
                )?;
            }
            CheckedScalarComputationKind::ByteSequencePredicate {
                operand_type,
                source_expression,
                subject,
                ..
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                if node.primitive_type != PrimitiveType::Bool {
                    return unsupported("byte predicate evaluation is not Boolean");
                }
                byte_predicates::validate(
                    checked,
                    state,
                    statement,
                    *source_expression,
                    *operand_type,
                    subject,
                )?;
                // The predicate computation claims its authored call
                // expression exactly like a Call computation does.
                calls.push(*source_expression);
            }
            CheckedScalarComputationKind::StructuralField {
                source_expression,
                subject,
                field,
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                crate::expression_preparation::source_custody::record_fields::validate_source(
                    checked,
                    machine,
                    state,
                    statement,
                    *source_expression,
                    subject,
                    *field,
                    node.primitive_type,
                )?;
            }
            CheckedScalarComputationKind::CaseMembership {
                source_expression,
                subject,
                case,
            } => {
                if node.primitive_type != PrimitiveType::Bool {
                    return unsupported("computed case observation is not Boolean");
                }
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                let fields =
                    crate::expression_preparation::source_custody::case_sources::membership(
                        checked,
                        machine,
                        state,
                        *source_expression,
                        subject,
                        *case,
                    )?;
                for (source, computation) in &fields {
                    super::value_correspondence::validate(
                        checked,
                        state,
                        statement,
                        *source,
                        plans.nodes.get(*computation).primitive_type,
                        &checked_trees::CheckedCallScalarArgument::Computation(*computation),
                    )?;
                }
                pending.extend(
                    fields
                        .into_iter()
                        .rev()
                        .map(|(source, computation)| (computation, false, source)),
                );
            }
            CheckedScalarComputationKind::SelectedComparison {
                operator_use,
                left,
                right,
            } => {
                crate::expression_preparation::source_custody::comparisons::occurrence(
                    checked,
                    *operator_use,
                    machine,
                    state,
                    statement,
                )?;
                let selected = checked.facts.operators.uses.get(*operator_use);
                if selected.occurrence != checked_trees::CheckedOperatorOccurrence::Expression {
                    return unsupported("ordinary comparison substituted an implicit occurrence");
                }
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    selected.expression,
                    node.primitive_type,
                )?;
                let operands =
                    selected
                        .operands(&checked.typed)
                        .ok_or(LoweringError::Unsupported(
                            "comparison lost source operands",
                        ))?;
                pending.push((*right, false, operands[1]));
                pending.push((*left, false, operands[0]));
            }
            CheckedScalarComputationKind::Qualification {
                source_expression,
                operand,
                result_type,
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                let source = qualifications::operand(
                    checked,
                    machine,
                    state,
                    statement,
                    *source_expression,
                    *operand,
                    *result_type,
                    node.primitive_type,
                )?;
                pending.push((*operand, false, source));
            }
            CheckedScalarComputationKind::AuthoredCast {
                source_expression,
                operand,
                result_type,
                ..
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                let source = qualifications::authored_operand(
                    checked,
                    machine,
                    state,
                    statement,
                    *source_expression,
                    *operand,
                    *result_type,
                    node.primitive_type,
                )?;
                pending.push((*operand, false, source));
            }
            CheckedScalarComputationKind::BooleanToInteger {
                source_expression,
                operand,
            } => {
                // The cast occurrence owns this conversion; its operand scope
                // is the authored cast value. The fixed 0/1 landing introduces
                // no separate operation source.
                let ExpressionNode::Cast(cast) =
                    checked.expression_table.expression(*source_expression)
                else {
                    return unsupported(
                        "Boolean-to-integer computation lost its authored cast occurrence",
                    );
                };
                if !plans.nodes.is_valid(*operand)
                    || plans.nodes.get(*operand).primitive_type != PrimitiveType::Bool
                {
                    return unsupported("Boolean-to-integer computation has a non-Boolean operand");
                }
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                pending.push((*operand, false, cast.value));
            }
            CheckedScalarComputationKind::Dispatch {
                source_expression,
                subject,
                arms,
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                let operands = dispatch::operands(
                    checked,
                    machine,
                    state,
                    statement,
                    *source_expression,
                    *subject,
                    *arms,
                    node.primitive_type,
                )?;
                pending.extend(
                    operands
                        .into_iter()
                        .rev()
                        .map(|(computation, source)| (computation, false, source)),
                );
            }
            CheckedScalarComputationKind::Value(value) => {
                let authored_scope = operand_scopes::value(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    node.value_source,
                )?;
                for source in expression_membership(checked, authored_scope, true)? {
                    if let ExpressionNode::Cast(cast) = checked.expression_table.expression(source)
                        && operand_scopes::cast_requires_custody(checked, machine, state, cast)?
                    {
                        return unsupported(
                            "pure computation cannot erase a semantic qualification",
                        );
                    }
                }
                crate::expression_preparation::source_custody::validate_storage_read_expression(
                    checked,
                    state,
                    statement,
                    authored_scope,
                    value,
                )?;
            }
            CheckedScalarComputationKind::Select {
                source_expression,
                condition,
                when_true,
                when_false,
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                if let ExpressionNode::Match(dispatch) =
                    checked.expression_table.expression(*source_expression)
                {
                    select_case_arm(
                        checked,
                        machine,
                        state,
                        plans,
                        &mut pending,
                        &mut case_selection_arms,
                        *source_expression,
                        dispatch,
                        *condition,
                        *when_true,
                        *when_false,
                    )?;
                    continue;
                }
                if let Some(arguments) =
                    operand_scopes::pure_scalar_builtin_arguments(checked, *source_expression)
                {
                    // A pure scalar builtin selects between both evaluated
                    // operands: `min(a, b)` lowers as `(a <= b) ? a : b`, so
                    // the operand scopes are the call's authored arguments and
                    // the synthesized comparison keeps the call's own scope.
                    let arguments = checked.expression_table.expression_handles(arguments);
                    let [true_scope, false_scope] = *arguments else {
                        return unsupported(
                            "computed selection lost its authored operand positions",
                        );
                    };
                    pending.extend([
                        (*when_false, false, false_scope),
                        (*when_true, false, true_scope),
                        (*condition, false, *source_expression),
                    ]);
                    continue;
                }
                let (condition_scope, selected_scope, evaluate_when) =
                    operand_scopes::selection(checked, *source_expression)?;
                let (selected, skipped) = if evaluate_when {
                    (*when_true, *when_false)
                } else {
                    (*when_false, *when_true)
                };
                if !plans.nodes.is_valid(skipped)
                    || !matches!(&plans.nodes.get(skipped).kind,
                        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(value))
                        if matches!(value.as_ref(), CheckedBooleanExpression::Constant(value)
                            if *value == !evaluate_when))
                {
                    return unsupported("computed selection substituted its skipped value");
                }
                pending.extend([
                    (selected, false, selected_scope),
                    (*condition, false, condition_scope),
                ]);
            }
            CheckedScalarComputationKind::Apply {
                source_expression,
                operands,
                ..
            } => {
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    *source_expression,
                    node.primitive_type,
                )?;
                // Synthetic case observations are meaningful only together:
                // no individual membership may stand in for this equality.
                if super::enum_equality::validate_application(checked, state, handle)? {
                    continue;
                }
                let operands = plans
                    .operands
                    .span(*operands)
                    .ok_or(LoweringError::Unsupported(
                        "computed invocation has an invalid operand span",
                    ))?;
                let scopes = operand_scopes::application(
                    checked,
                    machine,
                    state,
                    *source_expression,
                    operands.len(),
                )?;
                pending.extend(
                    operands
                        .iter()
                        .zip(scopes)
                        .rev()
                        .map(|(operand, scope)| (*operand, false, scope)),
                );
            }
            CheckedScalarComputationKind::BoundaryCall {
                source_call,
                target_state,
                call_ordinal,
                arguments,
                ..
            } => {
                // A nested boundary application owns exactly one authored
                // call occurrence, like an ordinary computed call. Its dense
                // scalar operands rejoin the authored arguments by position:
                // the admitted signatures carry no self or structural formal
                // that could shift an operand, and a named operator's
                // qualifier (`F64::`) is not an operand.
                if !control.calls.is_valid(*source_call) {
                    return unsupported("nested boundary application has no live source call");
                }
                let source = control.calls.get(*source_call);
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    source.authored_expression,
                    node.primitive_type,
                )?;
                let scoped_expressions = expression_membership(checked, authored_scope, true)?;
                let matching = source_calls
                    .iter()
                    .filter(|candidate| {
                        candidate.statement_index == statement as usize
                            && candidate.call_ordinal == *call_ordinal as usize
                    })
                    .collect::<Vec<_>>();
                if matching.len() != 1
                    || !std::ptr::eq(matching[0], source)
                    || source.target_symbol != *target_state
                    || !executable.contains(&source.authored_expression)
                    || !scoped_expressions.contains(&source.authored_expression)
                    || calls.contains(&source.authored_expression)
                {
                    return unsupported(
                        "nested boundary application disagrees with its authored occurrence",
                    );
                }
                calls.push(source.authored_expression);
                let ExpressionNode::Call(authored) = checked
                    .expression_table
                    .expression(source.authored_expression)
                else {
                    return unsupported("nested boundary application lost its authored call");
                };
                let authored_arguments = checked
                    .expression_table
                    .expression_handles(authored.arguments);
                let operands =
                    plans
                        .operands
                        .span(*arguments)
                        .ok_or(LoweringError::Unsupported(
                            "nested boundary application has an invalid operand span",
                        ))?;
                if operands.len() != authored_arguments.len() {
                    return unsupported(
                        "nested boundary application operands disagree with its authored arguments",
                    );
                }
                pending.extend(
                    operands
                        .iter()
                        .zip(authored_arguments)
                        .rev()
                        .map(|(operand, argument)| (*operand, false, *argument)),
                );
            }
            CheckedScalarComputationKind::Call {
                source_call,
                target_state,
                call_ordinal,
                ..
            } => {
                if !control.calls.is_valid(*source_call) {
                    return unsupported("computed invocation has no live checked source call");
                }
                let source = control.calls.get(*source_call);
                dispatch::source_scope(
                    checked,
                    machine,
                    state,
                    authored_scope,
                    source.authored_expression,
                    node.primitive_type,
                )?;
                let scoped_expressions = expression_membership(checked, authored_scope, true)?;
                let matching = source_calls
                    .iter()
                    .filter(|candidate| {
                        candidate.statement_index == statement as usize
                            && candidate.call_ordinal == *call_ordinal as usize
                    })
                    .collect::<Vec<_>>();
                if matching.len() != 1
                    || !std::ptr::eq(matching[0], source)
                    || source.target_symbol != *target_state
                    || !executable.contains(&source.authored_expression)
                    || !scoped_expressions.contains(&source.authored_expression)
                    || calls.contains(&source.authored_expression)
                {
                    return unsupported(
                        "computed invocation disagrees with its authored occurrence",
                    );
                }
                calls.push(source.authored_expression);
                let arguments =
                    rejoin_computation_call_arguments(checked, machine, state, statement, handle)?;
                for argument in arguments.into_iter().rev() {
                    match argument {
                        RejoinedComputationArgument::Scalar {
                            expression,
                            computation,
                        } => {
                            pending.push((computation, false, expression));
                        }
                        RejoinedComputationArgument::Structural { expression } => {
                            if !scoped_expressions.contains(&expression) {
                                return unsupported(
                                    "computed borrow escaped its authored argument scope",
                                );
                            }
                        }
                        RejoinedComputationArgument::Construction {
                            expression,
                            elements,
                        } => {
                            if !scoped_expressions.contains(&expression) {
                                return unsupported(
                                    "computed construction escaped its authored argument scope",
                                );
                            }
                            pending.extend(
                                elements.into_iter().rev().map(|(expression, computation)| {
                                    (computation, false, expression)
                                }),
                            );
                        }
                    }
                }
            }
        }
    }
    if source_calls.iter().any(|source| {
        source.statement_index == statement as usize
            && executable.contains(&source.authored_expression)
            && !calls.contains(&source.authored_expression)
            && !checked
                .symbols
                .builtin_function_for_symbol(source.target_symbol)
                .is_some_and(symbols::BuiltinFunction::has_empty_write_frame)
    }) {
        return unsupported("computed invocation omitted an authored source call");
    }
    Ok(())
}

/// A discriminant dispatch lowers to a chain of selections over the authored
/// match's classifier arms, in order: each link's condition is that arm's
/// case-membership observation over the match, its taken operand is the arm's
/// authored value, and its tail is either the next link or the wildcard arm's
/// value. The chain carries no evaluated pattern operand, so the And/Or scope
/// rules do not apply.
fn select_case_arm(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    plans: &checked_trees::CheckedScalarComputationPlans,
    pending: &mut Vec<(CheckedScalarComputationHandle, bool, ExpressionHandle)>,
    case_selection_arms: &mut std::collections::HashMap<ExpressionHandle, usize>,
    source_expression: ExpressionHandle,
    dispatch: &checked_trees::expression::TableMatchExpression,
    condition: CheckedScalarComputationHandle,
    when_true: CheckedScalarComputationHandle,
    when_false: CheckedScalarComputationHandle,
) -> Result<(), LoweringError> {
    let typed_machine = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .ok_or(LoweringError::Unsupported(
            "computed selection lost its source machine",
        ))?;
    let typed_state = checked
        .machine_states(typed_machine)
        .iter()
        .find(|candidate| candidate.symbol == state)
        .ok_or(LoweringError::Unsupported(
            "computed selection lost its source state",
        ))?;
    let plan =
        validation::match_case_dispatch(&checked.typed, typed_machine, typed_state, dispatch)
            .ok_or(LoweringError::Unsupported(
                "computed selection lost its discriminant dispatch",
            ))?;
    let ordinal = case_selection_arms
        .get(&source_expression)
        .copied()
        .unwrap_or(0);
    case_selection_arms.insert(source_expression, ordinal + 1);
    let arm = plan
        .arms
        .get(ordinal)
        .filter(|arm| arm.case.is_some())
        .ok_or(LoweringError::Unsupported(
            "computed selection lost its classifier arm order",
        ))?;
    let observed = arm.case.ok_or(LoweringError::Unsupported(
        "computed selection lost its classifier arm",
    ))?;
    if !plans.nodes.is_valid(condition)
        || !matches!(
            &plans.nodes.get(condition).kind,
            CheckedScalarComputationKind::CaseMembership {
                source_expression: source,
                case,
                ..
            } if *source == source_expression && *case == observed
        )
    {
        return unsupported("computed selection substituted its membership condition");
    }
    pending.push((when_true, false, arm.value));
    pending.push((condition, false, source_expression));
    match plan.arms.get(ordinal + 1) {
        Some(next) if next.case.is_some() => {
            if !plans.nodes.is_valid(when_false)
                || !matches!(
                    &plans.nodes.get(when_false).kind,
                    CheckedScalarComputationKind::Select {
                        source_expression: source,
                        ..
                    } if *source == source_expression
                )
            {
                return unsupported("computed selection lost its next case arm");
            }
            pending.push((when_false, false, source_expression));
        }
        Some(wildcard) if wildcard.case.is_none() && ordinal + 1 == plan.arms.len() - 1 => {
            pending.push((when_false, false, wildcard.value));
        }
        _ => return unsupported("computed selection has no wildcard tail"),
    }
    Ok(())
}

/// The state parameter a `Parameter { parameter_index }` source names. The
/// index is dense over structural parameters only: an `[erased]` binding or a
/// primitive-typed one occupies no position. `missing` is the lane's
/// diagnostic for an index past the last structural parameter.
fn structural_parameter_at<'a>(
    checked: &CheckedTrees,
    parameters: &'a [checked_trees::signature::StateParameter],
    parameter_index: u32,
    missing: &'static str,
) -> Result<&'a checked_trees::signature::StateParameter, LoweringError> {
    parameters
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && checked
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .nth(parameter_index as usize)
        .ok_or(LoweringError::Unsupported(missing))
}

/// Whether `symbol` binds exactly one parameter of `state` and no authored
/// local of `state` rebinds it, so an authored name of `symbol` can only mean
/// that parameter.
fn parameter_binding_is_unique(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    symbol: symbols::SymbolHandle,
) -> bool {
    checked
        .state_parameters(state)
        .iter()
        .filter(|parameter| parameter.symbol == symbol)
        .count()
        == 1
        && !checked
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::LocalData(local) if local.symbol == symbol)
            })
}

/// The one authored local of `state` that binds `symbol`, declared before
/// statement `before` with an initializer or authenticated default storage.
/// `missing` is the lane's
/// diagnostic when no local binds `symbol`; `unestablished` covers a second
/// binding, a declaration at or after `before`, and a stale initializer.
fn preceding_local<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    symbol: symbols::SymbolHandle,
    before: usize,
    missing: &'static str,
    unestablished: &'static str,
) -> Result<&'a checked_trees::statement::TableLocalData, LoweringError> {
    let mut locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
            _ => None,
        });
    let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(missing))?;
    if locals.next().is_some()
        || ordinal >= before
        || if local.initial_value.is_valid() {
            !checked
                .expression_table
                .expression_is_valid(local.initial_value)
        } else {
            crate::emission::call_source_custody::projected_receivers::owned_local_declaration(
                checked, state, before, symbol,
            )
            .is_none()
        }
    {
        return unsupported(unestablished);
    }
    Ok(local)
}

// A retained computation cannot make a not-yet-established source local
// available. This checks declaration order, not pure-expression semantics.
// Earlier mutable locals remain available to the storage-read evaluator.
fn validate_local_availability(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
    expressions: &[ExpressionHandle],
) -> Result<(), LoweringError> {
    let (_, state) = authored_state(checked, state)?;
    let pending = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement as usize..)
        .ok_or(LoweringError::Unsupported(
            "computed value has no authored declaration position",
        ))?;
    for expression in expressions {
        let ExpressionNode::Name(path) = checked.expression_table.expression(*expression) else {
            continue;
        };
        if path.symbol.is_valid()
            && path.head_symbol.is_valid()
            && path.symbol != path.head_symbol
            && checked
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1
        {
            return unsupported("computed value has inconsistent resolved name identities");
        }
        if pending.iter().any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if local.symbol.is_valid()
                    && (local.symbol == path.symbol || local.symbol == path.head_symbol))
        }) {
            return unsupported("computed value reads a local before its establishment");
        }
    }
    Ok(())
}

// This is expression-tree membership only. Call ordinals remain the retained
// semantic traversal's identity, and unselected syntax need not have a flow row.
pub(crate) fn authored_expressions(
    checked: &CheckedTrees,
    root: ExpressionHandle,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    expression_membership(checked, root, false)
}

// Availability still uses the complete authored tree. Only call coverage and
// executable operand custody exclude independently proved dead Match arms.
fn expression_membership(
    checked: &CheckedTrees,
    root: ExpressionHandle,
    executable_only: bool,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    let table = &checked.expression_table;
    let mut expressions = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(root, false)];
    while let Some((expression, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            expressions.push(expression);
            continue;
        }
        if !table.expression_is_valid(expression) {
            return unsupported("computed invocation has a stale authored expression");
        }
        if active.contains(&expression) {
            return unsupported("computed invocation has a cyclic authored expression");
        }
        if expressions.contains(&expression) {
            continue;
        }
        active.push(expression);
        pending.push((expression, true));
        let mut children = Vec::new();
        match table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                children.push(atomic.value);
                if atomic.result.is_valid() {
                    children.push(atomic.result);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                children.extend_from_slice(table.expression_handles(*elements))
            }
            ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
            ExpressionNode::Match(dispatch) => {
                if executable_only
                    && let Some(selected) =
                        operand_scopes::anonymous_match_value(checked, expression)
                {
                    pending.push((selected, false));
                    continue;
                }
                children.push(dispatch.subject);
                let arms = table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return unsupported("computed dispatch has a stale authored arm span");
                }
                for arm in arms {
                    if let checked_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        children.push(pattern);
                    }
                    children.push(arm.value);
                }
            }
            ExpressionNode::Borrow(borrow) => children.push(borrow.target),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    children.push(call.receiver);
                }
                children.extend_from_slice(table.expression_handles(call.arguments));
            }
            ExpressionNode::Cast(cast) => children.push(cast.value),
            ExpressionNode::Indexed(indexed) => {
                children.extend([indexed.collection, indexed.index])
            }
            ExpressionNode::Member(member) => children.push(member.receiver),
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    children.push(range.start);
                }
                if range.end.is_valid() {
                    children.push(range.end);
                }
            }
            ExpressionNode::StructLiteral(literal) => children.extend(
                table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Unary(unary) => children.push(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(expressions)
}

#[cfg(test)]
mod tests {

    use super::{CheckedTrees, ExpressionNode, authored_expressions};
    use checked_trees::expression::{BinaryOperator, TableBinaryExpression};

    #[test]
    fn authored_membership_rejects_expression_backedges() {
        let mut checked = CheckedTrees::default();
        let leaf = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let root = checked
            .typed
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: leaf,
                operator: BinaryOperator::And,
                right: leaf,
            }));
        let ExpressionNode::Binary(binary) = checked.typed.expression_table.expression_mut(root)
        else {
            panic!("binary source root");
        };
        binary.right = root;
        assert!(authored_expressions(&checked, root).is_err());
    }
}
