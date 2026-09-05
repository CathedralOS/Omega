//! Width-gate custody for anonymous expressions with checked scalar consumers.

use super::*;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(in crate::literals) fn append_destination_literals(
    program: &TypedTrees,
    blessed: &mut Vec<ExpressionHandle>,
) {
    let admitted = |destination, expression| {
        has_large_leaf(program, expression)
            && program.arithmetic_domain_for_type_reference(destination) == ArithmeticDomain::Exact
            && program
                .primitive_type_reference(destination)
                .is_some_and(|primitive| {
                    land_anonymous_integer_expression(
                        program,
                        expression,
                        primitive,
                        |expression| has_anonymous_operator_meaning(program, expression),
                    )
                    .is_some()
                })
    };
    let mut owned = Vec::new();
    let mut other_roots = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::Expression(expression) => {
                        if admitted(state.return_type, *expression) {
                            append_tree(program, *expression, &mut owned);
                        } else {
                            other_roots.push(*expression);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            other_roots.push(guard);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Value(expression)
                                    if transition.exit
                                        == typed_trees::statement::TransitionExit::Ordinary
                                        && admitted(state.return_type, *expression) =>
                                {
                                    append_tree(program, *expression, &mut owned)
                                }
                                TransitionTargetNode::Value(expression) => {
                                    other_roots.push(*expression)
                                }
                                TransitionTargetNode::Named { arguments, .. } => other_roots
                                    .extend(program.statement_table.expression_handles(*arguments)),
                                _ => {}
                            }
                        }
                    }
                    StatementNode::LocalData(local) => {
                        if admitted(local.type_reference, local.initial_value) {
                            append_tree(program, local.initial_value, &mut owned);
                        } else {
                            other_roots.push(local.initial_value);
                        }
                    }
                    StatementNode::Assignment(assignment) => {
                        other_roots.push(assignment.target);
                        let destination = crate::places::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        )
                        .or_else(|| {
                            crate::places::declared_indexed_projection_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                        });
                        if destination.is_some_and(|destination| {
                            admitted(
                                crate::places::assignment_value_type(program, destination),
                                assignment.value,
                            )
                        }) {
                            append_tree(program, assignment.value, &mut owned);
                        } else {
                            other_roots.push(assignment.value);
                        }
                    }
                    StatementNode::Call(call) => other_roots
                        .extend(program.statement_table.expression_handles(call.arguments)),
                    StatementNode::AssemblyFact(fact) => other_roots.push(fact.expression),
                }
            }
        }
    }
    if owned.is_empty() {
        return;
    }
    // A shared node is not globally granted a new width position merely
    // because one of its uses has a checked destination. An external parent or
    // unsupported executable root retains the old gate for that shared part.
    let mut excluded = Vec::new();
    for root in other_roots {
        if owned.contains(&root) {
            append_tree(program, root, &mut excluded);
        }
    }
    for (parent, node) in program.expression_table.expression_entries() {
        if owned.contains(&parent) {
            continue;
        }
        children(program, node, |child| {
            if owned.contains(&child) {
                append_tree(program, child, &mut excluded);
            }
        });
    }
    for expression in owned {
        if !excluded.contains(&expression)
            && matches!(program.expression_table.expression(expression), ExpressionNode::Integer(literal) if literal.value_i64().is_none())
        {
            blessed.push(expression);
        }
    }
}

fn has_large_leaf(program: &TypedTrees, root: ExpressionHandle) -> bool {
    let mut pending = vec![root];
    let mut seen = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) || seen.contains(&expression) {
            continue;
        }
        seen.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal)
                if literal.landing().is_none() && literal.value_i64().is_none() =>
            {
                return true;
            }
            ExpressionNode::Binary(binary) => {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            _ => {}
        }
    }
    false
}

fn append_tree(
    program: &TypedTrees,
    root: ExpressionHandle,
    collected: &mut Vec<ExpressionHandle>,
) {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression)
            || collected.contains(&expression)
        {
            continue;
        }
        collected.push(expression);
        children(
            program,
            program.expression_table.expression(expression),
            |child| pending.push(child),
        );
    }
}

fn children(program: &TypedTrees, node: &ExpressionNode, mut child: impl FnMut(ExpressionHandle)) {
    match node {
        ExpressionNode::Binary(binary) => {
            child(binary.left);
            child(binary.right);
        }
        ExpressionNode::Unary(unary) => child(unary.operand),
        ExpressionNode::Borrow(borrow) => child(borrow.target),
        ExpressionNode::Cast(cast) => child(cast.value),
        ExpressionNode::Atomic(atomic) => {
            child(atomic.value);
            child(atomic.result);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                child(*element);
            }
        }
        ExpressionNode::Call(call) => {
            child(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                child(*argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            child(indexed.collection);
            child(indexed.index);
        }
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Range(range) => {
            child(range.start);
            child(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                child(field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
