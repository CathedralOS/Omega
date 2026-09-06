//! Stored ordinary results remain distinct roots after whole-input transfer.

use super::super::shared::*;
use super::super::structural_layout::{
    expected_maximal_residual_subtrees, resolve_structural_projection_path,
    root_array_projection_metadata, structural_shape,
};
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
use terminal_psi::TerminalAffineCleanupAction;

/// The direct fragment must fit one existing native load/store width. Wider
/// packing is independent work; no store may round up the logical byte extent.
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
                if *value_byte_offset != cursor || !matches!(byte_size, 1 | 2 | 4 | 8) {
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
            && home.result == *result
            && home.defining_operation == *psi_operation
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

pub(super) fn validate_cleanup(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    cleanup: &[TerminalAffineCleanupAction],
) -> Option<()> {
    let ([authored_parameter], [parameter], [producer, consumers @ ..]) = (
        function.structural_parameters.as_slice(),
        parameters,
        operations,
    ) else {
        return None;
    };
    let TargetUnitOperation::StructuralResultCall {
        result,
        arguments,
        scalar_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = producer
    else {
        return None;
    };
    let [input] = arguments.as_slice() else {
        return None;
    };
    let (home, placement) = source(operations, result.place)?;
    if consumers.is_empty()
        || !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || authored_parameter.place != parameter.place
        || authored_parameter.structural_type != parameter.structural_type
        || authored_parameter.position != 0
        || authored_parameter.is_self
        || authored_parameter.access != StructuralAccess::Owned
        || authored_parameter.multiplicity != StructuralMultiplicity::Affine
        || !authored_parameter.qualifications.is_empty()
        || !authored_parameter.projected_qualifications.is_empty()
        || input.place != parameter.place
        || input.place == result.place
        || input.access != StructuralAccess::Owned
        || !input.path.is_empty()
        || input.root_structural_type != parameter.structural_type
        || input.structural_type != result.structural_type
        || input.source != parameter.placement
        || input.shape != parameter.shape
        || input.source_byte_offset != 0
        || input.fixed_array_length.is_some()
        || input.element_stride.is_some()
        || !scalar_arguments.is_empty()
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
    {
        return None;
    }
    let mut shapes = BTreeMap::new();
    let mut active = BTreeSet::new();
    let root_shape = structural_shape(
        result.structural_type,
        structural_types,
        &mut shapes,
        &mut active,
    )
    .ok()?;
    if home.layout.shape() != root_shape {
        return None;
    }
    let metadata = root_array_projection_metadata(
        result.structural_type,
        structural_types,
        &mut shapes,
        &mut active,
    )
    .ok()?;
    let mut moved = Vec::new();
    for operation in consumers {
        let TargetUnitOperation::Call {
            callee,
            arguments,
            scalar_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            ..
        } = operation
        else {
            return None;
        };
        let callee = functions.get(callee).copied()?;
        let ([callee_parameter], [argument]) = (
            callee.structural_parameters.as_slice(),
            arguments.as_slice(),
        ) else {
            return None;
        };
        let (projected_type, shape, offset) = resolve_structural_projection_path(
            result.structural_type,
            &argument.path,
            structural_types,
            &mut shapes,
            &mut active,
        )
        .ok()?;
        if callee.result != AbstractFunctionResult::Unit
            || !callee.parameters.is_empty()
            || !callee.entry_claims.is_empty()
            || !callee.published_service_ceiling.is_empty()
            || callee_parameter.position != 0
            || callee_parameter.is_self
            || callee_parameter.structural_type != projected_type
            || callee_parameter.multiplicity != StructuralMultiplicity::Affine
            || callee_parameter.access != StructuralAccess::Owned
            || !callee_parameter.qualifications.is_empty()
            || !callee_parameter.projected_qualifications.is_empty()
            || !scalar_arguments.is_empty()
            || !claim_transfers.is_empty()
            || !requirement_obligations.is_empty()
            || !crash_continuations.is_empty()
            || argument.place != result.place
            || argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || argument.root_structural_type != result.structural_type
            || argument.structural_type != projected_type
            || argument.shape != shape
            || argument.source != *placement
            || argument.source_byte_offset != offset
            || (argument.fixed_array_length, argument.element_stride) != metadata
            || offset
                .checked_add(u32::from(shape.byte_size))
                .is_none_or(|end| end > u32::from(root_shape.byte_size))
        {
            return None;
        }
        moved.push((argument.path.clone(), projected_type));
    }
    let expected = expected_maximal_residual_subtrees(
        result.structural_type,
        &moved,
        structural_types,
        cleanup.len(),
    )?;
    (expected.len() == cleanup.len() && cleanup.iter().zip(expected).all(|(action, (path, structural_type))| {
        matches!(action, TerminalAffineCleanupAction::DiscardResidual(discard)
            if discard.place == result.place && discard.path == path && discard.structural_type == structural_type)
    })).then_some(())
}
