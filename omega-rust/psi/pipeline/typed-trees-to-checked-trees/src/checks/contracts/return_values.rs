//! Exact normal-return occurrences shared by exit proof consumers.

use checked_trees::FlowExitFact;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    statement::{StatementNode, TransitionExit, TransitionTargetNode},
};

pub(super) fn exit_return_expression(
    program: &TypedTrees,
    exit: &FlowExitFact,
) -> ExpressionHandle {
    let Some(state) = crate::find_state_in_machine(program, exit.machine_symbol, exit.state_symbol)
    else {
        return Default::default();
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    match statements.get(exit.statement_index) {
        Some(StatementNode::Expression(value))
            if !exit.transition_target.is_valid()
                && exit.statement_index.checked_add(1) == Some(statements.len()) =>
        {
            *value
        }
        Some(StatementNode::Transition(transition))
            if exit.transition_target.is_valid()
                && [transition.target, transition.continuation]
                    .contains(&exit.transition_target)
                && transition.exit == TransitionExit::Ordinary =>
        {
            match program
                .statement_table
                .transition_target(exit.transition_target)
            {
                TransitionTargetNode::Value(value) => *value,
                _ => Default::default(),
            }
        }
        _ => Default::default(),
    }
}

pub(super) fn is_result_reference(
    program: &TypedTrees,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    // The current typed contract surface retains the synthetic result name,
    // not a result binder handle. An authored entry parameter takes precedence.
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    !program
        .state_parameters(entry)
        .iter()
        .any(|parameter| parameter.name.as_str() == "result")
        && matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
            if matches!(program.expression_table.name_path_members(path.members), [name]
                if name.as_str() == "result"))
}
