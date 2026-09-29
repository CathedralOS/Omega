//! A whole local array lent through a borrowed byte or element view.
//!
//! Terminal verification admits one loan of activation-local storage as a
//! fixed-array view: an unrestricted, unqualified, claim-free plain array
//! result named with an empty path, presented through the argument's own
//! shared or exclusive borrow. The call receives a descriptor over the array's
//! exact extent while the argument keeps naming the producing home; no copy or
//! descriptor allocation replaces the backing. Parameter windows keep their
//! entrance route in `exclusive`; this route rejoins the dominating producer.
use super::{
    AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction, StructuralAccess,
    StructuralArgument, TargetStructuralArgument, ValueShape,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::aggregate_results::{node_site, precedes};
use crate::legalization::scalar_graph_input::structural_case;
use abstract_operations::{AbstractBoundaryResult, AbstractStructuralCallResult};
use semantic_vocabulary::StructuralPlaceKind;
use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

/// Rejoin a whole local array loan, or `Ok(None)` when the argument is not
/// one: a projected path, an owned transfer, a non-local root, or a
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
    if !semantic.path.is_empty()
        || !matches!(
            semantic.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
    {
        return Ok(None);
    }
    let Ok((producer, result)) = structural_case::source_result(caller, semantic.place) else {
        return Ok(None);
    };
    let types = &plan.structural_types;
    // The home presents its backing through this loan exactly as Terminal's
    // array-result loan does; its own owned access is unchanged.
    let presented = StructuralParameterDeclaration {
        place: result.place,
        position: destination.position,
        is_self: false,
        structural_type: result.structural_type,
        multiplicity: result.multiplicity,
        access: semantic.access,
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
    let (source_byte_offset, length, stride) = match (byte_window, element_view) {
        (Some((offset, length)), None) => (offset, length, 1),
        (None, Some((offset, length, stride))) => (offset, length, stride),
        (None, None) => return Ok(None),
        (Some(_), Some(_)) => return Err(LegalizationError::custody()),
    };
    let producer_site = node_site(caller, |operation| {
        matches!(operation,
            AbstractOperation::EstablishScalarArray { psi_operation, result: actual, .. }
            | AbstractOperation::EstablishZeroedStructural { psi_operation, result: actual }
            | AbstractOperation::StructuralLeafCopy { psi_operation, result: actual, .. }
            | AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result: actual, .. },
                ..
            }
            | AbstractOperation::BoundaryCall {
                psi_operation,
                result: AbstractBoundaryResult::Structural(actual),
                ..
            }
            if *psi_operation == producer && actual == result)
    })
    .ok_or(LegalizationError::custody())?;
    let shape = ValueShape::borrowed_reference(16, 8);
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(LegalizationError::custody())?;
    if destination.access != semantic.access
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || result.multiplicity != StructuralMultiplicity::Unrestricted
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
        path: Vec::new(),
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
