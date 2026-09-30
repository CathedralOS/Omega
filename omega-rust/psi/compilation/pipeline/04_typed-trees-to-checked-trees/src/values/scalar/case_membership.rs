//! Runtime membership uses the same exact authored selection judgment as
//! checking. It is a tag observation, never equality against a fabricated
//! payload value. The current scalar namespace names parameter referents;
//! constructor and local referents must arrive through ordinary value sequencing.
use crate::values::scalar::structural_fields::structural_parameter_field_path;
use checked_trees::CheckedBooleanExpression;
use checked_trees::CheckedStructuralParameterField;
use typed_trees::TypedTrees;
use typed_trees::expression::BinaryOperator;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::signature::StateParameter;

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
    let case = match program.expression_table.expression(binary.right) {
        ExpressionNode::Name(selected) => {
            if !validation::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                expression,
                binary,
            ) {
                return None;
            }
            structural_case_name(program, selected.symbol)?
        }
        // A payload-free case spelled as a value (`subject == T::Case`) arrives
        // as a construction, not a name: the authored test carries no
        // case-selection roster for the exact-meaning rejoin, but builtin
        // equality on a union's operands is already the same tag observation.
        ExpressionNode::StructLiteral(literal)
            if literal.case_symbol.is_some()
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .is_empty() =>
        {
            if !validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                return None;
            }
            let Some(case_symbol) = literal.case_symbol else {
                return None;
            };
            let owner = program.data_definitions().iter().find(|data| {
                data.symbol == literal.type_symbol
                    && program.data_members(data).iter().any(|member| {
                        matches!(
                            member,
                            typed_trees::data::DataMember::Variant(variant)
                                if variant.symbol == case_symbol
                                    && program.data_payload_fields(variant).is_empty()
                        )
                    })
            })?;
            let subject_type =
                validation::declared_place_type_raw(program, machine, Some(state), subject)?;
            if crate::values::scalar::structural_fields::structural_data(program, subject_type)?
                .symbol
                != owner.symbol
            {
                return None;
            }
            structural_case_name(program, case_symbol)?
        }
        _ => return None,
    };
    Some(CheckedBooleanExpression::StructuralCaseMembership {
        subject: CheckedStructuralParameterField {
            parameter_position,
            path,
        },
        case,
    })
}

/// The discriminant key a checked case-membership test stores: the declared
/// `#<decimal>` identity an authored discriminant value writes, or the case's
/// declared name when no identity exists.
pub(super) fn structural_case_name(
    program: &TypedTrees,
    selected: symbols::SymbolHandle,
) -> Option<String> {
    let case = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(case) if case.symbol == selected => Some(case),
            _ => None,
        })?;
    Some(case.path_identity())
}
