//! Project an invocation's retained value tree onto the child that supplied it.
//! Destination paths identify fields independently of authored evaluation order.
//! Each selected child keeps its own completion context and type: siblings may
//! have copied the same source at different times, or may carry live loans.

use checked_trees::{FlowControlFacts, FlowOperandFact};
use facts::PlaceSegment;
use typed_trees::{TypedTrees, types::TypeReferenceHandle};

/// Contextual construction uses the bound destination declaration. A source
/// declaration is retained only when that destination cannot be projected.
/// Neither path infers a type from a sibling's expression handle.
pub(super) fn operand_child_type(
    program: &TypedTrees,
    parent: TypeReferenceHandle,
    destination: &[PlaceSegment],
    declared: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    parent
        .is_valid()
        .then(|| super::project_type_reference_from_segments(program, parent, destination))
        .flatten()
        .or_else(|| declared.is_valid().then_some(declared))
}

pub(crate) struct CapturedValue<'capture> {
    pub operand: &'capture FlowOperandFact,
    pub type_reference: TypeReferenceHandle,
    pub projection: super::LiteralValueProjection,
}

/// A dynamic selector can produce several candidates. Consumers must prove
/// each independently; combining their contexts would invent a capture epoch.
pub(crate) fn captured_operand_values<'capture>(
    program: &TypedTrees,
    control: &'capture FlowControlFacts,
    operand: &'capture FlowOperandFact,
    type_reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<Vec<CapturedValue<'capture>>> {
    super::literal_value_projections(program, operand.expression, type_reference, path, false)?
        .into_iter()
        .map(|projection| {
            let (captured, selected_type) = captured_operand_at_path(
                program,
                control,
                operand,
                type_reference,
                &projection.destination,
            )?;
            // The resolved constructor path and the retained evaluation must
            // name the same authored occurrence, not just the same storage.
            (captured.expression == projection.expression).then_some(CapturedValue {
                operand: captured,
                type_reference: selected_type,
                projection,
            })
        })
        .collect()
}

pub(crate) fn captured_operand_at_path<'capture>(
    program: &TypedTrees,
    control: &'capture FlowControlFacts,
    mut operand: &'capture FlowOperandFact,
    mut type_reference: TypeReferenceHandle,
    mut path: &[PlaceSegment],
) -> Option<(&'capture FlowOperandFact, TypeReferenceHandle)> {
    while !path.is_empty() {
        let mut selected = None;
        for child in control.operand_children.span(operand.children)? {
            let destination = control.operand_child_paths.span(child.path)?;
            if destination.is_empty() {
                return None;
            }
            if destination.len() <= path.len()
                && destination.iter().zip(path).all(|(left, right)| {
                    super::canonical_place_segments_equal(program, *left, *right)
                })
            {
                if selected.is_some() || !control.operands.is_valid(child.value) {
                    return None;
                }
                selected = Some((child, destination.len()));
            }
        }
        let (child, length) = selected?;
        operand = control.operands.get(child.value);
        type_reference = operand_child_type(
            program,
            type_reference,
            control.operand_child_paths.span(child.path)?,
            child.type_reference,
        )?;
        path = &path[length..];
    }
    type_reference
        .is_valid()
        .then_some((operand, type_reference))
}
