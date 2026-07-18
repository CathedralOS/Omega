use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::name::Identifier;

use super::{EdgeClass, patterns};

/// Proves a self-loop terminates under a lexicographic `measure`, comparing the
/// ordered projection components left-to-right: the first component that strictly
/// decreases proves termination, provided every earlier component is
/// non-increasing.
pub(super) fn state_has_proven_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    fields: &[Identifier],
) -> bool {
    if fields.is_empty() {
        return false;
    }

    let Some((parameter, argument_index)) =
        patterns::parameter_and_argument_index_matched_by_expression(program, state, decreases)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            let Some(argument) = self_loop.arguments.get(argument_index).copied() else {
                return false;
            };
            matches!(
                argument_lexicographic_comparison(program, argument, parameter, fields),
                Some(Comparison::StrictlyDecreasing)
            )
        })
}

pub(super) fn classify_cross_machine_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
    fields: &[Identifier],
) -> EdgeClass {
    let Some((parameter, _)) =
        patterns::parameter_and_argument_index_matched_by_expression(program, source, decreases)
    else {
        return EdgeClass::Unknown;
    };
    let Some(argument_index) = program
        .state_parameters(target)
        .iter()
        .filter(|candidate| !candidate.is_self)
        .position(|candidate| candidate.name == parameter.name)
    else {
        return EdgeClass::Unknown;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return EdgeClass::Unknown;
    };

    if patterns::expression_is_parameter(program, argument, parameter) {
        return EdgeClass::NonIncreasing;
    }
    match argument_lexicographic_comparison(program, argument, parameter, fields) {
        Some(Comparison::StrictlyDecreasing) => EdgeClass::Strict,
        Some(Comparison::NonIncreasing) => EdgeClass::NonIncreasing,
        None => EdgeClass::Unknown,
    }
}

fn argument_lexicographic_comparison(
    program: &omega_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    fields: &[Identifier],
) -> Option<Comparison> {
    let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(argument)
    else {
        return None;
    };
    let literal_fields = program
        .expression_table
        .struct_fields(struct_literal.fields);

    for field in fields.iter() {
        let Some(value) = literal_fields
            .iter()
            .find(|literal_field| literal_field.name.as_str() == field.as_str())
            .map(|literal_field| literal_field.value)
        else {
            return None;
        };

        match component_comparison(program, value, parameter, field) {
            Some(Comparison::StrictlyDecreasing) => {
                return Some(Comparison::StrictlyDecreasing);
            }
            Some(Comparison::NonIncreasing) => continue,
            None => return None,
        }
    }

    Some(Comparison::NonIncreasing)
}

enum Comparison {
    StrictlyDecreasing,
    NonIncreasing,
}

fn component_comparison(
    program: &omega_typed_trees::TypedTrees,
    value: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    field: &Identifier,
) -> Option<Comparison> {
    // `<param>.field - positive` strictly decreases.
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(value) {
        if matches!(binary.operator, BinaryOperator::Subtract)
            && patterns::expression_is_parameter_member(
                program,
                binary.left,
                parameter,
                field.as_str(),
            )
            && matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Integer(literal)
                    if literal.value_i64().is_some_and(|literal| literal > 0)
            )
        {
            return Some(Comparison::StrictlyDecreasing);
        }
    }

    // `<param>.field` unchanged is non-increasing.
    if patterns::expression_is_parameter_member(program, value, parameter, field.as_str()) {
        return Some(Comparison::NonIncreasing);
    }

    None
}
