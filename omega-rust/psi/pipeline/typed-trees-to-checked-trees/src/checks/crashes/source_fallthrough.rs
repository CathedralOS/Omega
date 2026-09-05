//! Stable source-site consequences of earlier, unselected transition arms.
//!
//! Entry scalar parameters are immutable value snapshots. Their facts survive
//! unrelated local writes and calls; storage and local predicates do not acquire
//! that authority. Project Boolean consequences before canonicalizing names, so
//! an opaque sibling call cannot erase a safe fact or impersonate a parameter.

use checked_trees::CrashSiteLocation;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionGuardNode};

pub(super) struct SiteFallthrough {
    pub location: CrashSiteLocation,
    pub guards: Vec<(ExpressionHandle, bool)>,
}

pub(super) fn collect(program: &TypedTrees, machine: &Machine) -> Vec<SiteFallthrough> {
    let Some(entry) = program.machine_states(machine).first() else {
        return Vec::new();
    };
    let parameters = program.state_parameters(entry);
    let mut sites = Vec::new();
    for state in program.machine_states(machine) {
        let mut guards = Vec::new();
        for (ordinal, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            let StatementNode::Transition(transition) = statement else {
                // Only immutable entry snapshots have been retained. Neither
                // an intervening assignment nor a call can change their value.
                continue;
            };
            if matches!(transition.exit, TransitionExit::Crash(_)) {
                sites.push(SiteFallthrough {
                    location: CrashSiteLocation::new(
                        state.symbol,
                        u32::try_from(ordinal).expect("statement ordinal fits u32"),
                    ),
                    guards: guards.clone(),
                });
            }
            match transition.guard {
                TransitionGuardNode::When(guard)
                    if program
                        .statement_table
                        .transition_target_is_valid(transition.target)
                        && !transition.continuation.is_valid() =>
                {
                    collect_stable_consequences(program, parameters, guard, true, &mut guards);
                }
                _ => guards.clear(),
            }
        }
    }
    sites
}

fn collect_stable_consequences(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
    negated: bool,
    output: &mut Vec<(ExpressionHandle, bool)>,
) {
    if is_entry_snapshot_expression(program, parameters, expression) {
        output.push((expression, negated));
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            collect_stable_consequences(program, parameters, unary.operand, !negated, output);
        }
        ExpressionNode::Binary(binary)
            if (!negated && binary.operator == BinaryOperator::And)
                || (negated && binary.operator == BinaryOperator::Or) =>
        {
            collect_stable_consequences(program, parameters, binary.left, negated, output);
            collect_stable_consequences(program, parameters, binary.right, negated, output);
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) =>
        {
            let operand_and_literal = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Boolean(literal), _) => Some((binary.right, *literal)),
                (_, ExpressionNode::Boolean(literal)) => Some((binary.left, *literal)),
                _ => None,
            };
            if let Some((operand, literal)) = operand_and_literal {
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                collect_stable_consequences(
                    program,
                    parameters,
                    operand,
                    equality_is_negated == literal,
                    output,
                );
            }
        }
        _ => {}
    }
}

fn is_entry_snapshot_expression(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) => true,
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            parameters.iter().any(|parameter| {
                parameter.symbol.is_valid()
                    && parameter.symbol == path.symbol
                    && parameter.symbol == path.head_symbol
                    && !parameter.is_mutable
                    && !parameter.is_self
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some()
                    && matches!(members, [member] if member == &parameter.name)
            })
        }
        ExpressionNode::Unary(unary) => {
            is_entry_snapshot_expression(program, parameters, unary.operand)
        }
        ExpressionNode::Binary(binary) => {
            is_entry_snapshot_expression(program, parameters, binary.left)
                && is_entry_snapshot_expression(program, parameters, binary.right)
        }
        _ => false,
    }
}
