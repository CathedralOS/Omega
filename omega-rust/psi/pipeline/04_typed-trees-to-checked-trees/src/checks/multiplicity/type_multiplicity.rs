//! The multiplicity a type carries, with generic substitutions applied, and
//! whether an expression establishes a linear obligation.

use crate::checks::multiplicity::linear_obligations::LinearPlace;
use crate::checks::multiplicity::linear_validation::linear_claim_frontier;
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn type_carries_linear_obligation(
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    !linear_claim_frontier(program, type_reference).is_empty()
}

pub(crate) fn expression_establishes_obligation(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    relative_path: &[facts::PlaceSegment],
    places: &[LinearPlace],
) -> bool {
    if let Some(source) = crate::flow::canonical_place_from_expression_in_state(
        program,
        state_symbol,
        statement_index,
        expression,
    ) && let facts::PlaceRoot::Symbol(symbol) = source.root
    {
        let mut source_path = source.segments;
        source_path.extend_from_slice(relative_path);
        if let Some(place) = places
            .iter()
            .find(|place| place.symbol == symbol && place.path == source_path)
        {
            return place.live;
        }
    }

    if let typed_trees::expression::ExpressionNode::ArrayLiteral(values) =
        program.expression_table.expression(expression)
        && let Some(facts::PlaceSegment::FixedIndex { index }) = relative_path.first()
    {
        return program
            .expression_table
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
            let Some(field) = program
                .expression_table
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
    program: &typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Multiplicity {
    program.type_multiplicity(type_reference)
}

pub(crate) fn find_data_definition<'program>(
    program: &'program typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|definition| {
        (symbol.is_valid() && definition.symbol == symbol) || definition.name.as_str() == name
    })
}

pub(crate) fn literal_variant<'program>(
    program: &'program typed_trees::TypedTrees,
    literal: &typed_trees::expression::TableStructLiteral,
) -> Option<&'program typed_trees::data::DataVariant> {
    let case_name = literal.case_name.as_ref()?;
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == literal.type_name)?;
    program.data_members(definition).iter().find_map(|member| {
        let typed_trees::data::DataMember::Variant(variant) = member else {
            return None;
        };
        (variant.name == *case_name).then_some(variant)
    })
}

pub(crate) fn data_field_name(
    program: &typed_trees::TypedTrees,
    field_symbol: SymbolHandle,
) -> Option<&str> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) => {
                    (field.symbol == field_symbol).then_some(field.name.as_str())
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.symbol == field_symbol).then_some(field.name.as_str())
                    }),
            })
    })
}
