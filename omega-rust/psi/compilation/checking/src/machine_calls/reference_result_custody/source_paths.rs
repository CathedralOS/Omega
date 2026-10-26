//! Typed origin paths distinguish a selected descriptor from its enclosing
//! collection. A symbolic trailing window retains a region; it never becomes
//! an exact pointer, descriptor equality or permission. Both custody replay
//! and completed operand capture use this same dimension-preserving projection.

use checked_trees::{CheckedReferenceOriginProjection, CheckedUnitStructuralPathSegment};
use semantic_vocabulary::ReferenceOriginExtent;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Runtime element selections describe one typed element; erased windows
/// describe a region of the collection. Neither enumerates addresses.
/// Exact formation and bounds remain in the invocation's captured loan.
pub fn reference_origin_path(
    program: &dyn crate::CheckingInput,
    mut selected: TypeReferenceHandle,
    mut segments: &[facts::PlaceSegment],
) -> Option<(
    Vec<CheckedReferenceOriginProjection>,
    TypeReferenceHandle,
    ReferenceOriginExtent,
)> {
    let mut output = Vec::new();
    // An erased Index still denotes one element. Erased range expressions
    // have their own OpaqueRange kind, so an invalid handle cannot collapse
    // a window into its element type.
    let is_element_selection = |expression: ExpressionHandle| {
        !expression.is_valid()
            || program.expression_table.expression_is_valid(expression)
                && program.expression_table.range_node(expression).is_none()
    };
    loop {
        let selection = segments.iter().position(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Index { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::FixedRange { .. }
                    | facts::PlaceSegment::OpaqueRange
            )
        });
        let prefix_length = selection.unwrap_or(segments.len());
        let (prefix, endpoint) = structural_path(program, selected, &segments[..prefix_length])?;
        output.extend(
            prefix
                .into_iter()
                .map(CheckedReferenceOriginProjection::Exact),
        );
        selected = endpoint;
        let Some(position) = selection else {
            return Some((output, selected, ReferenceOriginExtent::Projected));
        };
        selected = crate::peel_declared_constraints(program, selected);
        // A window that ends the path narrows a collection without selecting an
        // element. Its origin is the collection it narrows, presented as
        // that collection is: a view formal presents its descriptor, as
        // the unprojected region below does. The descriptor retains the
        // captured window and bound. A fixed array keeps its own storage
        // type here; its returned slice is a separate descriptor, never an
        // assertion that the array and slice types are interchangeable.
        let trailing_window = position + 1 == segments.len()
            && match segments[position] {
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => true,
                facts::PlaceSegment::Index { expression } => !is_element_selection(expression),
                _ => false,
            };
        if trailing_window {
            let collection = crate::reference_shell(program, selected).map_or(selected, |shell| {
                crate::peel_declared_constraints(program, shell.referee)
            });
            return matches!(
                program.type_reference_table.type_reference(collection),
                TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. }
            )
            .then_some((output, selected, ReferenceOriginExtent::WithinRegion));
        }
        if let Some(shell) = crate::reference_shell(program, selected) {
            output.push(CheckedReferenceOriginProjection::Exact(
                CheckedUnitStructuralPathSegment::Referent,
            ));
            selected = crate::peel_declared_constraints(program, shell.referee);
        }
        let (TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type }) =
            program.type_reference_table.type_reference(selected)
        else {
            return None;
        };
        selected = *element_type;
        let (projection, consumed) = match segments[position] {
            facts::PlaceSegment::FixedIndex { index } => (
                CheckedReferenceOriginProjection::Exact(
                    CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(index).ok()?),
                ),
                1,
            ),
            facts::PlaceSegment::Index { expression } if is_element_selection(expression) => {
                (CheckedReferenceOriginProjection::Element, 1)
            }
            facts::PlaceSegment::FixedRange { start, end } => {
                // A view window and its element selector share one array
                // dimension. This is provenance, not a new address: the
                // descriptor retains its captured window, selector and
                // bound. Never consult the selector's later value here.
                let projection = match segments.get(position.checked_add(1)?)? {
                    facts::PlaceSegment::Index { expression }
                        if start < end && is_element_selection(*expression) =>
                    {
                        CheckedReferenceOriginProjection::Element
                    }
                    facts::PlaceSegment::FixedIndex { index }
                        if *index < end.checked_sub(start)? =>
                    {
                        CheckedReferenceOriginProjection::Exact(
                            CheckedUnitStructuralPathSegment::FixedIndex(
                                u64::try_from(start.checked_add(*index)?).ok()?,
                            ),
                        )
                    }
                    _ => return None,
                };
                (projection, 2)
            }
            facts::PlaceSegment::OpaqueRange => {
                match segments.get(position.checked_add(1)?)? {
                    facts::PlaceSegment::Index { expression }
                        if is_element_selection(*expression) => {}
                    facts::PlaceSegment::FixedIndex { .. } => {}
                    _ => return None,
                }
                (CheckedReferenceOriginProjection::Element, 2)
            }
            _ => return None,
        };
        output.push(projection);
        segments = &segments[position + consumed..];
    }
}

pub(super) fn structural_path(
    program: &dyn crate::CheckingInput,
    mut selected: TypeReferenceHandle,
    segments: &[facts::PlaceSegment],
) -> Option<(Vec<CheckedUnitStructuralPathSegment>, TypeReferenceHandle)> {
    let mut path = Vec::new();
    let mut payload = None;
    for segment in segments {
        selected = crate::peel_declared_constraints(program, selected);
        while let Some(shell) = crate::reference_shell(program, selected) {
            selected = shell.referee;
            path.push(CheckedUnitStructuralPathSegment::Referent);
        }
        match segment {
            facts::PlaceSegment::Case { variant } => {
                let data = program.closed_data_definition(selected)?;
                let case = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Variant(case) if case.symbol == *variant => {
                            Some(case)
                        }
                        _ => None,
                    })?;
                path.push(CheckedUnitStructuralPathSegment::Case(
                    checked_trees::CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    ),
                ));
                payload = Some(case);
            }
            facts::PlaceSegment::Field { symbol } => {
                let field = if let Some(case) = payload.take() {
                    program
                        .data_payload_fields(case)
                        .iter()
                        .find(|field| field.symbol == *symbol)?
                } else {
                    let data = program.closed_data_definition(selected)?;
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if field.symbol == *symbol =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })?
                };
                path.push(CheckedUnitStructuralPathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
                selected = field.type_reference;
            }
            facts::PlaceSegment::FixedIndex { index } => {
                let TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(selected)
                else {
                    return None;
                };
                selected = *element_type;
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
            }
            _ => return None,
        }
    }
    Some((path, selected))
}
