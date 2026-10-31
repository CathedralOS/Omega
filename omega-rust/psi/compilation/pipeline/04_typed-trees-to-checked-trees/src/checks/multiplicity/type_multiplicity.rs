//! The multiplicity a type carries, with generic substitutions applied, and
//! whether an expression establishes a linear obligation.

use crate::checks::multiplicity::linear_obligations::LinearPlace;
use crate::flow::expressions;
use crate::places::place_at;
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn type_carries_linear_obligation(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    validation::type_carries_linear_obligation(program, type_reference)
}

pub(crate) fn expression_establishes_obligation(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> bool {
    if let Some(source) = place_at(program, state_symbol, statement_index, expression) {
        let mut source_path = source.segments;
        source_path.extend_from_slice(relative_path);
        if let Some(place) = places.iter().find(|place| {
            place.root == source.root && place.path == source_path && place.ever_established
        }) {
            return place.live;
        }
    }

    if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        return expressions(program)
            .expression_handles(*values)
            .get(*index)
            .is_some_and(|value| {
                expression_establishes_obligation(
                    program,
                    state_symbol,
                    statement_index,
                    *value,
                    &relative_path[1..],
                    places,
                )
            });
    }

    if let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        program.expression_table.expression(expression)
    {
        let mut remaining = relative_path;
        if literal.case_name.is_some() {
            let Some(facts::PlaceSegment::Case { variant }) = remaining.first() else {
                return remaining.is_empty();
            };
            if literal_variant(program, literal).map(|candidate| candidate.symbol) != Some(*variant)
            {
                return false;
            }
            remaining = &remaining[1..];
        }
        if let Some(facts::PlaceSegment::Field { symbol }) = remaining.first() {
            let Some(field_name) = data_field_name(program, *symbol) else {
                return false;
            };
            let Some(field) = expressions(program)
                .struct_fields(literal.fields)
                .iter()
                .find(|field| field.name.as_str() == field_name)
            else {
                return false;
            };
            return expression_establishes_obligation(
                program,
                state_symbol,
                statement_index,
                field.value,
                &remaining[1..],
                places,
            );
        }
        return remaining.is_empty();
    }

    if matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Name(path)
            if program.data_definitions().iter().any(|definition| {
                program.data_members(definition).iter().any(|member| {
                    matches!(
                        member,
                        typed_trees::data::DataMember::Variant(variant)
                            if variant.symbol == path.symbol
                    )
                })
            })
    ) {
        // A payloadless case has no obligation for a conditional payload
        // frontier, but constructing a data type declared `[linear]` still
        // establishes that type's root obligation. The latter arrives with an
        // empty relative path; case/payload claims retain at least one segment.
        return relative_path.is_empty();
    }

    // Calls and boundary results have unknown active cases. Every case path is
    // retained as a possible obligation until a static case projection selects
    // one alternative.
    true
}

pub(crate) fn type_multiplicity(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    program.type_multiplicity(type_reference)
}

pub(crate) fn literal_variant<'program>(
    program: &'program dyn ::validation::CheckingInput,
    literal: &typed_trees::expression::TableStructLiteral,
) -> Option<&'program typed_trees::data::DataVariant> {
    let case_name = literal.case_name.as_ref()?;
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == literal.type_name)?;
    program.data_members(definition).iter().find_map(|member| {
        let variant = member.as_variant()?;
        (variant.name == *case_name).then_some(variant)
    })
}

pub(crate) fn data_field_name(
    program: &dyn ::validation::CheckingInput,
    field_symbol: SymbolHandle,
) -> Option<&str> {
    program
        .any_field_owner(field_symbol)
        .map(|(_, field)| field.name.as_str())
}
