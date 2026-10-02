//! A local array lent through a borrowed byte window or element view.
//!
//! Terminal verification admits one loan of activation-local storage as a
//! fixed-array view: a non-linear, unqualified, claim-free collection
//! result with an independently checked window, presented through the argument's own
//! shared or exclusive borrow. The call receives a descriptor over the array's
//! exact extent while the argument keeps naming the producing home; no copy or
//! descriptor allocation replaces the backing. Parameter windows keep their
//! entrance route in `exclusive`; this route rejoins the dominating producer.
use super::{
    AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess, StructuralArgument,
    TargetStructuralArgument, ValueShape,
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
) -> Result<Option<TargetStructuralArgument>, LegalizationError> {
    if !matches!(
        semantic.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) {
        return Ok(None);
    }
    let Ok((producer, result)) = structural_case::source_result(caller, semantic.place) else {
        return Ok(None);
    };
    let types = &plan.structural_types;
    // The home retains its actual owned custody. Borrowed presentation and
    // exact field geometry are decided by the common collection-loan contract.
    let presented = StructuralParameterDeclaration {
        place: result.place,
        position: destination.position,
        is_self: false,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access: StructuralAccess::Owned,
        qualifications: result.qualifications.clone(),
        projected_qualifications: result.projected_qualifications.clone(),
    };
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
    let producer_site = node_site(caller, |operation| {
        operation.structural_result() == Some((producer, result))
    })
    .ok_or(LegalizationError::custody())?;
    let carrier = crate::structural_inputs::structural_reference_input::shape(
        destination.structural_type,
        types,
    )
    .ok_or(LegalizationError::custody())?;
    let shape = ValueShape::borrowed_reference(carrier.byte_size, carrier.alignment);
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(LegalizationError::custody())?;
    if destination.access != semantic.access
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || result.multiplicity == StructuralMultiplicity::Linear
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
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
        || placement.shape != shape
    {
        return Err(LegalizationError::custody());
    }
    Ok(Some(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: result.structural_type,
        structural_type: destination.structural_type,
        shape,
        source_byte_offset,
        fixed_array_length: Some(length),
        element_stride: Some(stride),
        indices: Vec::new(),
        source: target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        },
        destination: placement.clone(),
    }))
}
