//! Exact projected exclusive references retain their incoming root pointer.
use super::{
    AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess, StructuralArgument,
    TargetOperationPlan, TargetStructuralArgument, ValueShape,
};
use crate::LegalizationError;
use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

pub(super) fn argument(
    semantic: &StructuralArgument,
    caller: &PsiOptimizationFunction,
    destination: &StructuralParameterDeclaration,
    call: &CallPlan,
    scalar_count: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    // An exclusive root may arrive through control flow rather than the machine
    // entrance. A non-entry block structural parameter carries the same custody
    // as an incoming parameter, and the caller's own parameter roster holds no
    // row for such a place, so reconstruct it from the block declaration and the
    // verifier-owned place role instead. Entry-block rows keep the entrance
    // route below, which owns the published parameter placement.
    if semantic.path.is_empty()
        && let Some((block, declaration)) = caller
            .blocks
            .iter()
            .filter(|block| block.id != caller.entry)
            .find_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == semantic.place)
                    .map(|parameter| (block, parameter))
            })
    {
        return block_parameter_argument(
            semantic,
            caller,
            block,
            declaration,
            destination,
            call,
            scalar_count,
            plan,
        );
    }
    let source = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == semantic.place)
        .ok_or(invalid.clone())?;
    let target_caller = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or(invalid.clone())?;
    let parameter = super::super::structural_parameters(target_caller)
        .ok_or(invalid.clone())?
        .iter()
        .find(|parameter| parameter.place == semantic.place)
        .ok_or(invalid.clone())?;
    let types = &plan.structural_types;
    let root_shape =
        crate::structural_inputs::structural_reference_input::shape(source.structural_type, types)
            .ok_or(invalid.clone())?;
    let byte_field = crate::structural_inputs::structural_reference_input::bounded_byte_field_view(
        source,
        semantic,
        destination.structural_type,
        types,
    );
    let projection = crate::structural_inputs::structural_reference_input::project(
        source.structural_type,
        &semantic.path,
        types,
    );
    let byte_view = crate::structural_inputs::structural_reference_input::fixed_byte_array_view(
        source,
        semantic,
        destination.structural_type,
        types,
    );
    // A bounded inline byte field has no projected carrier identity; its own
    // record walk supplies the field offset and the destination supplies the
    // borrowed-view type the descriptor presents.
    let (structural_type, source_byte_offset) = match (projection, byte_field, byte_view) {
        (_, None, Some((offset, _))) => (destination.structural_type, offset),
        (Some((projected, offset)), None, None) => (projected, offset),
        (None, Some((offset, _)), None) => (destination.structural_type, offset),
        _ => return Err(invalid),
    };
    let referent =
        crate::structural_inputs::structural_reference_input::shape(structural_type, types)
            .ok_or(invalid.clone())?;
    let descriptor = byte_view.is_some() || byte_field.is_some();
    let shape = if descriptor {
        ValueShape::borrowed_reference(16, 8)
    } else {
        ValueShape::borrowed_reference(referent.byte_size, referent.alignment)
    };
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
        || !source.qualifications.is_empty()
        || !source.projected_qualifications.is_empty()
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || (structural_type != destination.structural_type && !descriptor)
        || parameter.place != source.place
        || parameter.structural_type != source.structural_type
        || parameter.access != source.access
        || parameter.multiplicity != source.multiplicity
        || !parameter.projected_qualifications.is_empty()
        || parameter.shape
            != ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment)
        || call
            .parameters
            .get(scalar_count)
            .is_none_or(|placement| placement.shape != shape)
    {
        return Err(invalid);
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
        fixed_array_length: byte_view.map(|(_, length)| length),
        element_stride: byte_view.map(|_| 1),
        source: parameter.placement.clone().into(),
        destination: call.parameters[scalar_count].clone(),
    })
}

/// One exclusive borrow whose root is established by control flow: a non-entry
/// block structural parameter. The verifier's own place roster supplies the
/// root's role and the block declaration its custody, so no entrance placement
/// is consulted; a borrowed view presents the descriptor the entrance route
/// publishes. Only a whole root reaches here — a projected path has no
/// block-local carrier identity to reconstruct.
#[allow(clippy::too_many_arguments)]
fn block_parameter_argument(
    semantic: &StructuralArgument,
    caller: &PsiOptimizationFunction,
    block: &optimization_unit::OptimizationBlock,
    declaration: &StructuralParameterDeclaration,
    destination: &StructuralParameterDeclaration,
    call: &CallPlan,
    scalar_count: usize,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    use semantic_vocabulary::StructuralPlaceKind;
    use target_operations::TargetStructuralArgumentSource;
    let invalid = LegalizationError::SourceCustodyMismatch;
    let shape = ValueShape::borrowed_reference(16, 8);
    if declaration.access != StructuralAccess::MutableBorrow
        || !matches!(
            semantic.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        )
        || destination.access != semantic.access
        || declaration.is_self
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || declaration.structural_type != destination.structural_type
        || !caller.structural_places.iter().any(|place| {
            place.id == declaration.place
                && place.kind
                    == StructuralPlaceKind::BlockParameter {
                        block: block.id,
                        position: declaration.position,
                    }
        })
        || !plan.structural_types.iter().any(|entry| {
            entry.id == declaration.structural_type
                && entry.shape
                    == terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    )
        })
        || call
            .parameters
            .get(scalar_count)
            .is_none_or(|placement| placement.shape != shape)
    {
        return Err(invalid);
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: Vec::new(),
        root_structural_type: declaration.structural_type,
        structural_type: declaration.structural_type,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: TargetStructuralArgumentSource::BlockParameter {
            block: block.id,
            place: declaration.place,
        },
        destination: call.parameters[scalar_count].clone(),
    })
}
