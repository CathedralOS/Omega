//! Symbol-based source uses are shared by loan analysis and cleanup planning.
//! Whole-owner retirement must include scalar siblings as well as stored loans;
//! borrowing a leaf does not make its owner's remaining data dead.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

pub fn statement_uses_symbol(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    symbol: SymbolHandle,
) -> bool {
    match statement {
        StatementNode::RootBinding(binding) => {
            expression_uses_symbol(program, binding.receiver, symbol)
                || (binding.implementation_operand.is_valid()
                    && expression_uses_symbol(program, binding.implementation_operand, symbol))
        }
        StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => false,
        StatementNode::Assignment(assignment) => {
            expression_uses_symbol(program, assignment.target, symbol)
                || expression_uses_symbol(program, assignment.value, symbol)
        }
        StatementNode::Call(call) => {
            call.receiver_symbol == symbol
                || program
                    .statement_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        StatementNode::Expression(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        StatementNode::LocalData(local_data) => {
            expression_uses_symbol(program, local_data.initial_value, symbol)
        }
        StatementNode::Transition(transition) => {
            transition_guard_uses_symbol(program, transition.guard, symbol)
                || transition_target_uses_symbol(
                    program,
                    program.statement_table.transition_target(transition.target),
                    symbol,
                )
                || transition_target_uses_symbol(
                    program,
                    program
                        .statement_table
                        .transition_target(transition.continuation),
                    symbol,
                )
        }
    }
}

pub(crate) fn expression_uses_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Match(dispatch) => {
            expression_uses_symbol(program, dispatch.subject, symbol)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        (matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_uses_symbol(program, pattern, symbol)))
                            || expression_uses_symbol(program, arm.value, symbol)
                    })
        }
        typed_trees::expression::ExpressionNode::Atomic(atomic) => {
            expression_uses_symbol(program, atomic.value, symbol)
        }
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| expression_uses_symbol(program, *value, symbol)),
        typed_trees::expression::ExpressionNode::Binary(binary) => {
            expression_uses_symbol(program, binary.left, symbol)
                || expression_uses_symbol(program, binary.right, symbol)
        }
        typed_trees::expression::ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && expression_uses_symbol(program, call.receiver, symbol))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        typed_trees::expression::ExpressionNode::Cast(cast) => {
            expression_uses_symbol(program, cast.value, symbol)
        }
        typed_trees::expression::ExpressionNode::Indexed(indexed) => {
            expression_uses_symbol(program, indexed.collection, symbol)
                || expression_uses_symbol(program, indexed.index, symbol)
        }
        typed_trees::expression::ExpressionNode::Range(range) => {
            (range.start.is_valid() && expression_uses_symbol(program, range.start, symbol))
                || (range.end.is_valid() && expression_uses_symbol(program, range.end, symbol))
        }
        typed_trees::expression::ExpressionNode::Member(member) => {
            member.member_symbol == symbol
                || expression_uses_symbol(program, member.receiver, symbol)
        }
        typed_trees::expression::ExpressionNode::Borrow(inner_expression) => {
            expression_uses_symbol(program, inner_expression.target, symbol)
        }
        typed_trees::expression::ExpressionNode::Unary(unary) => {
            expression_uses_symbol(program, unary.operand, symbol)
        }
        typed_trees::expression::ExpressionNode::Name(path) => {
            program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .first()
                .copied()
                .filter(|member| member.is_valid())
                .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
                .or_else(|| path.symbol.is_valid().then_some(path.symbol))
                .is_some_and(|path_symbol| path_symbol == symbol)
                || program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                    .contains(&symbol)
                || path.symbol == symbol
        }
        typed_trees::expression::ExpressionNode::StructLiteral(struct_literal) => program
            .expression_table
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| expression_uses_symbol(program, field.value, symbol)),
        typed_trees::expression::ExpressionNode::Boolean(_)
        | typed_trees::expression::ExpressionNode::Float(_)
        | typed_trees::expression::ExpressionNode::UndeterminedNaN(_)
        | typed_trees::expression::ExpressionNode::Integer(_)
        | typed_trees::expression::ExpressionNode::String(_)
        | typed_trees::expression::ExpressionNode::ZeroValue(_) => false,
    }
}

fn transition_guard_uses_symbol(
    program: &dyn crate::CheckingInput,
    guard: typed_trees::statement::TransitionGuardNode,
    symbol: SymbolHandle,
) -> bool {
    match guard {
        typed_trees::statement::TransitionGuardNode::Always => false,
        typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_symbol(program, expression, symbol)
        }
    }
}

fn transition_target_uses_symbol(
    program: &dyn crate::CheckingInput,
    target: &typed_trees::statement::TransitionTargetNode,
    symbol: SymbolHandle,
) -> bool {
    match target {
        typed_trees::statement::TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            path.head_symbol == symbol
                || path.symbol == symbol
                || program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| expression_uses_symbol(program, *argument, symbol))
        }
        typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}
