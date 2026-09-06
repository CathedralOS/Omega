//! Exact function-scope replay for finite affine projected call carriers.

#[cfg(test)]
mod tests;

use machine_code::{
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, UnitAffineCleanupRecord,
    UnitParameterHomeRecord,
};
use target_operations::CallSiteOwner;
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

pub(crate) fn exact_fully_consumed_affine_parameter(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> bool {
    cleanup.is_some_and(|cleanup| cleanup.actions.is_empty())
        && exact_affine_projected_calls(parameter_homes, calls, cleanup)
}

pub(crate) fn exact_partially_consumed_affine_parameter(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> bool {
    cleanup.is_some_and(|cleanup| !cleanup.actions.is_empty())
        && exact_affine_projected_calls(parameter_homes, calls, cleanup)
}

fn exact_affine_projected_calls(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &[InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> bool {
    let ([home], Some(cleanup)) = (parameter_homes, cleanup) else {
        return false;
    };
    if calls.is_empty()
        || home.multiplicity != StructuralMultiplicity::Affine
        || home.access != StructuralAccess::Owned
        || !cleanup.locals.is_empty()
        || calls
            .iter()
            .map(|call| call.owner)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != calls.len()
        || calls.windows(2).any(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_none_or(|end| end > pair[1].code_offset)
        })
        || calls.iter().enumerate().any(|(ordinal, call)| {
            let [argument] = call.arguments.as_slice() else {
                return true;
            };
            !matches!(call.owner, CallSiteOwner::Operation(_))
                || call.operation_ordinal != ordinal
                || call.result.is_some()
                || call.semantic_result.is_some()
                || call.structural_result.is_some()
                || !call.scalar_arguments.is_empty()
                || !call.claim_transfers.is_empty()
                || !exact_owned_projection(argument, home, &cleanup.structural_types)
        })
    {
        return false;
    }
    let residuals = cleanup
        .actions
        .iter()
        .filter_map(|action| match action {
            TerminalAffineCleanupAction::DiscardResidual(residual) => Some(residual),
            _ => None,
        })
        .collect::<Vec<_>>();
    if residuals.len() != cleanup.actions.len()
        || residuals
            .iter()
            .any(|residual| residual.place != home.place)
    {
        return false;
    }
    let moved = calls
        .iter()
        .map(|call| {
            let argument = &call.arguments[0];
            (argument.path.as_slice(), argument.structural_type)
        })
        .collect::<Vec<_>>();
    crate::exact_partial_cleanup_partition(
        &cleanup.structural_types,
        home.structural_type,
        &moved,
        &residuals,
    )
}

/// Replay the whole path, retaining root-array metadata even for deeper leaves.
/// Record-root mixed paths carry no array metadata.
pub(crate) fn exact_owned_projection(
    argument: &InternalUnitCallArgumentRecord,
    home: &UnitParameterHomeRecord,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    exact_owned_projection_from_root(
        argument,
        home.place,
        home.structural_type,
        home.shape,
        &home.source,
        home.location,
        declarations,
    ) && home.access == StructuralAccess::Owned
        && home.multiplicity == StructuralMultiplicity::Affine
}

pub(crate) fn exact_owned_result_projection(
    argument: &InternalUnitCallArgumentRecord,
    result: &machine_code::InternalStructuralCallResult,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Some(home) = &result.result_home else {
        return false;
    };
    exact_owned_projection_from_root(
        argument,
        result.operation_result.place,
        result.operation_result.structural_type,
        home.requirement.layout.shape(),
        &result.caller_result_placement,
        machine_code::StructuralSourceLocation::Stack {
            byte_offset: home.home_byte_offset,
        },
        declarations,
    )
}

fn exact_owned_projection_from_root(
    argument: &InternalUnitCallArgumentRecord,
    place: semantic_vocabulary::PlaceId,
    structural_type: semantic_vocabulary::StructuralTypeId,
    shape: calling_conventions::ValueShape,
    source: &calling_conventions::ValuePlacement,
    location: machine_code::StructuralSourceLocation,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Some((leaf_type, leaf_shape, byte_offset)) =
        crate::structural_condition_layout::replay_structural_projection(
            structural_type,
            &argument.path,
            declarations,
        )
    else {
        return false;
    };
    let Some(root_shape) = crate::structural_condition_layout::replay_structural_value_shape(
        structural_type,
        declarations,
    ) else {
        return false;
    };
    let Some(root) = declarations
        .iter()
        .find(|declaration| declaration.id == structural_type)
    else {
        return false;
    };
    let metadata_matches = match root.shape {
        StructuralTypeShape::FixedArray { element, length } => {
            crate::structural_condition_layout::replay_structural_value_shape(element, declarations)
                .and_then(|shape| {
                    u32::from(shape.byte_size).checked_next_multiple_of(u32::from(shape.alignment))
                })
                .is_some_and(|stride| {
                    argument.fixed_array_length == Some(length)
                        && argument.element_stride == Some(stride)
                })
        }
        StructuralTypeShape::Record { .. } => {
            argument.fixed_array_length.is_none() && argument.element_stride.is_none()
        }
        _ => false,
    };
    metadata_matches
        && !argument.path.is_empty()
        && argument.access == StructuralAccess::Owned
        && argument.place == place
        && argument.root_structural_type == structural_type
        && argument.structural_type == leaf_type
        && argument.shape == leaf_shape
        && shape == root_shape
        && argument.source_byte_offset == byte_offset
        && &argument.source == source
        && argument.source.shape == shape
        && location
            .stack_byte_offset()
            .is_some_and(|offset| argument.source_location.stack_byte_offset() == Some(offset))
}

/// A real producer transfers the sole input before any result-root projection.
/// The result remains distinct from the incoming parameter and is never added
/// to the parameter roster merely to reuse its physical source lookup.
pub(crate) fn exact_projected_affine_result<'a>(
    parameter_homes: &[UnitParameterHomeRecord],
    calls: &'a [InternalUnitCallRecord],
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> Option<&'a machine_code::InternalStructuralCallResult> {
    let ([parameter], Some(cleanup)) = (parameter_homes, cleanup) else {
        return None;
    };
    let (producer, consumers) = calls.split_first()?;
    let result = producer.structural_result.as_ref()?;
    let home = result.result_home.as_ref()?;
    let [input] = producer.arguments.as_slice() else {
        return None;
    };
    if consumers.is_empty()
        || !cleanup.locals.is_empty()
        || producer.operation_ordinal != 0
        || producer.owner != CallSiteOwner::Operation(home.requirement.defining_operation)
        || home.requirement.result != result.operation_result
        || !matches!(
            home.requirement.layout,
            target_operations::TargetStructuralHomeLayout::Aggregate(_)
        )
        || result.operation_result.multiplicity != StructuralMultiplicity::Affine
        || !result.operation_result.qualifications.is_empty()
        || !result.operation_result.projected_qualifications.is_empty()
        || !result.operation_result.claims.is_empty()
        || !result.returned_claim_transfers.is_empty()
        || !result.returned_claims.is_empty()
        || result.operation_result.place == parameter.place
        || result.operation_result.structural_type != parameter.structural_type
        || parameter.multiplicity != StructuralMultiplicity::Affine
        || parameter.access != StructuralAccess::Owned
        || parameter.shape != home.requirement.layout.shape()
        || input.place != parameter.place
        || !input.path.is_empty()
        || input.root_structural_type != parameter.structural_type
        || input.structural_type != parameter.structural_type
        || input.access != StructuralAccess::Owned
        || input.shape != parameter.shape
        || producer.result.is_some()
        || producer.semantic_result.is_some()
        || !producer.scalar_arguments.is_empty()
        || !producer.claim_transfers.is_empty()
        || calls
            .iter()
            .map(|call| call.owner)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != calls.len()
        || calls.windows(2).any(|pair| {
            pair[0]
                .code_offset
                .checked_add(pair[0].byte_count)
                .is_none_or(|end| end > pair[1].code_offset)
        })
        || consumers.iter().enumerate().any(|(ordinal, call)| {
            let [argument] = call.arguments.as_slice() else {
                return true;
            };
            !matches!(call.owner, CallSiteOwner::Operation(_))
                || call.operation_ordinal != ordinal + 1
                || call.result.is_some()
                || call.semantic_result.is_some()
                || call.structural_result.is_some()
                || !call.scalar_arguments.is_empty()
                || !call.claim_transfers.is_empty()
                || home
                    .code_offset
                    .checked_add(home.byte_count)
                    .is_none_or(|end| end > argument.code_offset)
                || !exact_owned_result_projection(argument, result, &cleanup.structural_types)
        })
    {
        return None;
    }
    let residuals = cleanup
        .actions
        .iter()
        .filter_map(|action| match action {
            TerminalAffineCleanupAction::DiscardResidual(residual) => Some(residual),
            _ => None,
        })
        .collect::<Vec<_>>();
    if residuals.len() != cleanup.actions.len()
        || residuals
            .iter()
            .any(|residual| residual.place != result.operation_result.place)
        || calls
            .last()?
            .code_offset
            .checked_add(calls.last()?.byte_count)?
            > cleanup.code_offset
    {
        return None;
    }
    let moved = consumers
        .iter()
        .map(|call| {
            let argument = &call.arguments[0];
            (argument.path.as_slice(), argument.structural_type)
        })
        .collect::<Vec<_>>();
    crate::exact_partial_cleanup_partition(
        &cleanup.structural_types,
        result.operation_result.structural_type,
        &moved,
        &residuals,
    )
    .then_some(result)
}
