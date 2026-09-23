//! Independent replay of shared address joins against the optimized unit.
//!
//! An address join (`abstract_operations::control_flow::address_joins`) is a
//! shared block parameter whose carrier is its referent's address. Target
//! lowering classified each edge source and call argument; this replay
//! reconstructs both from the optimized function alone, without reading the
//! target's classification:
//!
//! - every edge into the join lends a readable, non-linear, unqualified root
//!   whose owner dominates the edge (a local producer, an owned block arrival,
//!   a machine parameter or an earlier address join), through a static
//!   projection naming exactly the join's referent type;
//! - a call argument lends the whole joined referent, shared, from a join
//!   whose block dominates the call, at the callee's exact borrowed shape.
//!
//! Loan duration is not decided here: the unit's current-ownership replay pins
//! each lending root while a join can observe it.

use super::{AbstractOperation, AbstractOperationPlan, CallPlan, PsiOptimizationFunction};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::target::control_flow::sources::dominates;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

/// The join declaration and its block when `place` is an address join.
pub(in crate::legalization) fn parameter<'a>(
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
    plan: &AbstractOperationPlan,
) -> Option<(BlockId, &'a StructuralParameterDeclaration)> {
    let mut matching = function.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, parameter) = matching.next()?;
    (matching.next().is_none()
        && block != function.entry
        && abstract_operations::control_flow::address_joins::is_address_join_in(
            parameter,
            &plan.structural_types,
        )
        && function.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == (StructuralPlaceKind::BlockParameter {
                        block,
                        position: parameter.position,
                    })
        }))
    .then_some((block, parameter))
}

/// Rejoin a whole shared argument lent by an address join.
pub(in crate::legalization) fn call_argument(
    semantic: &terminal_psi::StructuralArgument,
    destination: &StructuralParameterDeclaration,
    call_operation: OperationId,
    caller: &PsiOptimizationFunction,
    call: &CallPlan,
    parameter_ordinal: usize,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (block, parameter) = parameter(caller, semantic.place, plan).ok_or(invalid.clone())?;
    let call_block = operation_block(caller, call_operation).ok_or(invalid.clone())?;
    let referent = crate::structural_inputs::structural_reference_input::shape(
        parameter.structural_type,
        &plan.structural_types,
    )
    .ok_or(invalid.clone())?;
    let shape =
        calling_conventions::ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(invalid.clone())?;
    if !semantic.path.is_empty()
        || semantic.access != StructuralAccess::SharedBorrow
        || destination.access != StructuralAccess::SharedBorrow
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || destination.structural_type != parameter.structural_type
        || !dominates(caller, block, call_block)
        || placement.shape != shape
    {
        return Err(invalid);
    }
    Ok(target_operations::TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: Vec::new(),
        root_structural_type: parameter.structural_type,
        structural_type: parameter.structural_type,
        shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source: target_operations::TargetStructuralArgumentSource::BlockParameter {
            block,
            place: parameter.place,
        },
        destination: placement.clone(),
    })
}

/// Replay every address-join binding on one edge leaving `source_block`.
/// Descriptor views and owned arrivals keep their existing replay.
pub(in crate::legalization) fn edge_bindings(
    function: &PsiOptimizationFunction,
    source_block: BlockId,
    target: BlockId,
    bindings: &[abstract_operations::AbstractStructuralBinding],
    plan: &AbstractOperationPlan,
) -> bool {
    let Some(destination) = function.blocks.iter().find(|block| block.id == target) else {
        return false;
    };
    bindings.len() == destination.structural_parameters.len()
        && bindings
            .iter()
            .zip(&destination.structural_parameters)
            .all(|(binding, declaration)| {
                if !abstract_operations::control_flow::address_joins::is_address_join_in(
                    declaration,
                    &plan.structural_types,
                ) {
                    return true;
                }
                binding.parameter == declaration.place
                    && binding.argument.access == StructuralAccess::SharedBorrow
                    && abstract_operations::control_flow::address_joins::is_static_projection(
                        &binding.argument.path,
                    )
                    && root_type(function, source_block, binding.argument.place, plan)
                        .and_then(|root| {
                            crate::structural_inputs::structural_reference_input::project(
                                root,
                                &binding.argument.path,
                                &plan.structural_types,
                            )
                        })
                        .is_some_and(|(referent, _)| referent == declaration.structural_type)
            })
}

/// The readable, non-linear, unqualified root type `place` names on an edge
/// leaving `block`, when its owner dominates that edge.
fn root_type(
    function: &PsiOptimizationFunction,
    block: BlockId,
    place: PlaceId,
    plan: &AbstractOperationPlan,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let plain = |multiplicity, qualifications: &[_], projected: &[_]| {
        multiplicity != StructuralMultiplicity::Linear
            && qualifications.is_empty()
            && projected.is_empty()
    };
    if let Some(source) = function
        .structural_parameters
        .iter()
        .find(|source| source.place == place)
    {
        return (!source.is_self
            && source.access != StructuralAccess::WriteOnlyBorrow
            && plain(
                source.multiplicity,
                &source.qualifications,
                &source.projected_qualifications,
            ))
        .then_some(source.structural_type);
    }
    if let Some((owner, joined)) = parameter(function, place, plan) {
        return (owner == block || dominates(function, owner, block))
            .then_some(joined.structural_type);
    }
    let (operation, result) =
        if let Some((operation, result, _)) = super::primitive_locals::producer(function, place) {
            (operation, result.clone())
        } else {
            match super::structural_case::source_owner(function, place).ok()? {
                legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                    operation,
                    result,
                } => (operation, result),
                legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                    block: owner,
                    declaration,
                } => {
                    return (owner == block || dominates(function, owner, block))
                        .then_some(declaration.structural_type);
                }
                legalized_operations::LegalizedStructuralCaseSource::Parameter { .. } => {
                    return None;
                }
            }
        };
    let owner = operation_block(function, operation)?;
    (result.claims.is_empty()
        && plain(
            result.multiplicity,
            &result.qualifications,
            &result.projected_qualifications,
        )
        && (owner == block || dominates(function, owner, block)))
    .then_some(result.structural_type)
}

fn operation_block(function: &PsiOptimizationFunction, operation: OperationId) -> Option<BlockId> {
    function.blocks.iter().find_map(|block| {
        block
            .nodes
            .iter()
            .any(|node| node_operation(&node.operation) == Some(operation))
            .then_some(block.id)
    })
}

fn node_operation(operation: &AbstractOperation) -> Option<OperationId> {
    match operation {
        AbstractOperation::EstablishRecord { psi_operation, .. }
        | AbstractOperation::EstablishScalarArray { psi_operation, .. }
        | AbstractOperation::EstablishScalarCase { psi_operation, .. }
        | AbstractOperation::EstablishPrimitiveLocal { psi_operation, .. }
        | AbstractOperation::CallStructural { psi_operation, .. }
        | AbstractOperation::CallStructuralScalar { psi_operation, .. }
        | AbstractOperation::CallUnit { psi_operation, .. }
        | AbstractOperation::BoundaryCall { psi_operation, .. } => Some(*psi_operation),
        _ => None,
    }
}
