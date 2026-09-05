//! Project a callee access route through constructors, without replacing a
//! caller binding by its captured storage origin. Constructors have no caller
//! address; their actual field/element expressions supply the access routes.

use super::*;
use typed_trees::data::DataMember;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn literal_argument_access_places(
    program: &typed_trees::TypedTrees,
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

pub(crate) fn literal_value_projections(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
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
                let TypeReferenceNode::Named { symbol, .. } =
                    program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                if program.symbols.get(*symbol).kind != symbols::SymbolKind::Data
                    || literal.type_symbol != *symbol
                {
                    return None;
                }
                let definition = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.symbol == *symbol)?;
                if !definition.type_parameters.is_empty() {
                    return None;
                }
                let mut segments = segments;
                let fields: Vec<_> =
                    if let Some(case_name) = &literal.case_name {
                        let mut variants = program.data_members(definition).iter().filter_map(
                            |member| match member {
                                DataMember::Variant(variant) if variant.name == *case_name => {
                                    Some(variant)
                                }
                                _ => None,
                            },
                        );
                        let variant = variants.next()?;
                        if program.symbols.get(variant.symbol).kind != symbols::SymbolKind::Variant
                            || variants.next().is_some()
                            || literal
                                .case_symbol
                                .is_some_and(|symbol| symbol.is_valid() && symbol != variant.symbol)
                        {
                            return None;
                        }
                        if let Some((facts::PlaceSegment::Case { variant: selected }, remaining)) =
                            segments.split_first()
                        {
                            if *selected != variant.symbol {
                                return None;
                            }
                            segments = remaining;
                        } else if !segments.is_empty() {
                            return None;
                        }
                        program.data_payload_fields(variant).iter().collect()
                    } else {
                        program
                            .data_members(definition)
                            .iter()
                            .map(|member| match member {
                                DataMember::Field(field) => Some(field),
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>()?
                    };
                let actuals = program.expression_table.struct_fields(literal.fields);
                let selected = match segments.split_first() {
                    Some((facts::PlaceSegment::Field { symbol }, remaining)) => {
                        Some((*symbol, remaining))
                    }
                    None => None,
                    _ => return None,
                };
                if selected
                    .is_some_and(|(symbol, _)| !fields.iter().any(|field| field.symbol == symbol))
                {
                    return None;
                }
                for (index, actual) in actuals.iter().enumerate() {
                    if actuals[..index]
                        .iter()
                        .any(|prior| prior.name == actual.name)
                    {
                        return None;
                    }
                    let field = fields.iter().find(|field| field.name == actual.name)?;
                    if program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
                        || (actual.field_symbol.is_valid() && actual.field_symbol != field.symbol)
                    {
                        return None;
                    }
                }
                for field in fields {
                    if selected.is_some_and(|(symbol, _)| symbol != field.symbol) {
                        continue;
                    }
                    let Some(actual) = actuals.iter().find(|actual| actual.name == field.name)
                    else {
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
