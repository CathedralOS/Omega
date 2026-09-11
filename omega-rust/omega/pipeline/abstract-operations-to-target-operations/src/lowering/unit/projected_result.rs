//! Stored ordinary results remain distinct roots after whole-input transfer.

use super::super::shared::*;
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};

/// A direct register fragment contains at most eight logical bytes. Native
/// packing must not round up the fragment's memory extent.
pub(super) fn has_store_fragments(placement: &ValuePlacement) -> bool {
    placement.shape.class == ValueClass::Integer
        && placement.shape.byte_size != 0
        && placement
            .locations
            .iter()
            .try_fold(0_u16, |cursor, location| {
                let ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                } = location
                else {
                    return None;
                };
                if *value_byte_offset != cursor || !matches!(byte_size, 1..=8) {
                    return None;
                }
                cursor.checked_add(*byte_size)
            })
            == Some(placement.shape.byte_size)
}

pub(super) fn source(
    operations: &[TargetUnitOperation],
    place: PlaceId,
) -> Option<(&TargetStructuralHomeRequirement, &ValuePlacement)> {
    let mut sources = operations.iter().filter_map(|operation| {
        let TargetUnitOperation::StructuralResultCall {
            psi_operation,
            result,
            result_home: Some(home),
            call_plan,
            ..
        } = operation
        else {
            return None;
        };
        let TargetStructuralHomeLayout::Aggregate(shape) = home.layout else {
            return None;
        };
        let placement = call_plan.result.as_ref()?;
        (result.place == place
            && home.operation_result() == Some((*psi_operation, result))
            && shape == placement.shape
            && has_store_fragments(placement)
            && result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty())
        .then_some((home, placement))
    });
    let result = sources.next()?;
    sources.next().is_none().then_some(result)
}
