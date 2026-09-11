//! Stored ordinary results remain distinct roots after whole-input transfer.

use super::super::shared::*;
use super::super::structural_layout::{
    expected_maximal_residual_subtrees, resolve_structural_projection_path,
    root_array_projection_metadata, structural_shape,
};
use target_operations::{TargetStructuralHomeLayout, TargetStructuralHomeRequirement};
use terminal_psi::TerminalAffineCleanupAction;

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

pub(super) fn validate_cleanup(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &StructuralTypeLookup<'_>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    cleanup: &[TerminalAffineCleanupAction],
) -> Option<()> {
    if !super::continuation::has_shape(function)
        && (function.structural_parameters.len() != 1
            || parameters.len() != 1
            || function.structural_parameters[0].position != 0)
    {
        return None;
    }
    let [producer, consumers @ ..] = operations else {
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
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.place == input.place)?;
    let authored_parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == input.place)?;
    let (home, placement) = source(operations, result.place)?;
    if consumers.is_empty()
        || (!function.parameters.is_empty() && !super::continuation::has_shape(function))
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || authored_parameter.place != parameter.place
        || authored_parameter.structural_type != parameter.structural_type
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
        || input.source != parameter.placement.clone().into()
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
    validate_consumers(
        result.place,
        result.structural_type,
        placement,
        home.layout.shape(),
        consumers,
        structural_types,
        functions,
        cleanup,
    )
}

pub(super) fn validate_parameter_cleanup(
    function: &AbstractFunction,
    parameters: &[TargetStructuralParameter],
    operations: &[TargetUnitOperation],
    structural_types: &StructuralTypeLookup<'_>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    cleanup: &[TerminalAffineCleanupAction],
) -> Option<()> {
    let TargetUnitOperation::Call { arguments, .. } = operations.first()? else {
        return None;
    };
    let [argument] = arguments.as_slice() else {
        return None;
    };
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)?;
    let authored = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)?;
    if !function.parameters.is_empty()
        || !function.entry_claims.is_empty()
        || !function.published_service_ceiling.is_empty()
        || authored.is_self
        || authored.access != StructuralAccess::Owned
        || authored.multiplicity != StructuralMultiplicity::Affine
        || !authored.qualifications.is_empty()
        || !authored.projected_qualifications.is_empty()
        || authored.structural_type != parameter.structural_type
    {
        return None;
    }
    validate_consumers(
        parameter.place,
        parameter.structural_type,
        &parameter.placement,
        parameter.shape,
        operations,
        structural_types,
        functions,
        cleanup,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_consumers(
    root: PlaceId,
    root_type: StructuralTypeId,
    placement: &ValuePlacement,
    supplied_shape: ValueShape,
    consumers: &[TargetUnitOperation],
    structural_types: &StructuralTypeLookup<'_>,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    cleanup: &[TerminalAffineCleanupAction],
) -> Option<()> {
    let mut shapes = BTreeMap::new();
    let mut active = BTreeSet::new();
    let root_shape =
        structural_shape(root_type, structural_types, &mut shapes, &mut active).ok()?;
    if supplied_shape != root_shape || consumers.is_empty() {
        return None;
    }
    let metadata =
        root_array_projection_metadata(root_type, structural_types, &mut shapes, &mut active)
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
            root_type,
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
            || argument.place != root
            || argument.path.is_empty()
            || argument.access != StructuralAccess::Owned
            || argument.root_structural_type != root_type
            || argument.structural_type != projected_type
            || argument.shape != shape
            || argument.source != placement.clone().into()
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
    let expected =
        expected_maximal_residual_subtrees(root_type, &moved, structural_types, cleanup.len())?;
    (expected.len() == cleanup.len() && cleanup.iter().zip(expected).all(|(action, (path, structural_type))| {
        matches!(action, TerminalAffineCleanupAction::DiscardResidual(discard)
            if discard.place == root && discard.path == path && discard.structural_type == structural_type)
    })).then_some(())
}
