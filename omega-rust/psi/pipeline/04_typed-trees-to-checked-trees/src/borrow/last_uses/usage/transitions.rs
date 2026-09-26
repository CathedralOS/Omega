use super::expressions::{expression_uses_local_name, expression_uses_symbol};
use symbols::SymbolHandle;

pub(super) fn transition_guard_uses_symbol(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    guard: symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode,
    symbol: SymbolHandle,
) -> bool {
    match guard {
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::Always => false,
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_symbol(program, expression, symbol)
        }
    }
}

pub(super) fn transition_guard_uses_local_name(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    guard: symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode,
    local_name: &str,
) -> bool {
    match guard {
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::Always => false,
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionGuardNode::When(expression) => {
            expression_uses_local_name(program, expression, local_name)
        }
    }
}

pub(super) fn transition_target_uses_symbol(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    target: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode,
    symbol: SymbolHandle,
) -> bool {
    match target {
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Named {
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
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_symbol(program, *expression, symbol)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::SelfTarget
        | symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}

pub(super) fn transition_target_uses_local_name(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    target: &symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode,
    local_name: &str,
) -> bool {
    match target {
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            program
                .statement_table
                .name_path_members(path.members)
                .first()
                .is_some_and(|member| member.as_str() == local_name)
                || program
                    .statement_table
                    .expression_handles(*arguments)
                    .iter()
                    .any(|argument| expression_uses_local_name(program, *argument, local_name))
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Value(expression) => {
            expression_uses_local_name(program, *expression, local_name)
        }
        symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::SelfTarget
        | symbol_resolved_trees_to_typed_trees::typed_trees::statement::TransitionTargetNode::Terminal => false,
    }
}
