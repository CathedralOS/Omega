//! Borrowed call arguments retain their incoming root pointer and projection.
//!
//! Machine parameters and state arrivals differ in where that pointer lives,
//! not in how the callee's referent is laid out. Resolve canonical arrival
//! custody once, then apply the same projection and call-placement checks.
//! Signature qualifications are semantic preconditions, not native carriers:
//! unit admission owns their discharge and the bounded-byte writable invariant
//! rule. Transport must not reject a qualified value or invent a postcondition.
use super::{
    AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess, StructuralArgument,
    TargetOperationPlan, TargetStructuralArgument, ValueShape,
};
use crate::LegalizationError;
use semantic_vocabulary::StructuralPlaceKind;
use target_operations::TargetStructuralArgumentSource;
use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

pub(super) fn argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination: &StructuralParameterDeclaration,
    call: &CallPlan,
    scalar_count: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let source = caller
        .arrival_parameter(semantic.place)
        .ok_or(LegalizationError::custody())?;
    let types = &plan.structural_types;
    let source_shape =
        crate::structural_inputs::structural_reference_input::parameter_shape(source, types)
            .ok_or(LegalizationError::custody())?;
    let source_placement = arrival_source(source, caller, native, source_shape)?;
    let byte_field = crate::structural_inputs::structural_reference_input::bounded_byte_view(
        source,
        semantic,
        destination.structural_type,
        types,
    );
    let byte_field_projection = byte_field.as_ref().map(|geometry| geometry.offset);
    let byte_owner = crate::structural_inputs::structural_reference_input::bounded_byte_field_owner(
        source,
        semantic,
        destination.structural_type,
        types,
    );
    // Static prefixes and runtime selectors share the joined-storage
    // geometry. Keep selectors tied to this call's dominating definitions;
    // the path still carries their independently accepted bound obligations.
    let geometry = crate::structural_inputs::structural_reference_input::runtime_projection(
        source.structural_type,
        &semantic.path,
        types,
    );
    let projection = geometry
        .as_ref()
        .map(|(selected, offset, _)| (*selected, *offset));
    let byte_view = crate::structural_inputs::structural_reference_input::fixed_byte_array_view(
        source,
        semantic,
        destination.structural_type,
        types,
    );
    let element_view =
        crate::structural_inputs::structural_reference_input::fixed_element_array_view(
            source,
            semantic,
            destination.structural_type,
            types,
        );
    // A bounded inline byte field has no projected carrier identity; its own
    // record walk supplies the field offset and the destination supplies the
    // borrowed-view type the descriptor presents. The same holds for a fixed
    // array loaned through an element view.
    let (structural_type, source_byte_offset) = match (
        projection,
        byte_field_projection,
        byte_view,
        element_view,
        byte_owner,
    ) {
        (None, None, None, None, Some((offset, _))) => (destination.structural_type, offset),
        // A `&[u8]` view over a `[u8; N]` array satisfies both the byte
        // and the u8 element view lookups; their windows coincide, so the
        // byte view wins like in `borrowed_argument`.
        (_, None, Some((offset, _)), _, None) | (_, None, None, Some((offset, _, _)), None) => {
            (destination.structural_type, offset)
        }
        (Some((projected, offset)), None, None, None, None) => (projected, offset),
        (None, Some(offset), None, None, None) => (destination.structural_type, offset),
        _ => return Err(LegalizationError::custody()),
    };
    let descriptor = byte_view.is_some() || byte_field.is_some() || element_view.is_some();
    let shape =
        crate::structural_inputs::structural_reference_input::parameter_shape(destination, types)
            .ok_or(LegalizationError::custody())?;
    if semantic.place != source.place
        || destination.access != semantic.access
        || !matches!(
            (source.access, semantic.access),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ) | (
                StructuralAccess::WriteOnlyBorrow,
                StructuralAccess::WriteOnlyBorrow
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            )
        )
        || source.multiplicity != StructuralMultiplicity::Unrestricted
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || (structural_type != destination.structural_type && !descriptor)
        || call
            .parameters
            .get(scalar_count)
            .is_none_or(|placement| placement.shape != shape)
    {
        return Err(LegalizationError::custody());
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: source.structural_type,
        structural_type: if descriptor {
            destination.structural_type
        } else {
            structural_type
        },
        shape,
        source_byte_offset,
        fixed_array_length: byte_view
            .map(|(_, length)| length)
            .or_else(|| element_view.map(|(_, length, _)| length)),
        element_stride: byte_view
            .map(|_| 1)
            .or_else(|| element_view.map(|(_, _, stride)| stride)),
        indices: super::super::target::control_flow::sources::call_indices(
            native,
            caller,
            target_operations::CallSiteOwner::Operation(call_operation),
            byte_field.as_ref().map_or_else(
                || {
                    geometry
                        .as_ref()
                        .map_or(&[][..], |(_, _, elements)| elements.as_slice())
                },
                |geometry| geometry.elements.as_slice(),
            ),
        )?,
        source: source_placement,
        destination: call.parameters[scalar_count].clone(),
    })
}

/// Rejoin the pointer's physical origin without imposing a second type roster
/// on state arrivals. Canonical arrival lookup checked role and position;
/// unit ownership checked availability and dominance before legalization.
fn arrival_source(
    source: &StructuralParameterDeclaration,
    caller: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    source_shape: ValueShape,
) -> Result<TargetStructuralArgumentSource, LegalizationError> {
    let place = caller
        .structural_place(source.place)
        .ok_or(LegalizationError::custody())?;
    match place.kind {
        StructuralPlaceKind::BlockParameter { block, .. } => {
            Ok(TargetStructuralArgumentSource::BlockParameter {
                block,
                place: source.place,
            })
        }
        StructuralPlaceKind::Parameter { .. } => {
            let parameter = native
                .function(caller.machine)
                .and_then(super::super::structural_parameters)
                .and_then(|parameters| parameters.iter().find(|row| row.place == source.place))
                .ok_or(LegalizationError::custody())?;
            if parameter.structural_type != source.structural_type
                || parameter.access != source.access
                || parameter.multiplicity != source.multiplicity
                || parameter.projected_qualifications != source.projected_qualifications
                || parameter.shape != source_shape
            {
                return Err(LegalizationError::custody());
            }
            Ok(parameter.placement.clone().into())
        }
        _ => Err(LegalizationError::custody()),
    }
}
