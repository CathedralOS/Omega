use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use validation::{CallFrameResolver, ProjectionStep};

use super::patterns;
use super::write_preservation::prefix_preserves_path;

/// Every self-edge rebuilds the exact ranked field with a positive subtraction.
/// Range formation and construction remain obligations of ordinary checking.
/// A nested `path` is rebuilt literal by literal down to the ranked field; a
/// borrowed subject arrives as a borrow of that literal, and the ranked path
/// through the binding must stay unwritten like any other input path.
pub(super) fn state_has_proven_self_loop(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    decreases: ExpressionHandle,
    path: &[ProjectionStep],
    field: &Identifier,
    field_symbol: SymbolHandle,
    owner: SymbolHandle,
    call_frames: Option<&CallFrameResolver<'_>>,
) -> bool {
    let Some(subject) = program.expression_table.name_path_node(decreases) else {
        return false;
    };
    let Some((argument_index, parameter)) = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| exact_parameter(program, decreases, parameter.symbol))
    else {
        return false;
    };
    let mut owned_frames = None;
    let Some(frames) = crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
    else {
        return false;
    };
    let mut rank_path = parameter.name.as_str().to_owned();
    for step in path {
        rank_path.push('.');
        rank_path.push_str(step.field.as_str());
    }
    rank_path.push('.');
    rank_path.push_str(field.as_str());
    let statements = program.statement_table.statements(state.statement_nodes);
    let edges = patterns::edges_to_state(program, state, state.symbol);
    !edges.is_empty()
        && edges.iter().all(|edge| {
            let Some(argument) = edge.arguments.get(argument_index).copied() else {
                return false;
            };
            // A borrowed subject is rebuilt as a borrow of the new literal.
            let argument = match program.expression_table.expression(argument) {
                ExpressionNode::Borrow(borrow) => borrow.target,
                _ => argument,
            };
            // Walk the literal chain: each record-typed step must be rebuilt
            // as a literal of the next exact record, and the final literal
            // carries the ranked field.
            let mut literal_owner = path.first().map_or(owner, |step| step.owner);
            let mut current = argument;
            for step in path {
                let Some(value) = validation::unique_literal_field(
                    program,
                    current,
                    literal_owner,
                    step.field_symbol,
                    &step.field,
                ) else {
                    return false;
                };
                current = value;
                literal_owner = path
                    .iter()
                    .skip_while(|candidate| candidate.field_symbol != step.field_symbol)
                    .nth(1)
                    .map_or(owner, |next| next.owner);
            }
            let Some(value) =
                validation::unique_literal_field(program, current, owner, field_symbol, field)
            else {
                return false;
            };
            let Some(subtraction) = program.expression_table.binary_node(value) else {
                return false;
            };
            if subtraction.operator != BinaryOperator::Subtract
                || !exact_projection(
                    program,
                    subtraction.left,
                    subject.symbol,
                    path,
                    field_symbol,
                    field,
                )
                || !validation::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    Some(state),
                    value,
                )
            {
                return false;
            }
            let prefix = &statements[..=edge.statement_ordinal];
            let Some((minimum_step, maximum_step)) =
                preserved_bounds(program, machine, state, subtraction.right, frames, prefix)
            else {
                return false;
            };
            if minimum_step <= 0 || !prefix_preserves_path(frames, machine, prefix, &rank_path) {
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
                        if exact_projection(
                            program,
                            left,
                            subject.symbol,
                            path,
                            field_symbol,
                            field,
                        ) =>
                    {
                        (right, operator == BinaryOperator::Greater)
                    }
                    BinaryOperator::Less | BinaryOperator::LessOrEqual
                        if exact_projection(
                            program,
                            right,
                            subject.symbol,
                            path,
                            field_symbol,
                            field,
                        ) =>
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
                preserved_bounds(program, machine, state, bound, frames, prefix).is_some_and(
                    |(minimum_bound, _)| {
                        i128::from(minimum_bound) + i128::from(strict) >= i128::from(maximum_step)
                    },
                )
            })
        })
}

fn exact_parameter(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    matches!(program.expression_table.expression(expression), ExpressionNode::Name(path)
        if program.expression_table.name_path_is_single_member(path)
            && path.symbol == symbol)
}

/// `subject.path[0]. ... .path[n].field`: the exact member chain of the ranked
/// projection, every step by resolved field symbol and spelling, rooted at
/// the exact subject parameter (owned or borrowed).
fn exact_projection(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    subject: SymbolHandle,
    path: &[ProjectionStep],
    field: SymbolHandle,
    name: &Identifier,
) -> bool {
    let Some(member) = program.expression_table.member_node(expression) else {
        return false;
    };
    if !field.is_valid()
        || member.member_symbol != field
        || member.member != *name
        || member.case_variant.is_some()
    {
        return false;
    }
    match path.split_last() {
        None => exact_parameter(program, member.receiver, subject),
        Some((last, outer)) => exact_projection(
            program,
            member.receiver,
            subject,
            outer,
            last.field_symbol,
            &last.field,
        ),
    }
}

/// Declared bounds remain valid only while every input path survives the prefix.
/// Step inputs may change on arrival; they are not invocation-fixed endpoints.
fn preserved_bounds(
    program: &dyn ::validation::CheckingInput,
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
