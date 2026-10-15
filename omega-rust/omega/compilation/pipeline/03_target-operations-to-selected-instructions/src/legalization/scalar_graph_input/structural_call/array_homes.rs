//! An owned array home lent through a borrowed byte window or element view.
//!
//! Terminal verification admits one loan of activation-local storage as a
//! fixed-array view: a non-linear, claim-free collection
//! result with an independently checked window, presented through the argument's own
//! shared or exclusive borrow. The call receives a descriptor over the array's
//! exact extent while the argument keeps naming the producing home; no copy or
//! descriptor allocation replaces the backing. Parameter windows keep their
//! entrance route in `exclusive`; this route rejoins the dominating producer.
use super::{
    AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess, StructuralArgument,
    TargetStructuralArgument,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::aggregate_results::{node_site, precedes};
use crate::legalization::scalar_graph_input::structural_case;
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

/// Rejoin a local array loan, or `Ok(None)` when the argument is not
/// one: an owned transfer, a non-local root, or a
/// destination that is no fixed-array view of the home. A local home that is
/// such a view but fails its custody or placement replay rejects.
pub(super) fn argument(
    semantic: &StructuralArgument,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    destination: &StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    plan: &AbstractOperationPlan,
    native: &target_operations::TargetOperationPlan,
) -> Result<Option<TargetStructuralArgument>, LegalizationError> {
    if !matches!(
        semantic.access,
        terminal_psi::reading_borrow_pair_pattern!()
    ) {
        return Ok(None);
    }
    let types = &plan.structural_types;
    // An array's home can be established by an operation or selected by a
    // block edge. Both keep the exact owned declaration and original storage.
    let producer_result = structural_case::source_result(caller, semantic.place).ok();
    let (presented, source) = if let Some((producer, result)) = producer_result {
        (
            StructuralParameterDeclaration {
                place: result.place,
                position: destination.position,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access: StructuralAccess::Owned,
                qualifications: result.qualifications.clone(),
                projected_qualifications: result.projected_qualifications.clone(),
            },
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
        )
    } else if let Some(parameter) = caller.structural_parameter(semantic.place) {
        if parameter.access != StructuralAccess::Owned {
            return Ok(None);
        }
        let target = native
            .function(caller.machine)
            .ok_or(LegalizationError::custody())?;
        let source = super::super::structural_parameters(target)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|source| source.place == parameter.place)
            })
            .ok_or(LegalizationError::custody())?;
        let shape =
            crate::structural_inputs::structural_reference_input::parameter_shape(parameter, types)
                .ok_or(LegalizationError::custody())?;
        if parameter.multiplicity == StructuralMultiplicity::Linear
            || source.structural_type != parameter.structural_type
            || source.access != parameter.access
            || source.multiplicity != parameter.multiplicity
            || source.projected_qualifications != parameter.projected_qualifications
            || source.shape != shape
            || !caller.structural_places.iter().any(|place| {
                place.id == parameter.place
                    && place.kind
                        == StructuralPlaceKind::Parameter {
                            position: parameter.position,
                        }
            })
        {
            return Err(LegalizationError::custody());
        }
        (parameter.clone(), source.placement.clone().into())
    } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
        block
            .structural_parameter(semantic.place)
            .map(|parameter| (block.id, parameter))
    }) {
        if parameter.access != StructuralAccess::Owned {
            return Ok(None);
        }
        if parameter.multiplicity == StructuralMultiplicity::Linear
            || !caller.structural_places.iter().any(|place| {
                place.id == parameter.place
                    && place.kind
                        == StructuralPlaceKind::BlockParameter {
                            block,
                            position: parameter.position,
                        }
            })
        {
            return Err(LegalizationError::custody());
        }
        (
            parameter.clone(),
            target_operations::TargetStructuralArgumentSource::BlockParameter {
                block,
                place: parameter.place,
            },
        )
    } else {
        return Ok(None);
    };
    // An existing fixed view keeps its pointer cell under attenuation. It is
    // forwarded by descriptor custody, never reinterpreted as inline bytes.
    if semantic.path.is_empty()
        && terminal_semantics::borrowed_referent_presentation(
            types.iter(),
            presented.structural_type,
            destination.structural_type,
        ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
    {
        return Ok(None);
    }
    let byte_window = crate::structural_inputs::structural_reference_input::fixed_byte_array_view(
        &presented,
        semantic,
        destination.structural_type,
        types,
    );
    let element_view =
        crate::structural_inputs::structural_reference_input::fixed_element_array_view(
            &presented,
            semantic,
            destination.structural_type,
            types,
        );
    // A `&[u8]` view over a `[u8; N]` home satisfies both lookups; the
    // windows coincide and the byte window wins.
    let (source_byte_offset, length, stride) = match (byte_window, element_view) {
        (Some((offset, length)), _) => (offset, length, 1),
        (None, Some((offset, length, stride))) => (offset, length, stride),
        (None, None) => return Ok(None),
    };
    if let Some((producer, result)) = producer_result {
        let producer_site = node_site(caller, |operation| {
            operation.structural_result() == Some((producer, result))
        })
        .ok_or(LegalizationError::custody())?;
        if result.multiplicity == StructuralMultiplicity::Linear
            || !result.claims.is_empty()
            || !precedes(caller, producer_site, call_operation, semantic)?
            || !caller.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
        {
            return Err(LegalizationError::custody());
        }
    }
    let shape =
        crate::structural_inputs::structural_reference_input::parameter_shape(destination, types)
            .ok_or(LegalizationError::custody())?;
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(LegalizationError::custody())?;
    if destination.access != semantic.access
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || placement.shape != shape
    {
        return Err(LegalizationError::custody());
    }
    Ok(Some(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: presented.structural_type,
        structural_type: destination.structural_type,
        shape,
        source_byte_offset,
        fixed_array_length: Some(length),
        element_stride: Some(stride),
        indices: Vec::new(),
        source,
        destination: placement.clone(),
    }))
}
