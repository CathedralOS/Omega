//! Reconstruct a shared descriptor over exact live owner storage.
//! Qualification discharge belongs to the independently validated abstract
//! call. Native replay must retain that call's operand and declaration while
//! reconstructing backing, current extent location and placement, not demand
//! that the owner and descriptor have the same type or predicate roster.
use crate::LegalizationError;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use calling_conventions::{CallPlan, ValueShape};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{OperationId, StructuralPlaceKind};
use target_operations::{
    TargetOperationPlan, TargetStructuralArgument, TargetStructuralArgumentSource,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
};

pub(super) fn argument(
    actual: &StructuralArgument,
    call_operation: OperationId,
    caller: &PsiOptimizationFunction,
    formal: &StructuralParameterDeclaration,
    call: &CallPlan,
    ordinal: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<Option<TargetStructuralArgument>, LegalizationError> {
    if actual.access != StructuralAccess::SharedBorrow
        || actual.path.iter().any(|segment| {
            !matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
        || formal.access != actual.access
        || !terminal_semantics::descriptor_results::is_shared_descriptor(
            plan.structural_types.iter(),
            formal.structural_type,
        )
    {
        return Ok(None);
    }
    let invalid = LegalizationError::custody;
    let after = caller
        .blocks
        .iter()
        .find_map(|block| {
            block.nodes.iter().position(|node| matches!(&node.operation,
            AbstractOperation::CallStructural { psi_operation, structural_arguments, .. }
            | AbstractOperation::BoundaryCall { psi_operation, structural_arguments, .. }
            if *psi_operation == call_operation && structural_arguments.contains(actual)
        )).map(|position| (block.id, position))
        })
        .ok_or_else(invalid)?;
    let (root, source, result_custody) = if let Some(arrival) =
        caller.arrival_parameter(actual.place)
    {
        let place = caller
            .structural_places
            .iter()
            .find(|place| place.id == actual.place)
            .ok_or_else(invalid)?;
        let source = match place.kind {
            StructuralPlaceKind::Parameter { .. } => {
                let parameter = native
                    .functions
                    .iter()
                    .find(|function| function.machine == caller.machine)
                    .and_then(super::super::structural_parameters)
                    .and_then(|parameters| {
                        parameters
                            .iter()
                            .find(|parameter| parameter.place == actual.place)
                    })
                    .ok_or_else(invalid)?;
                if parameter.structural_type != arrival.structural_type
                    || parameter.access != arrival.access
                    || parameter.multiplicity != arrival.multiplicity
                    || parameter.projected_qualifications != arrival.projected_qualifications
                {
                    return Err(invalid());
                }
                parameter.placement.clone().into()
            }
            StructuralPlaceKind::BlockParameter { block, .. } => {
                if block != after.0
                    && !super::super::target::control_flow::sources::dominates(
                        caller, block, after.0,
                    )
                {
                    return Err(invalid());
                }
                TargetStructuralArgumentSource::BlockParameter {
                    block,
                    place: actual.place,
                }
            }
            _ => return Err(invalid()),
        };
        (arrival.clone(), source, None)
    } else {
        let mut results = caller.blocks.iter().flat_map(|block| {
            block
                .nodes
                .iter()
                .enumerate()
                .filter_map(move |(position, node)| {
                    let (producer, result) = node.operation.structural_result()?;
                    (result.place == actual.place).then_some((block.id, position, producer, result))
                })
        });
        let Some((block, position, producer, result)) = results.next() else {
            return Ok(None);
        };
        if results.next().is_some()
            || (block == after.0 && position >= after.1)
            || (block != after.0
                && !super::super::target::control_flow::sources::dominates(caller, block, after.0))
            || !caller.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind
                        == StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
        {
            return Err(invalid());
        }
        (
            StructuralParameterDeclaration {
                place: result.place,
                position: 0,
                is_self: false,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                access: StructuralAccess::Owned,
                qualifications: result.qualifications.clone(),
                projected_qualifications: result.projected_qualifications.clone(),
            },
            TargetStructuralArgumentSource::StructuralHome {
                psi_operation: producer,
            },
            Some(result),
        )
    };
    // Descriptor payload and bounded-owner presentation are distinct physical
    // carriers. A completed descriptor already has the formal's exact payload;
    // an owner needs the existing backing/extent presentation adapter.
    let offset = if actual.path.is_empty()
        && super::super::byte_views::descriptor_result(caller, actual.place, &plan.structural_types)
            .is_some_and(|(_, identity)| identity == formal.structural_type)
    {
        0
    } else {
        let Some((offset, _)) =
            crate::structural_inputs::structural_reference_input::bounded_byte_view(
                &root,
                actual,
                formal.structural_type,
                &plan.structural_types,
            )
        else {
            return Ok(None);
        };
        offset
    };
    let destination = call.parameters.get(ordinal).ok_or_else(invalid)?;
    if result_custody.is_some_and(|result| {
        !result.claims.is_empty() || !result.qualification_establishments.is_empty()
    }) || formal.is_self
        || formal.multiplicity != StructuralMultiplicity::Unrestricted
        || !formal.projected_qualifications.is_empty()
        || destination.shape != ValueShape::borrowed_reference(16, 8)
    {
        return Err(invalid());
    }
    Ok(Some(TargetStructuralArgument {
        place: actual.place,
        access: actual.access,
        path: actual.path.clone(),
        root_structural_type: root.structural_type,
        structural_type: formal.structural_type,
        shape: destination.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source,
        destination: destination.clone(),
    }))
}
