//! Runtime membership uses the same exact authored selection judgment as
//! checking. It is a tag observation, never equality against a fabricated
//! payload value. The current scalar namespace names parameter referents;
//! constructor and local referents must arrive through ordinary value sequencing.

use super::*;

pub(super) fn lower(
    program: &TypedTrees,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<CheckedBooleanExpression> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::CaseMembership
    ) {
        return None;
    }
    let subject = match program.expression_table.expression(binary.left) {
        ExpressionNode::Borrow(borrow) => borrow.target,
        _ => binary.left,
    };
    let mut path = Vec::new();
    let parameter_position =
        structural_parameter_field_path(program, parameters, subject, &mut path)?;
    let root = parameters.get(parameter_position as usize)?;
    // The existing scalar builder supplies its authored parameter namespace,
    // not a machine handle. Rejoin through the selected root symbol, which is
    // unique to its declaring state; never choose a same-named parameter.
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program.machine_states(machine).iter().find_map(|state| {
            let authored = program.state_parameters(state);
            (authored
                .get(parameter_position as usize)
                .is_some_and(|parameter| parameter.symbol == root.symbol)
                && authored == parameters)
                .then_some((machine, state))
        })
    })?;
    if !validation::has_exact_case_membership_meaning(
        program,
        machine,
        Some(state),
        expression,
        binary,
    ) {
        return None;
    }
    let ExpressionNode::Name(selected) = program.expression_table.expression(binary.right) else {
        return None;
    };
    let case = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(case) if case.symbol == selected.symbol => {
                Some(case)
            }
            _ => None,
        })?;
    Some(CheckedBooleanExpression::StructuralCaseMembership {
        subject: CheckedStructuralParameterField {
            parameter_position,
            path,
        },
        case: case
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| case.name.as_str().to_owned()),
    })
}
