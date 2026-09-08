use facts::NormalizedWriteFrame;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::statement::StatementNode;

use super::patterns;

/// Proves a self-loop terminates under a struct-view measure that projects a
/// single field, e.g. `measure Card::PowerOrder(card: Card) -> usize { card.power }`
/// used as `terminates by card -> Card::PowerOrder`.
///
/// The decreasing value is the whole struct value; each recursive call argument is
/// a struct literal whose projected field must strictly decrease relative to the
/// projected field of the decreasing value, guarded by `<decrease>.field > 0`.
pub(super) fn state_has_proven_self_loop(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    decreases: ExpressionHandle,
    field: &Identifier,
) -> bool {
    let Some((parameter, argument_index)) =
        patterns::parameter_and_argument_index_matched_by_expression(program, state, decreases)
    else {
        return false;
    };

    let Some(frames) = validation::CallFrameResolver::new(program) else {
        return false;
    };
    let rank_path = format!("{}.{}", parameter.name.as_str(), field.as_str());
    let statements = program.statement_table.statements(state.statement_nodes);
    statements
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| {
            patterns::guarded_self_loop(program, state, statement)
                .map(|self_loop| (ordinal, self_loop))
        })
        .any(|(ordinal, self_loop)| {
            let Some(argument) = self_loop.arguments.get(argument_index).copied() else {
                return false;
            };
            prefix_preserves_rank(&frames, machine, &statements[..=ordinal], &rank_path)
                && validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    self_loop.guard,
                )
                && guard_is_positive_member(program, self_loop.guard, parameter, field)
                && argument_rebuilds_with_field_minus_one(
                    program, machine, state, argument, parameter, field,
                )
        })
}

fn prefix_preserves_rank<'program>(
    frames: &validation::CallFrameResolver<'program>,
    machine: &'program typed_trees::machine::Machine,
    statements: &'program [StatementNode],
    rank_path: &str,
) -> bool {
    let disjoint = |frame: NormalizedWriteFrame| {
        frame.into_complete_paths().is_some_and(|paths| {
            paths
                .iter()
                .all(|path| !validation::frame_paths_overlap(path, rank_path))
        })
    };
    statements.iter().all(|statement| {
        let direct_writes_preserve = match statement {
            StatementNode::Assignment(_) => {
                disjoint(frames.assignment_write_frame(machine, statement))
            }
            StatementNode::Call(call) => disjoint(frames.may_write_frame(machine, call)),
            _ => true,
        };
        direct_writes_preserve && disjoint(frames.statement_value_write_frame(machine, statement))
    })
}

fn guard_is_positive_member(
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    field: &Identifier,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    let ExpressionNode::Integer(literal) = program.expression_table.expression(binary.right) else {
        return false;
    };
    let Some(floor) = literal.value_i64() else {
        return false;
    };
    let positive = match binary.operator {
        BinaryOperator::Greater => floor >= 0,
        BinaryOperator::GreaterOrEqual => floor > 0,
        _ => false,
    };
    positive
        && patterns::expression_is_parameter_member(program, binary.left, parameter, field.as_str())
}

fn argument_rebuilds_with_field_minus_one(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    argument: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    field: &Identifier,
) -> bool {
    let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(argument)
    else {
        return false;
    };

    program
        .expression_table
        .struct_fields(struct_literal.fields)
        .iter()
        .any(|literal_field| {
            literal_field.name.as_str() == field.as_str()
                && validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    literal_field.value,
                )
                && field_value_is_member_minus_one(program, literal_field.value, parameter, field)
        })
}

fn field_value_is_member_minus_one(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    field: &Identifier,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(value) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Subtract)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, field.as_str())
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
}
