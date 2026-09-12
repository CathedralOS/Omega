//! Executable expression collection outside generic templates.

use super::super::*;

pub(in crate::generic_data) fn concrete_machine_expression_handles(
    syntax: &SyntaxTrees,
) -> HashSet<u32> {
    let mut handles = HashSet::new();
    for state in concrete_machine_state_handles(syntax) {
        let state = syntax.items.state(state);
        for statement in syntax.items.statements(state.statements) {
            collect_statement_expression_handles(syntax, *statement, &mut handles);
        }
    }
    handles
        .into_iter()
        .map(|handle| handle.arena_index())
        .collect()
}

pub(in crate::generic_data) fn concrete_machine_state_handles(
    syntax: &SyntaxTrees,
) -> Vec<syntax_trees::item::StateHandle> {
    syntax.root_items().filter_map(|item| {
        let Item::Machine(machine) = item else { return None; };
        if !machine.type_parameters.is_empty() || machine.attached_data.as_ref().is_some_and(|attached|
            syntax.root_items().any(|item| matches!(item, Item::Data(data) if data.name == *attached && !data.type_parameters.is_empty()))) {
            return None;
        }
        Some(syntax.items.state_handles(machine.states))
    }).flatten().copied().collect()
}

pub(in crate::generic_data) fn collect_statement_expression_handles(
    syntax: &SyntaxTrees,
    statement: syntax_trees::statement::StatementHandle,
    handles: &mut HashSet<ExpressionHandle>,
) {
    use syntax_trees::statement::{TransitionGuardNode, TransitionTargetNode};
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
    handles: &mut HashSet<ExpressionHandle>,
) {
    if !expression.is_valid() || !handles.insert(expression) {
        return;
    }
    match syntax.expressions.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            collect_expression_handles(syntax, dispatch.subject, handles);
            for arm in syntax.expressions.match_arms(dispatch.arms) {
                if let syntax_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    collect_expression_handles(syntax, pattern, handles);
                }
                collect_expression_handles(syntax, arm.value, handles);
            }
        }
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

/// Advisory constructor rewriting borrows the actual value frontier. A captured
/// Name remains authored syntax for the ordinary resolver; no binding is chosen
/// here. Callers visit concrete machines, whose template binders are already gone.
pub(in crate::generic_data) struct ConstructorFrontier<'a> {
    pub parameters: HandleSpan<syntax_trees::item::StateParameterHandle>,
    pub prior_statements: &'a [syntax_trees::statement::StatementHandle],
}

impl ConstructorFrontier<'_> {
    pub fn captures(&self, syntax: &SyntaxTrees, path: HandleSpan<Identifier>) -> bool {
        let Some(head) = syntax.expressions.identifier_path_members(path).first() else {
            return false;
        };
        self.prior_statements.iter().rev().any(|statement| matches!(
            syntax.statements.statement(*statement), StatementNode::LocalData(local) if local.name == *head
        )) || syntax.items.state_parameters(self.parameters).iter().any(|parameter|
            syntax.items.state_parameter(*parameter).name == *head)
    }
}
