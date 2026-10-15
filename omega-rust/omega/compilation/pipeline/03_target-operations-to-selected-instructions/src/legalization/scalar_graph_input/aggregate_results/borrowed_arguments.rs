//! Rejoin aggregate references to exact incoming, operation, or block-entry storage.
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::target;

use super::{
    AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction,
    StructuralMultiplicity, StructuralPlaceKind, TargetOperationPlan,
};
use crate::legalization::scalar_graph_input::aggregate_results::home_layout;
use crate::legalization::scalar_graph_input::structural_case;
use crate::legalization::scalar_graph_input::structural_parameters;
pub(in crate::legalization::scalar_graph_input) fn reconstruct(
    argument: &terminal_psi::StructuralArgument,
    position: usize,
    call_operation: semantic_vocabulary::OperationId,
    caller: &PsiOptimizationFunction,
    callee: abstract_operations::AbstractCallSignature<'_>,
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
    callee: abstract_operations::AbstractCallSignature<'_>,
    call: &CallPlan,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let declaration = crate::legalization::scalar_graph_input::call_signature::parameter(
        callee,
        position,
        argument.place,
    )?;
    let destination = &declaration;
    // The argument carries the access the call site actually grants, which must
    // equal the callee's declared access; the *source* may hold wider authority
    // (an owned or mutable root can lend a write-only or shared view, but a
    // write-only or shared root can never be widened back).
    // Unit admission discharges qualifications before native transport; they
    // do not change the referent layout or require another argument carrier.
    if argument.access != destination.access
        || !matches!(
            argument.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        )
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
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
            callee.scalar_parameter_count() + position,
            native,
            plan,
        );
    }
    let (root, source) = if let Some(parameter) = caller.structural_parameter(argument.place) {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow
        ) && parameter.access != argument.access
        {
            return Err(LegalizationError::custody());
        }
        let retained = native
            .function(caller.machine)
            .and_then(structural_parameters)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|value| value.place == argument.place)
            })
            .ok_or(LegalizationError::custody())?;
        (parameter.structural_type, retained.placement.clone().into())
    } else {
        // Source ownership already identifies the exact establishing operation.
        // Locate that identity independently of whether an ordinary call, a
        // provider boundary, or a local construction produced the storage.
        let site = |operation| {
            super::node_site(caller, |candidate| {
                candidate.psi_operation() == Some(operation)
            })
        };
        let after = if let Some(operation) = owner.operation() {
            site(operation)
        } else {
            caller.blocks.iter().find_map(|block| {
                block
                    .nodes
                    .iter()
                    .position(|node| {
                        node.operation
                            .cleanup_rosters()
                            .any(|(edge, _)| Some(edge) == owner.edge())
                    })
                    .map(|position| (block.id, position))
            })
        }
        .ok_or(LegalizationError::custody())?;
        if let Some((block, declaration)) =
            super::borrowed_block_arrival(caller, argument.place, plan)
        {
            // A view arriving at a state block lends one element through its
            // descriptor, as an incoming view parameter does; selection
            // resolves the element through the view's backing pointer.
            if !target::control_flow::sources::dominates(caller, block, after.0)
                || !(declaration.access == argument.access
                    || declaration.access == terminal_psi::StructuralAccess::MutableBorrow)
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
        } else if let Some((producer, view_type)) = element_view_producer(caller, argument.place) {
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
                    if !target::control_flow::sources::dominates(caller, block, after.0)
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
    // A runtime selector loans one element: the byte offset retains the
    // static prefix and each RuntimeIndex segment's selector resolves to its
    // dominating scalar source — the same transport the leaf-copy sibling
    // already carries. A static path has no selectors.
    let (selected, offset, elements) =
        crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
            root,
            &argument.path,
            argument.access,
            Some(destination.structural_type),
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
    let indices = if elements.is_empty() {
        Vec::new()
    } else {
        target::control_flow::sources::call_indices(native, caller, owner, &elements)?
    };
    let shape = crate::structural_inputs::structural_reference_input::internal_borrow_shape(
        selected,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    let placement = call
        .parameters
        .get(callee.scalar_parameter_count() + position)
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

/// A borrowed element or byte view that arrives at one state block.
pub(super) fn view_arrival<'caller>(
    caller: &'caller PsiOptimizationFunction,
    place: semantic_vocabulary::PlaceId,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<(
    semantic_vocabulary::BlockId,
    &'caller terminal_psi::StructuralParameterDeclaration,
)> {
    let mut arrivals = caller.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, declaration) = arrivals.next()?;
    let shape = terminal_psi::find_structural_type(structural_types, declaration.structural_type)
        .map(|row| &row.shape)?;
    let view = matches!(shape, terminal_psi::StructuralTypeShape::ElementView { .. })
        || shape.is_borrowed_byte_view();
    (arrivals.next().is_none()
        && block != caller.entry
        && caller.structural_places.iter().any(|place| {
            place.id == declaration.place
                && place.kind
                    == StructuralPlaceKind::BlockParameter {
                        block,
                        position: declaration.position,
                    }
        })
        && view
        && matches!(
            declaration.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        && declaration.multiplicity == StructuralMultiplicity::Unrestricted
        && (declaration.qualifications.is_empty()
            || declaration.access == terminal_psi::StructuralAccess::SharedBorrow
                && shape.is_borrowed_byte_view())
        && declaration.projected_qualifications.is_empty())
    .then_some((block, declaration))
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
    caller.block_nodes().find_map(|node| match &node.operation {
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
