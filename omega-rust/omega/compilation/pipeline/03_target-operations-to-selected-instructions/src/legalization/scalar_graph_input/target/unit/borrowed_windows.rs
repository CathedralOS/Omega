//! Rejoin a borrowed window's two byte copies to their abstract rows.
//!
//! The move retains a `StructuralLeafCopy` row whose path is the spelled
//! window path extended by the vacated field, whose result home is the move's
//! own fresh result and the exact runtime selectors. The store retains the
//! exact destination row, spelled path and field of its abstract row, the
//! consumed value's semantic place (local home or incoming parameter), and the independently resolved field
//! offset. See `scalar_graph_input::borrowed_windows`.
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, TargetUnitOperation, ValueId,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use target_operations::TargetUnitScalarArgumentSource as Source;
use terminal_psi::StructuralAccess;

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &PsiOptimizationFunction,
    sources: &[(ValueId, Source)],
    plan: &AbstractOperationPlan,
    custody: &scalar_graph_input::reference_custody::Custody,
) -> Result<(), LegalizationError> {
    match (target, abstracted) {
        (
            TargetUnitOperation::StructuralLeafCopy {
                psi_operation,
                result_home,
                source,
                path,
                byte_offset,
                indices,
            },
            AbstractOperation::MoveStructuralField {
                psi_operation: expected_operation,
                result,
                source: root,
                path: spelled,
                field,
            },
        ) => {
            let extent = scalar_graph_input::borrowed_windows::moved(
                optimized, root, spelled, *field, result, plan, custody,
            )?;
            if psi_operation != expected_operation
                || *source != root.place()
                || *path != abstract_operations::StructuralCopyPath::Ordinary(extent.path)
                || *byte_offset != extent.byte_offset
                || !super::runtime_indices_rejoin(indices, &extent.elements, sources)
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || result_home.layout.shape() != extent.shape
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::StoreStructuralField {
                psi_operation,
                destination,
                path,
                field,
                value: retained_value,
                byte_offset,
                indices,
            },
            AbstractOperation::StoreStructuralField {
                disposition,
                psi_operation: expected_operation,
                destination: expected_destination,
                path: expected_path,
                field: expected_field,
                value,
            },
        ) => {
            let extent = scalar_graph_input::borrowed_windows::extent(
                optimized,
                expected_destination,
                expected_path,
                *expected_field,
                plan,
                custody,
                disposition.required_access(),
            )?;
            if psi_operation != expected_operation
                || destination != expected_destination
                || path != expected_path
                || field != expected_field
                || *byte_offset != extent.byte_offset
                || !super::runtime_indices_rejoin(indices, &extent.elements, sources)
                || value.access != StructuralAccess::Owned
                || !value.path.is_empty()
                || *retained_value != value.place
            {
                return Err(LegalizationError::custody());
            }
            // Construction and replacement receive the same value contract.
            // A view arrival contributes descriptor storage, not ownership of
            // its backing. The exact type fixes the stored field's layout.
            if super::aggregate_results::owned_projected_field_source(
                optimized,
                value,
                &plan.structural_types,
            )? != extent.field_type
            {
                return Err(LegalizationError::custody());
            }
        }
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}
