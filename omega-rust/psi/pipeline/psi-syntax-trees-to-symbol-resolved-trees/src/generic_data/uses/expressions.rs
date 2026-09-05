//! Executable expression collection outside generic templates.

use super::super::*;

pub(in crate::generic_data) fn concrete_machine_expression_handles(
    syntax: &SyntaxTrees,
) -> HashSet<u32> {
    let mut handles = HashSet::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        if !machine.type_parameters.is_empty()
            || machine.attached_data.as_ref().is_some_and(|attached| {
                syntax.root_items().any(|item| {
                    matches!(item, Item::Data(definition)
                        if definition.name == *attached && !definition.type_parameters.is_empty())
                })
            })
        {
            continue;
        }
        for state in syntax.tables.items.state_handles(machine.states) {
            let state = syntax.tables.items.state(*state);
            for statement in syntax.tables.items.statements(state.statements) {
                collect_statement_expression_handles(syntax, *statement, &mut handles);
            }
        }
    }
    handles
}

pub(in crate::generic_data) fn collect_statement_expression_handles(
    syntax: &SyntaxTrees,
    statement: psi_syntax_trees::statement::StatementHandle,
    handles: &mut HashSet<u32>,
) {
    use psi_syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
    match syntax.tables.statements.statement(statement) {
        StatementNode::AssemblyFact(fact) => {
            collect_expression_handles(syntax, fact.expression, handles)
        }
        StatementNode::Assignment(assignment) => {
            collect_expression_handles(syntax, assignment.target, handles);
            collect_expression_handles(syntax, assignment.value, handles);
        }
        StatementNode::Call(call) => {
            for argument in syntax.tables.statements.expression_handles(call.arguments) {
                collect_expression_handles(syntax, *argument, handles);
            }
        }
        StatementNode::ProofOutputBindingStatement(binding) => {
            collect_expression_handles(syntax, binding.call, handles)
        }
        StatementNode::Expression(expression) => {
            collect_expression_handles(syntax, *expression, handles)
        }
        StatementNode::LocalData(local) => {
            collect_expression_handles(syntax, local.initial_value, handles)
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_handles(syntax, guard, handles);
            }
            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match syntax.tables.statements.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in syntax.tables.statements.expression_handles(*arguments) {
                            collect_expression_handles(syntax, *argument, handles);
                        }
                    }
                    TransitionTargetNode::Value(value) => {
                        collect_expression_handles(syntax, *value, handles)
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }
        }
    }
}

pub(in crate::generic_data) fn collect_expression_handles(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    handles: &mut HashSet<u32>,
) {
    if !expression.is_valid() || !handles.insert(expression.arena_index()) {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(expressions) => {
            for expression in syntax.expressions.expression_handles(*expressions) {
                collect_expression_handles(syntax, *expression, handles);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            collect_expression_handles(syntax, atomic.value, handles);
            collect_expression_handles(syntax, atomic.result, handles);
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_handles(syntax, binary.left, handles);
            collect_expression_handles(syntax, binary.right, handles);
        }
        ExpressionNode::Cast(cast) => collect_expression_handles(syntax, cast.value, handles),
        ExpressionNode::Call(call) => {
            collect_expression_handles(syntax, call.receiver, handles);
            for argument in syntax.expressions.expression_handles(call.arguments) {
                collect_expression_handles(syntax, *argument, handles);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_handles(syntax, indexed.collection, handles);
            collect_expression_handles(syntax, indexed.index, handles);
        }
        ExpressionNode::Membership(membership) => {
            collect_expression_handles(syntax, membership.value, handles)
        }
        ExpressionNode::Member(member) => {
            collect_expression_handles(syntax, member.receiver, handles)
        }
        ExpressionNode::Borrow(inner) => collect_expression_handles(syntax, inner.target, handles),
        ExpressionNode::Range(range) => {
            collect_expression_handles(syntax, range.start, handles);
            collect_expression_handles(syntax, range.end, handles);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in syntax.expressions.struct_fields(literal.fields) {
                collect_expression_handles(syntax, field.value, handles);
            }
        }
        ExpressionNode::Unary(unary) => collect_expression_handles(syntax, unary.operand, handles),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::SelfValue
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
