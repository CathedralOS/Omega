use omega_typed_trees::data::DataMember;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RankingOrder {
    NatDescending,
    SliceLength,
    CustomNatDescending,
}

impl RankingOrder {
    pub(super) fn from_path(
        program: &omega_typed_trees::TypedTrees,
        state: &omega_typed_trees::state::State,
        decreases: ExpressionHandle,
        order: &[omega_typed_trees::name::Identifier],
    ) -> Option<Self> {
        if order.is_empty() || path_matches(order, &["Nat", "Descending"]) {
            return Some(Self::NatDescending);
        }
        if path_matches(order, &["Slice", "Length"]) {
            return Some(Self::SliceLength);
        }
        if custom_nat_descending_order_is_declared(program, state, decreases, order) {
            return Some(Self::CustomNatDescending);
        }

        None
    }
}

fn custom_nat_descending_order_is_declared(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    order: &[omega_typed_trees::name::Identifier],
) -> bool {
    let Some(decrease_type_name) = expression_type_name(program, state, decreases) else {
        return false;
    };

    program
        .operators()
        .iter()
        .chain(
            program
                .domain_definitions()
                .iter()
                .flat_map(|domain| program.domain_operators(domain).iter()),
        )
        .any(|operator| {
            let parameters = program.operator_parameters(operator);
            path_matches_path(program.operator_path_members(operator.name), order)
                && parameters.len() == 1
                && program
                    .type_reference_table
                    .display_name(parameters[0].type_reference)
                    == decrease_type_name
                && program
                    .type_reference_table
                    .display_name(operator.return_type)
                    == "usize"
        })
}

fn expression_type_name(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<String> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => state_parameter_type_name(
            program,
            state,
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|member| member.as_str()),
        ),
        ExpressionNode::Member(member) if member.member.as_str() == "len" => {
            Some("usize".to_string())
        }
        ExpressionNode::Member(member) => program
            .data_definitions()
            .iter()
            .flat_map(|data| program.data_members(data).iter())
            .find_map(|data_member| match data_member {
                DataMember::Field(field)
                    if field.symbol == member.member_symbol
                        || field.name.as_str() == member.member.as_str() =>
                {
                    Some(
                        program
                            .type_reference_table
                            .display_name(field.type_reference),
                    )
                }
                _ => None,
            }),
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                omega_typed_trees::expression::BinaryOperator::Subtract
            ) =>
        {
            Some("usize".to_string())
        }
        _ => None,
    }
}

fn state_parameter_type_name(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    symbol: omega_core::symbols::SymbolHandle,
    fallback_name: Option<&str>,
) -> Option<String> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            !parameter.is_self
                && (parameter.symbol == symbol
                    || fallback_name.is_some_and(|name| parameter.name.as_str() == name))
        })
        .map(|parameter| {
            program
                .type_reference_table
                .display_name(parameter.type_reference)
        })
}

fn path_matches_path(
    actual: &[omega_typed_trees::name::Identifier],
    expected: &[omega_typed_trees::name::Identifier],
) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual == expected)
}

fn path_matches(order: &[omega_typed_trees::name::Identifier], expected: &[&str]) -> bool {
    order.len() == expected.len()
        && order
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
}
