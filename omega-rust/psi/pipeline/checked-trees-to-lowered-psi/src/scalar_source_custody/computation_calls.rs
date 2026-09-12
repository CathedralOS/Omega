//! Rejoin computed invocations to captured expression occurrences, not spans.
//! Semantic casts are part of that ordering: equal final tags cannot justify
//! skipping an explicit erasure followed by reintroduction. Only casts whose
//! operand is independently known to be bare may be peeled as payload wrappers.

use super::*;
use checked_trees::{CheckedScalarComputationHandle, CheckedScalarComputationKind};

use crate::attached_unit::primitive_locals::borrows as borrow_rows;
mod dispatch;
mod mixed_arguments;
mod operand_scopes;
mod owned_arguments;
pub(crate) mod primitive_arguments;
mod qualifications;

pub(crate) use mixed_arguments::access_occurrences::rejoin as rejoin_call_accesses;
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
                let fields = crate::scalar_computations::cases::source::membership(
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
                crate::scalar_computations::comparisons::occurrence(
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
                let authored_scope =
                    operand_scopes::value(checked, authored_scope, node.value_source)?;
                for source in expression_membership(checked, authored_scope, true)? {
                    if let ExpressionNode::Cast(cast) = checked.expression_table.expression(source)
                        && operand_scopes::cast_requires_custody(checked, machine, state, cast)?
                    {
                        return unsupported(
                            "pure computation cannot erase a semantic qualification",
                        );
                    }
                }
                crate::scalar_source_custody::validate_storage_read_expression(
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
                        RejoinedComputationArgument::Array {
                            expression,
                            elements,
                        } => {
                            if !scoped_expressions.contains(&expression) {
                                return unsupported(
                                    "computed array escaped its authored argument scope",
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
    }) {
        return unsupported("computed invocation omitted an authored source call");
    }
    Ok(())
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
    use super::*;
    use checked_trees::expression::{BinaryOperator, TableBinaryExpression};

    #[test]
    fn computed_application_rejects_another_same_typed_operation_occurrence() {
        let source = r#"
            machine identity(value: bool) -> bool { value }
            machine first() -> bool { !identity(true) }
            machine second() -> bool { !identity(false) }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let roots = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root.clone())
            .filter(|root| root.role == CheckedScalarExpressionRole::Return)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2);
        for root in &roots {
            let authored = checked
                .facts
                .values
                .scalar_computations
                .nodes
                .get(root.root)
                .authored_root;
            validate_computation_calls(
                &checked,
                root.machine,
                root.state,
                root.statement_ordinal,
                root.root,
                authored,
            )
            .expect("original exact application custody");
        }
        let replacement = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(roots[1].root)
            .authored_root;
        let node = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(roots[0].root);
        let authored = node.authored_root;
        let CheckedScalarComputationKind::Apply {
            source_expression, ..
        } = &mut node.kind
        else {
            panic!("negation retains an application");
        };
        *source_expression = replacement;
        assert!(
            validate_computation_calls(
                &checked,
                roots[0].machine,
                roots[0].state,
                roots[0].statement_ordinal,
                roots[0].root,
                authored,
            )
            .is_err(),
            "same-typed operation cannot replace the authored occurrence"
        );
    }

    #[test]
    fn computation_call_coverage_does_not_require_unselected_syntax() {
        let source = r#"
            machine identity(value: bool) -> bool { value }
            machine choose() -> bool { identity(false) && (false && identity(true)) }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .find(|(_, root)| root.role == CheckedScalarExpressionRole::Return)
            .map(|(_, root)| root)
            .unwrap();
        let authored = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root.root)
            .authored_root;
        let syntax_calls = authored_expressions(&checked, authored)
            .unwrap()
            .iter()
            .filter(|expression| {
                matches!(
                    checked.expression_table.expression(**expression),
                    ExpressionNode::Call(_)
                )
            })
            .count();
        assert_eq!(syntax_calls, 2);
        validate_computation_calls(
            &checked,
            root.machine,
            root.state,
            root.statement_ordinal,
            root.root,
            authored,
        )
        .expect("only selected source-flow calls need computation custody");
    }

    #[test]
    fn computed_field_assignment_rejects_erased_authored_call() {
        let source = r#"
            machine identity(value: bool) -> bool { value }
            data Record { flag: bool; }
            machine Record::replace(&mut self) { self.flag = identity(true); }
        "#;
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
        let root = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .find(|(_, root)| root.role == CheckedScalarExpressionRole::AssignmentValue)
            .map(|(_, root)| root.clone())
            .unwrap();
        let authored = checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get(root.root)
            .authored_root;
        validate_computation_calls(
            &checked,
            root.machine,
            root.state,
            root.statement_ordinal,
            root.root,
            authored,
        )
        .expect("original exact call custody");
        checked
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(root.root)
            .kind = CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(
            Box::new(checked_trees::CheckedBooleanExpression::Constant(true)),
        ));
        assert!(
            validate_computation_calls(
                &checked,
                root.machine,
                root.state,
                root.statement_ordinal,
                root.root,
                authored
            )
            .is_err(),
            "same-typed replacement cannot erase the authored call"
        );
    }

    #[test]
    fn authored_membership_accepts_shared_expression_children() {
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
        let expressions = authored_expressions(&checked, root).unwrap();
        assert_eq!(expressions, vec![leaf, root]);
    }

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
