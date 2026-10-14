//! Independent replay of borrowed address joins against the optimized unit.
//!
//! An address join (`abstract_operations::control_flow::address_joins`) is a
//! borrowed block parameter whose carrier is its referent's address. Target
//! lowering classified each edge source and call argument; this replay
//! reconstructs both from the optimized function alone, without reading the
//! target's classification:
//!
//! - every edge into the join lends a non-linear, unqualified root at exactly
//!   the permitted access, whose owner dominates the edge (a local producer, an owned block arrival,
//!   a machine parameter or an earlier address join), through a static
//!   projection naming exactly the join's referent type;
//! - a call argument projects from the joined referent, without widening
//!   access, from a join whose block dominates the call, at the callee's exact
//!   borrowed shape and with available runtime selectors.
//!
//! Loan duration is not decided here: the unit's current-ownership replay pins
//! each lending root while a join can access it, including write-only loans.

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

/// Rejoin a borrowed projection lent by an address join without widening access.
/// Semantic call admission has already checked the formal's qualifications;
/// replay reconstructs its original address, projection and ABI placement.
pub(in crate::legalization) fn call_argument(
    semantic: &terminal_psi::StructuralArgument,
    destination: &StructuralParameterDeclaration,
    call_operation: OperationId,
    caller: &PsiOptimizationFunction,
    call: &CallPlan,
    parameter_ordinal: usize,
    native: &target_operations::TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let (block, parameter) =
        parameter(caller, semantic.place, plan).ok_or(LegalizationError::custody())?;
    let call_block = operation_block(caller, call_operation).ok_or(LegalizationError::custody())?;
    if let Some(window) = window_argument(
        semantic,
        destination,
        block,
        parameter,
        call,
        parameter_ordinal,
        plan,
    ) {
        return if dominates(caller, block, call_block) {
            window
        } else {
            Err(LegalizationError::custody())
        };
    }
    let (selected, offset, elements) =
        crate::structural_inputs::structural_reference_input::runtime_borrowed_projection(
            parameter.structural_type,
            &semantic.path,
            semantic.access,
            Some(destination.structural_type),
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
    let shape = crate::structural_inputs::structural_reference_input::internal_borrow_shape(
        selected,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    let placement = call
        .parameters
        .get(parameter_ordinal)
        .ok_or(LegalizationError::custody())?;
    if !loan_access_permitted(parameter.access, semantic.access)
        || destination.access != semantic.access
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || destination.structural_type != selected
        || !dominates(caller, block, call_block)
        || placement.shape != shape
    {
        return Err(LegalizationError::custody());
    }
    Ok(target_operations::TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: parameter.structural_type,
        structural_type: selected,
        shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices: super::target::control_flow::sources::call_indices(
            native,
            caller,
            call_operation,
            &elements,
        )?,
        source: target_operations::TargetStructuralArgumentSource::BlockParameter {
            block,
            place: parameter.place,
        },
        destination: placement.clone(),
    })
}

/// A fixed-array window of the joined referent, presented to a borrowed byte
/// or element view exactly as an incoming parameter's field window is: the
/// descriptor carries the backing's address and element count. `None` when
/// the argument presents no such window.
fn window_argument(
    semantic: &terminal_psi::StructuralArgument,
    destination: &StructuralParameterDeclaration,
    block: BlockId,
    parameter: &StructuralParameterDeclaration,
    call: &CallPlan,
    parameter_ordinal: usize,
    plan: &AbstractOperationPlan,
) -> Option<Result<target_operations::TargetStructuralArgument, LegalizationError>> {
    let types = &plan.structural_types;
    let (source_byte_offset, length, stride) =
        crate::structural_inputs::structural_reference_input::fixed_byte_array_view(
            parameter,
            semantic,
            destination.structural_type,
            types,
        )
        .map(|(offset, length)| (offset, length, 1))
        .or_else(|| {
            crate::structural_inputs::structural_reference_input::fixed_element_array_view(
                parameter,
                semantic,
                destination.structural_type,
                types,
            )
        })?;
    let shape = calling_conventions::ValueShape::borrowed_reference(16, 8);
    let Some(placement) = call.parameters.get(parameter_ordinal) else {
        return Some(Err(LegalizationError::custody()));
    };
    if destination.access != semantic.access
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || placement.shape != shape
    {
        return Some(Err(LegalizationError::custody()));
    }
    Some(Ok(target_operations::TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: parameter.structural_type,
        structural_type: destination.structural_type,
        shape,
        source_byte_offset,
        fixed_array_length: Some(length),
        element_stride: Some(stride),
        indices: Vec::new(),
        source: target_operations::TargetStructuralArgumentSource::BlockParameter {
            block,
            place: parameter.place,
        },
        destination: placement.clone(),
    }))
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
    let Some(destination) = function.block(target) else {
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
                if abstract_operations::control_flow::owner_loans::is_bounded_owner_loan_in(
                    declaration,
                    &plan.structural_types,
                ) {
                    // A whole argument lends a completed owner or keeps the
                    // identical-declaration forward; a projection names the byte
                    // leaf whose carrier is the loan's referent — the same
                    // storage either spelling reaches.
                    return binding.parameter == declaration.place
                        && binding.argument.access == declaration.access
                        && if binding.argument.path.is_empty() {
                            bounded_owner_source(
                                function,
                                source_block,
                                binding.argument.place,
                                declaration,
                                plan,
                            )
                        } else {
                            abstract_operations::control_flow::address_joins::is_address_projection(
                                &binding.argument.path,
                            ) && root_type(
                                function,
                                source_block,
                                binding.argument.place,
                                &binding.argument.path,
                                declaration.access,
                                plan,
                            )
                            .and_then(|root| {
                                abstract_operations::control_flow::address_joins::borrowed_referent_type(plan.structural_types.iter(), root, &binding.argument.path, binding.argument.access)
                            })
                            .is_some_and(|referent| {
                                referent == declaration.structural_type
                            })
                        };
                }
                binding.parameter == declaration.place
                    && binding.argument.access == declaration.access
                    && abstract_operations::control_flow::address_joins::is_address_projection(
                        &binding.argument.path,
                    )
                    && root_type(
                        function,
                        source_block,
                        binding.argument.place,
                        &binding.argument.path,
                        declaration.access,
                        plan,
                    )
                    .and_then(|root| {
                        abstract_operations::control_flow::address_joins::borrowed_referent_type(plan.structural_types.iter(), root, &binding.argument.path, binding.argument.access)
                    })
                    .is_some_and(|referent| referent == declaration.structural_type)
            })
}

/// A bounded owner loan transports the address of its exact carrier. Completed
/// local results and forwarded parameters share that layout; the former must
/// dominate this edge, and the latter retain their incoming contract. Predicate
/// admission belongs to the checked abstract unit, not an instruction selector.
fn bounded_owner_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    place: PlaceId,
    declaration: &StructuralParameterDeclaration,
    plan: &AbstractOperationPlan,
) -> bool {
    if let Some((producer, structural_type)) =
        super::byte_views::owned_byte_result(function, place, &plan.structural_types)
    {
        return structural_type == declaration.structural_type
            && operation_block(function, producer)
                .is_some_and(|owner| owner == block || dominates(function, owner, block));
    }
    let identical = |source: &StructuralParameterDeclaration| {
        source.structural_type == declaration.structural_type
            && source.access == declaration.access
            && source.multiplicity == declaration.multiplicity
            && source.qualifications == declaration.qualifications
            && source.projected_qualifications.is_empty()
    };
    if let Some(source) = function.structural_parameter(place) {
        return identical(source);
    }
    parameter(function, place, plan)
        .is_some_and(|(owner, source)| identical(source) && dominates(function, owner, block))
}

/// Root attenuation for an argument lent by an existing address join. Its
/// runtime projection separately checks the endpoint and descriptor access;
/// static state edges use the canonical path query below.
fn loan_access_permitted(source: StructuralAccess, destination: StructuralAccess) -> bool {
    use StructuralAccess::{MutableBorrow, Owned, SharedBorrow, WriteOnlyBorrow};
    matches!(
        (source, destination),
        (
            MutableBorrow,
            SharedBorrow | MutableBorrow | WriteOnlyBorrow
        ) | (Owned | SharedBorrow, SharedBorrow)
            | (WriteOnlyBorrow, WriteOnlyBorrow)
    )
}

/// The non-linear, unqualified root type `place` names on an edge leaving
/// `block`, with sufficient access and an owner that dominates that edge.
fn root_type(
    function: &PsiOptimizationFunction,
    block: BlockId,
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    access: StructuralAccess,
    plan: &AbstractOperationPlan,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let plain = |multiplicity, qualifications: &[_], projected: &[_]| {
        multiplicity != StructuralMultiplicity::Linear
            && qualifications.is_empty()
            && projected.is_empty()
    };
    let permitted = |source: &StructuralParameterDeclaration| {
        abstract_operations::control_flow::address_joins::loan_access_permitted(
            plan.structural_types.iter(),
            source.structural_type,
            path,
            source.access,
            access,
        )
    };
    if let Some(source) = function.structural_parameter(place) {
        // Explicit parameters lend their declared referents. Namespace
        // attachment neither supplies a root nor changes its authority.
        return (permitted(source)
            && plain(
                source.multiplicity,
                &source.qualifications,
                &source.projected_qualifications,
            ))
        .then_some(source.structural_type);
    }
    if let Some((owner, joined)) = parameter(function, place, plan) {
        return ((owner == block || dominates(function, owner, block)) && permitted(joined))
            .then_some(joined.structural_type);
    }
    if matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())
        && !function.structural_places.iter().any(|declaration| {
            declaration.id == place
                && matches!(
                    declaration.kind,
                    StructuralPlaceKind::OperationResult { .. }
                )
        })
    {
        return None;
    }
    let (operation, result) = if let Some((operation, result, _)) =
        super::primitive_locals::producer(function, place)
    {
        (operation, result.clone())
    } else if let Some((operation, result)) =
        function
            .block_nodes()
            .find_map(|node| match &node.operation {
                AbstractOperation::EstablishElementView {
                    psi_operation,
                    result,
                    ..
                }
                | AbstractOperation::ElementViewSubslice {
                    psi_operation,
                    result,
                    ..
                } if result.place == place => Some((*psi_operation, result.clone())),
                _ => None,
            })
    {
        // An element view's descriptor root is a readable non-linear source:
        // the edge lends an element's address through its live extent.
        (operation, result)
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
            legalized_operations::LegalizedStructuralCaseSource::Parameter { .. }
            | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { .. } => {
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
            .any(|node| node.operation.psi_operation() == Some(operation))
            .then_some(block.id)
    })
}
