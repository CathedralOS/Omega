use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use validation::CallFrameResolver;

use super::patterns;
use super::write_preservation::prefix_preserves_path;

/// Every self-edge rebuilds the exact ranked field with a positive subtraction.
/// Range formation and construction remain obligations of ordinary checking.
pub(super) fn state_has_proven_self_loop(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    decreases: ExpressionHandle,
    field: &Identifier,
    field_symbol: SymbolHandle,
    owner: SymbolHandle,
) -> bool {
    let ExpressionNode::Name(subject) = program.expression_table.expression(decreases) else {
        return false;
    };
    let Some((argument_index, parameter)) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find(|(_, parameter)| exact_parameter(program, decreases, parameter.symbol))
    else {
        return false;
    };
    let Some(frames) = CallFrameResolver::new(program) else {
        return false;
    };
    let rank_path = format!("{}.{}", parameter.name.as_str(), field.as_str());
    let statements = program.statement_table.statements(state.statement_nodes);
    let edges = patterns::edges_to_state(program, state, state.symbol);
    !edges.is_empty()
        && edges.iter().all(|edge| {
            let Some(argument) = edge.arguments.get(argument_index).copied() else {
                return false;
            };
            let ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(argument)
            else {
                return false;
            };
            if literal.type_symbol != owner
                || literal.case_symbol.is_some()
                || literal.case_name.is_some()
            {
                return false;
            }
            let mut fields = program
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .filter(|candidate| {
                    candidate.field_symbol == field_symbol && candidate.name == *field
                });
            let Some(value) = fields.next() else {
                return false;
            };
            if fields.next().is_some() {
                return false;
            }
            let ExpressionNode::Binary(subtraction) =
                program.expression_table.expression(value.value)
            else {
                return false;
            };
            if subtraction.operator != BinaryOperator::Subtract
                || !exact_member(
                    program,
                    subtraction.left,
                    subject.symbol,
                    field_symbol,
                    field,
                )
                || !validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    value.value,
                )
            {
                return false;
            }
            let prefix = &statements[..=edge.statement_ordinal];
            let Some((minimum_step, maximum_step)) =
                preserved_bounds(program, machine, state, subtraction.right, &frames, prefix)
            else {
                return false;
            };
            if minimum_step <= 0 || !prefix_preserves_path(&frames, machine, prefix, &rank_path) {
                return false;
            }
            edge.guards.iter().any(|guard| {
                if !validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    guard.expression,
                ) {
                    return false;
                }
                let Some((left, operator, right)) = patterns::comparison(program, *guard) else {
                    return false;
                };
                let (bound, strict) = match operator {
                    BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
                        if exact_member(program, left, subject.symbol, field_symbol, field) =>
                    {
                        (right, operator == BinaryOperator::Greater)
                    }
                    BinaryOperator::Less | BinaryOperator::LessOrEqual
                        if exact_member(program, right, subject.symbol, field_symbol, field) =>
                    {
                        (left, operator == BinaryOperator::Less)
                    }
                    _ => return false,
                };
                // Equal trees name the same immutable inputs, not matching text.
                // Otherwise a guard's minimum must cover the step's maximum.
                if program
                    .expression_table
                    .expressions_structurally_equal(bound, subtraction.right)
                {
                    return true;
                }
                preserved_bounds(program, machine, state, bound, &frames, prefix).is_some_and(
                    |(minimum_bound, _)| {
                        i128::from(minimum_bound) + i128::from(strict) >= i128::from(maximum_step)
                    },
                )
            })
        })
}

fn exact_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
        if symbol.is_valid() && path.symbol == symbol && path.head_symbol == symbol
            && program.expression_table.name_path_members(path.members).len() == 1)
}

fn exact_member(
    program: &TypedTrees,
    expression: ExpressionHandle,
    subject: SymbolHandle,
    field: SymbolHandle,
    name: &Identifier,
) -> bool {
    matches!(program.expression_table.expression(expression), ExpressionNode::Member(member)
        if field.is_valid() && member.member_symbol == field && member.member == *name && member.case_variant.is_none()
            && exact_parameter(program, member.receiver, subject))
}

/// Declared bounds remain valid only while every input path survives the prefix.
/// Step inputs may change on arrival; they are not invocation-fixed endpoints.
fn preserved_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    frames: &CallFrameResolver<'_>,
    prefix: &[StatementNode],
) -> Option<(i64, i64)> {
    let bounds =
        validation::immutable_integer_expression_bounds(program, machine, state, expression)?;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(_) => {}
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Name(_) => {
                let parameter = program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| exact_parameter(program, expression, parameter.symbol))?;
                if !prefix_preserves_path(frames, machine, prefix, parameter.name.as_str()) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(bounds)
}
