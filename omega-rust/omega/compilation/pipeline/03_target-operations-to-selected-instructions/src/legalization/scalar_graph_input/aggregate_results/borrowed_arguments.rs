//! Rejoin aggregate references to exact incoming, operation, or block-entry storage.
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::target;

use super::{
    AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction,
    StructuralMultiplicity, StructuralPlaceKind, TargetOperationPlan, ValueShape,
};
use crate::legalization::scalar_graph_input::aggregate_results::home_layout;
use crate::legalization::scalar_graph_input::structural_case;
use crate::legalization::scalar_graph_input::structural_parameters;
pub(super) fn reconstruct(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    reconstruct_at_owner(
        argument,
        position,
        target_operations::CallSiteOwner::Operation(call_operation),
        caller,
        callee,
        call,
        native,
        plan,
    )
}

pub(in crate::legalization::scalar_graph_input) fn reconstruct_at_owner(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    owner: target_operations::CallSiteOwner,
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let destination = callee
        .structural_parameters
        .get(position)
        .ok_or(LegalizationError::custody())?;
    // The argument carries the access the call site actually grants, which must
    // equal the callee's declared access; the *source* may hold wider authority
    // (an owned or mutable root can lend a write-only or shared view, but a
    // write-only or shared root can never be widened back).
    if argument.access != destination.access
        || !matches!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        )
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    // A state loan carries the referent address, even when the call returns
    // an owned aggregate. Reuse the ordinary call's independent join replay;
    // treating that loan as an owned arrival invents a copied backing home.
    if crate::legalization::scalar_graph_input::address_joins::parameter(
        caller,
        argument.place,
        plan,
    )
    .is_some()
    {
        return crate::legalization::scalar_graph_input::address_joins::call_argument(
            argument,
            destination,
            owner.operation().ok_or(LegalizationError::custody())?,
            caller,
            call,
            callee.parameters.len() + position,
            native,
            plan,
        );
    }
    let (root, source) = if let Some(parameter) = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
    {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
        ) && parameter.access != argument.access
        {
            return Err(LegalizationError::custody());
        }
        let retained = native
            .functions
            .iter()
            .find(|function| function.machine == caller.machine)
            .and_then(structural_parameters)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|value| value.place == argument.place)
            })
            .ok_or(LegalizationError::custody())?;
        (parameter.structural_type, retained.placement.clone().into())
    } else {
        let site = |operation| {
            caller.blocks.iter().find_map(|block| {
                block
                    .nodes
                    .iter()
                    .position(|node| {
                        matches!(&node.operation,
                AbstractOperation::EstablishRecord { psi_operation, .. }
                | AbstractOperation::EstablishScalarArray { psi_operation, .. }
                | AbstractOperation::EstablishZeroedStructural { psi_operation, .. }
                | AbstractOperation::EstablishStructuralLocal { psi_operation, .. }
                | AbstractOperation::EstablishCase { psi_operation, .. }
                | AbstractOperation::CallStructural {
                    psi_operation,
                    ..
                } if *psi_operation == operation)
                    })
                    .map(|position| (block.id, position))
            })
        };
        let after = if let Some(operation) = owner.operation() {
            site(operation)
        } else {
            caller.blocks.iter().find_map(|block| block.nodes.iter().position(|node| {
                matches!(&node.operation, AbstractOperation::ReturnUnit { psi_edge, .. } | AbstractOperation::Return { psi_edge, .. }
                    if Some(*psi_edge) == owner.edge())
            }).map(|position| (block.id, position)))
        }.ok_or(LegalizationError::custody())?;
        if let Some((producer, view_type)) = element_view_producer(caller, argument.place) {
            // An element view is a descriptor root: the argument names its
            // establishing producer, never a storage home, and the projected
            // element offset rides on top. The descriptor's own liveness is
            // proven by the established-view input check downstream.
            (
                view_type,
                target_operations::TargetStructuralArgumentSource::EstablishedElementView {
                    psi_operation: producer,
                },
            )
        } else {
            match structural_case::source_owner(caller, argument.place)? {
                legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                    operation,
                    result,
                } => {
                    home_layout(&result, plan)?;
                    let before = site(operation).ok_or(LegalizationError::custody())?;
                    if (before.0 == after.0 && before.1 >= after.1)
                        || (before.0 != after.0
                            && !target::control_flow::sources::dominates(caller, before.0, after.0))
                    {
                        return Err(LegalizationError::custody());
                    }
                    (
                        result.structural_type,
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: operation,
                        },
                    )
                }
                legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                    block,
                    declaration,
                } => {
                    if argument.access != terminal_psi::StructuralAccess::SharedBorrow
                        || !target::control_flow::sources::dominates(caller, block, after.0)
                        || !caller.structural_places.iter().any(|place| {
                            place.id == declaration.place
                                && place.kind
                                    == StructuralPlaceKind::BlockParameter {
                                        block,
                                        position: declaration.position,
                                    }
                        })
                    {
                        return Err(LegalizationError::custody());
                    }
                    (
                        declaration.structural_type,
                        target_operations::TargetStructuralArgumentSource::BlockParameter {
                            block,
                            place: declaration.place,
                        },
                    )
                }
                // `source_owner` never resolves a function parameter.
                legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
                | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                    ..
                } => {
                    return Err(LegalizationError::custody());
                }
            }
        }
    };
    let (selected, offset, indices) = if argument.path.iter().all(|segment| {
        !matches!(
            segment,
            terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
        )
    }) {
        let (selected, offset) = crate::structural_inputs::structural_reference_input::project(
            root,
            &argument.path,
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
        (selected, offset, Vec::new())
    } else {
        // A runtime selector loans one element: the byte offset retains the
        // static prefix and each RuntimeIndex segment's selector resolves to
        // its dominating scalar source — the same transport the leaf-copy
        // sibling already carries.
        let (selected, offset, elements) =
            crate::structural_inputs::structural_reference_input::runtime_projection(
                root,
                &argument.path,
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?;
        if elements.is_empty() {
            return Err(LegalizationError::custody());
        }
        let indices = target::control_flow::sources::call_indices(
            native,
            caller,
            owner.operation().ok_or(LegalizationError::custody())?,
            &elements,
        )?;
        (selected, offset, indices)
    };
    let referent = crate::structural_inputs::structural_reference_input::shape(
        selected,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    let placement = call
        .parameters
        .get(callee.parameters.len() + position)
        .ok_or(LegalizationError::custody())?;
    if selected != destination.structural_type || placement.shape != shape {
        return Err(LegalizationError::custody());
    }
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root,
        structural_type: selected,
        shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices,
        source,
        destination: placement.clone(),
    })
}

/// The caller place's exact element-view producer: an establishment writes
/// `destination`, a subslice writes `result.place`. A mutable view's affine
/// result still answers here — the argument loans the projected element, not
/// the descriptor's own custody.
fn element_view_producer(
    caller: &PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::OperationId,
    semantic_vocabulary::StructuralTypeId,
)> {
    caller
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            AbstractOperation::EstablishElementView {
                psi_operation,
                result,
                destination,
                ..
            } if *destination == place
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty() =>
            {
                Some((*psi_operation, result.structural_type))
            }
            AbstractOperation::ElementViewSubslice {
                psi_operation,
                result,
                ..
            } if result.place == place
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty() =>
            {
                Some((*psi_operation, result.structural_type))
            }
            _ => None,
        })
}
