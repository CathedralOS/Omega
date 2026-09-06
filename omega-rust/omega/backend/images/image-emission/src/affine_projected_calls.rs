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
    let Some((leaf_type, leaf_shape, byte_offset)) =
        crate::structural_condition_layout::replay_structural_projection(
            home.structural_type,
            &argument.path,
            declarations,
        )
    else {
        return false;
    };
    let Some(root_shape) = crate::structural_condition_layout::replay_structural_value_shape(
        home.structural_type,
        declarations,
    ) else {
        return false;
    };
    let Some(root) = declarations
        .iter()
        .find(|declaration| declaration.id == home.structural_type)
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
        && home.access == StructuralAccess::Owned
        && home.multiplicity == StructuralMultiplicity::Affine
        && argument.place == home.place
        && argument.root_structural_type == home.structural_type
        && argument.structural_type == leaf_type
        && argument.shape == leaf_shape
        && home.shape == root_shape
        && argument.source_byte_offset == byte_offset
        && argument.source == home.source
        && argument.source.shape == home.shape
        && argument.source_home_byte_offset == home.byte_offset
}
