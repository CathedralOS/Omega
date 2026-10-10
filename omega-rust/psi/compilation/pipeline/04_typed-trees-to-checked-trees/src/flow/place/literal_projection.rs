//! Project a callee access route through constructors, without replacing a
//! caller binding by its captured storage origin. Constructors have no caller
//! address; their actual field/element expressions supply the access routes.
//! Common fields use Field; selected payload fields use Case/Field. Both
//! belong to one authored constructor roster, and resolved symbols join each
//! operand to its declaration without letting a common field select a case.

use validation::CanonicalPlace;
use validation::canonical_place_from_expression_in_state;

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn literal_argument_access_places(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<Vec<CanonicalPlace>> {
    literal_projections(
        program,
        expression,
        reference,
        segments,
        ProjectionMode::Access,
    )?
    .into_iter()
    .map(|projection| {
        let mut place = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            projection.expression,
        )?;
        place.segments.extend_from_slice(&projection.remaining);
        Some(place)
    })
    .collect()
}

#[derive(Clone)]
pub(crate) struct LiteralValueProjection {
    pub expression: ExpressionHandle,
    pub remaining: Vec<facts::PlaceSegment>,
    pub destination: Vec<facts::PlaceSegment>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjectionMode {
    Access,
    SelectedValue,
    ConstructedValues,
}

/// Only an exact constructed alternative makes a payload contract inapplicable.
/// Unknown stored values and malformed projections remain obligations. Resolve
/// nested fields/array elements through the same projection owner as accesses;
/// never replay a local initializer to discover its former tag.
pub(crate) fn literal_value_path_is_inactive(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> bool {
    segments.iter().enumerate().any(|(position, segment)| {
        let facts::PlaceSegment::Case { variant } = segment else {
            return false;
        };
        let Some(projections) =
            literal_value_projections(program, expression, reference, &segments[..position], false)
        else {
            return false;
        };
        !projections.is_empty()
            && projections.iter().all(|projection| {
                if !projection.remaining.is_empty() {
                    return false;
                }
                let actual = match program.expression_table.expression(projection.expression) {
                    ExpressionNode::StructLiteral(literal) => {
                        literal.case_symbol.filter(|symbol| {
                            symbol.is_valid()
                                && program.symbols.get(*symbol).parent == literal.type_symbol
                        })
                    }
                    ExpressionNode::Name(name)
                        if program.symbols.get(name.symbol).kind
                            == symbols::SymbolKind::Variant =>
                    {
                        Some(name.symbol)
                    }
                    _ => None,
                };
                actual.is_some_and(|actual| {
                    actual != *variant
                        && variant.is_valid()
                        && program.symbols.get(actual).kind == symbols::SymbolKind::Variant
                        && program.symbols.get(*variant).kind == symbols::SymbolKind::Variant
                        && program.symbols.get(actual).parent
                            == program.symbols.get(*variant).parent
                })
            })
    })
}

pub(crate) fn literal_value_projections(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
    enumerate: bool,
) -> Option<Vec<LiteralValueProjection>> {
    literal_projections(
        program,
        expression,
        reference,
        segments,
        if enumerate {
            ProjectionMode::ConstructedValues
        } else {
            ProjectionMode::SelectedValue
        },
    )
}

fn literal_projections(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    reference: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
    mode: ProjectionMode,
) -> Option<Vec<LiteralValueProjection>> {
    let mut pending = vec![(expression, reference, segments, Vec::new())];
    let mut places = Vec::new();
    while let Some((expression, reference, segments, destination)) = pending.pop() {
        if !reference.is_valid() {
            return None;
        }
        if let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            pending.push((expression, *base_type, segments, destination));
            continue;
        }
        if mode == ProjectionMode::SelectedValue && segments.is_empty() {
            places.push(LiteralValueProjection {
                expression,
                remaining: Vec::new(),
                destination,
            });
            continue;
        }
        if program.primitive_type_reference(reference).is_some()
            || matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Unit
            )
        {
            // A primitive constructor value is copied into callee-owned
            // storage. Its evaluation is checked separately; writing that
            // copy does not access the caller expression.
            if !segments.is_empty() {
                return None;
            }
            if mode != ProjectionMode::Access {
                places.push(LiteralValueProjection {
                    expression,
                    remaining: Vec::new(),
                    destination,
                });
            }
            continue;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                let owner = program
                    .type_reference_table
                    .type_reference(reference)
                    .as_named_symbol()?;
                if program.symbols.get(owner).kind != symbols::SymbolKind::Data
                    || literal.type_symbol != owner
                {
                    return None;
                }
                let definition = program.data_definition_by_symbol(owner)?;
                if !definition.type_parameters.is_empty() {
                    return None;
                }
                let members = program.data_members(definition);
                let variant = match literal.case_symbol {
                    Some(symbol) => Some(members.iter().find_map(|member| match member {
                        DataMember::Variant(variant)
                            if variant.symbol == symbol
                                && symbol.is_valid()
                                && program.symbols.get(symbol).kind
                                    == symbols::SymbolKind::Variant
                                && program.symbols.get(symbol).parent == owner =>
                        {
                            Some(variant)
                        }
                        _ => None,
                    })?),
                    None if members
                        .iter()
                        .any(|member| matches!(member, DataMember::Variant(_))) =>
                    {
                        return None;
                    }
                    None => None,
                };
                let mut fields = members
                    .iter()
                    .filter_map(DataMember::as_field)
                    .collect::<Vec<_>>();
                if let Some(variant) = variant {
                    fields.extend(program.data_payload_fields(variant));
                }
                let (selected_variant, segments) = match segments.split_first() {
                    Some((facts::PlaceSegment::Case { variant: selected }, remaining)) => {
                        if variant.is_none_or(|variant| variant.symbol != *selected) {
                            return None;
                        }
                        (Some(*selected), remaining)
                    }
                    _ => (None, segments),
                };
                let actuals = program.expression_table.struct_fields(literal.fields);
                let selected = match segments.split_first() {
                    Some((facts::PlaceSegment::Field { symbol }, remaining)) => {
                        Some((*symbol, remaining))
                    }
                    None => None,
                    _ => return None,
                };
                if selected.is_some_and(|(symbol, _)| {
                    !fields.iter().any(|field| field.symbol == symbol)
                        || facts::payload_variant_for_field(program, symbol) != selected_variant
                }) {
                    return None;
                }
                for (index, actual) in actuals.iter().enumerate() {
                    if actuals[..index]
                        .iter()
                        .any(|prior| prior.field_symbol == actual.field_symbol)
                    {
                        return None;
                    }
                    let field = fields
                        .iter()
                        .find(|field| field.symbol == actual.field_symbol)?;
                    if program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
                        || !actual.field_symbol.is_valid()
                    {
                        return None;
                    }
                }
                for field in fields {
                    if selected_variant.is_some_and(|variant| {
                        facts::payload_variant_for_field(program, field.symbol) != Some(variant)
                    }) {
                        continue;
                    }
                    if selected.is_some_and(|(symbol, _)| symbol != field.symbol) {
                        continue;
                    }
                    let Some(actual) = actuals
                        .iter()
                        .find(|actual| actual.field_symbol == field.symbol)
                    else {
                        // Enumeration transports completed authored children;
                        // implicit zeros have no source expression to project.
                        // Their admissibility remains a construction obligation,
                        // and selecting an omitted subtree still needs evidence.
                        if mode == ProjectionMode::ConstructedValues && selected.is_none() {
                            continue;
                        }
                        if program
                            .primitive_type_reference(field.type_reference)
                            .is_some()
                            && selected.is_none_or(|(_, remaining)| remaining.is_empty())
                        {
                            continue;
                        }
                        return None;
                    };
                    let mut destination = destination.clone();
                    if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
                        destination.push(facts::PlaceSegment::Case { variant });
                    }
                    destination.push(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    });
                    pending.push((
                        actual.value,
                        field.type_reference,
                        selected.map_or(&[][..], |(_, remaining)| remaining),
                        destination,
                    ));
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                let elements = program.expression_table.expression_handles(*elements);
                if elements.len() != *length {
                    return None;
                }
                match segments.split_first() {
                    None => {
                        for (index, element) in elements.iter().enumerate() {
                            let mut destination = destination.clone();
                            destination.push(facts::PlaceSegment::FixedIndex { index });
                            pending.push((*element, *element_type, &[], destination));
                        }
                    }
                    Some((facts::PlaceSegment::FixedIndex { index }, remaining)) => {
                        let mut destination = destination;
                        destination.push(facts::PlaceSegment::FixedIndex { index: *index });
                        pending.push((
                            *elements.get(*index)?,
                            *element_type,
                            remaining,
                            destination,
                        ));
                    }
                    Some((facts::PlaceSegment::Index { .. }, remaining)) => {
                        for (index, element) in elements.iter().enumerate() {
                            let mut destination = destination.clone();
                            destination.push(facts::PlaceSegment::FixedIndex { index });
                            pending.push((*element, *element_type, remaining, destination));
                        }
                    }
                    Some((facts::PlaceSegment::FixedRange { start, end }, remaining))
                        if start <= end && *end <= elements.len() =>
                    {
                        for (offset, element) in elements[*start..*end].iter().enumerate() {
                            let mut destination = destination.clone();
                            destination.push(facts::PlaceSegment::FixedIndex {
                                index: start + offset,
                            });
                            pending.push((*element, *element_type, remaining, destination));
                        }
                    }
                    _ => return None,
                }
            }
            _ => {
                places.push(LiteralValueProjection {
                    expression,
                    remaining: segments.to_vec(),
                    destination,
                });
            }
        }
    }
    Some(places)
}
