use omega_typed_trees::data::DataMember;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::measure::MeasureDefinition;

/// The well-founded ordering selected for a `decreases value -> Order` clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RankingOrder {
    /// Built-in descending naturals (also used for a simple `usize`-valued measure
    /// whose body forwards the parameter directly).
    NatDescending,
    /// Built-in `Slice::Length`.
    SliceLength,
    /// A declared `measure` whose body forwards the (already numeric) parameter.
    CustomNatDescending,
    /// A declared `measure` whose body projects a field of a struct parameter,
    /// e.g. `measure Card::PowerOrder(card: Card) -> usize { card.power }`. The
    /// stored identifier is the projected field name.
    CustomStructView(omega_typed_trees::name::Identifier),
    /// A declared lexicographic `measure`; the stored field names are the ordered
    /// projection components compared left-to-right.
    Lexicographic(Vec<omega_typed_trees::name::Identifier>),
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

        let measure = find_declared_measure(program, order)?;

        if measure.lexicographic {
            let components = lexicographic_component_fields(program, measure)?;
            return Some(Self::Lexicographic(components));
        }

        // Simple measure: validate parameter / return shape, then classify the body.
        if !measure_return_is_usize(program, measure) {
            return None;
        }

        match measure_body_shape(program, measure)? {
            MeasureBodyShape::ParameterForward => {
                // The decreasing value is already the numeric quantity.
                if measure_parameter_is_usize(program, measure)
                    && expression_type_name(program, state, decreases).as_deref() == Some("usize")
                {
                    Some(Self::CustomNatDescending)
                } else {
                    None
                }
            }
            MeasureBodyShape::FieldProjection(field) => {
                // The decreasing value must have the measure's parameter type.
                let parameter_type = measure.parameter.as_ref().map(|parameter| {
                    program
                        .type_reference_table
                        .display_name(parameter.type_reference)
                })?;
                if expression_type_name(program, state, decreases).as_deref()
                    != Some(parameter_type.as_str())
                {
                    return None;
                }
                Some(Self::CustomStructView(field))
            }
        }
    }
}

enum MeasureBodyShape {
    /// `{ param }` — the body forwards the parameter directly.
    ParameterForward,
    /// `{ param.field }` — the body projects a single field.
    FieldProjection(omega_typed_trees::name::Identifier),
}

fn measure_body_shape(
    program: &omega_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> Option<MeasureBodyShape> {
    let body = program.expression_table.expression_handles(measure.body);
    if body.len() != 1 {
        return None;
    }
    match program.expression_table.expression(body[0]) {
        ExpressionNode::Name(_) => Some(MeasureBodyShape::ParameterForward),
        ExpressionNode::Member(member) => {
            Some(MeasureBodyShape::FieldProjection(member.member.clone()))
        }
        _ => None,
    }
}

fn lexicographic_component_fields(
    program: &omega_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> Option<Vec<omega_typed_trees::name::Identifier>> {
    let body = program.expression_table.expression_handles(measure.body);
    if body.is_empty() {
        return None;
    }
    let mut fields = Vec::with_capacity(body.len());
    for component in body {
        match program.expression_table.expression(*component) {
            ExpressionNode::Name(path) => {
                let member = program
                    .expression_table
                    .name_path_members(path.members)
                    .last()
                    .cloned()?;
                fields.push(member);
            }
            ExpressionNode::Member(member) => fields.push(member.member.clone()),
            _ => return None,
        }
    }
    Some(fields)
}

fn find_declared_measure<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    order: &[omega_typed_trees::name::Identifier],
) -> Option<&'program MeasureDefinition> {
    program
        .measures()
        .iter()
        .find(|measure| path_matches_path(program.measure_path_members(measure.name), order))
}

fn measure_return_is_usize(
    program: &omega_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> bool {
    program
        .type_reference_table
        .display_name(measure.return_type)
        == "usize"
}

fn measure_parameter_is_usize(
    program: &omega_typed_trees::TypedTrees,
    measure: &MeasureDefinition,
) -> bool {
    measure.parameter.as_ref().is_some_and(|parameter| {
        program
            .type_reference_table
            .display_name(parameter.type_reference)
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
