//! Rejoin a borrowed window's two byte copies to their abstract rows.
//!
//! The move retains a `StructuralLeafCopy` row whose path is the spelled
//! window path extended by the vacated field, whose result home is the move's
//! own fresh result and which scales no runtime index. The store retains the
//! exact destination row, spelled path and field of its abstract row, the
//! consumed value's producer home, and the independently resolved field
//! offset. See `scalar_graph_input::borrowed_windows`.
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, TargetUnitOperation,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::custody;
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
                optimized, root, spelled, *field, result, plan,
            )?;
            if psi_operation != expected_operation
                || *source != root.place
                || *path != extent.path
                || *byte_offset != extent.byte_offset
                || !indices.is_empty()
                || *result_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        result.place,
                        plan,
                    )?
                || result_home.layout.shape() != extent.shape
            {
                return Err(invalid());
            }
        }
        (
            TargetUnitOperation::StoreStructuralField {
                psi_operation,
                destination,
                path,
                field,
                value_home,
                byte_offset,
            },
            AbstractOperation::StoreStructuralField {
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
            )?;
            if psi_operation != expected_operation
                || destination != expected_destination
                || path != expected_path
                || field != expected_field
                || *byte_offset != extent.byte_offset
                || value.access != StructuralAccess::Owned
                || !value.path.is_empty()
                || *value_home
                    != scalar_graph_input::aggregate_results::result_home(
                        optimized,
                        value.place,
                        plan,
                    )?
                || value_home.structural_type() != extent.field_type
                || value_home.multiplicity() == StructuralMultiplicity::Linear
                || value_home.has_claims()
                || !value_home.qualifications().is_empty()
                || !value_home.projected_qualifications().is_empty()
                || value_home.layout.shape() != extent.shape
            {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}
