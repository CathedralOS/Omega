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
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    optimized: &PsiOptimizationFunction,
    sources: &[(ValueId, Source)],
    plan: &AbstractOperationPlan,
    custody: &scalar_graph_input::reference_custody::Custody,
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
                return Err(invalid());
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
                return Err(invalid());
            }
            // The source's semantic place selects its real storage origin.
            // Incoming owned parameters need no invented producer/home; the
            // function-signature rejoin separately checks their ABI placement.
            if let Some(parameter) = optimized
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == value.place)
            {
                if parameter.access != StructuralAccess::Owned
                    || parameter.is_self
                    || parameter.structural_type != extent.field_type
                    || parameter.multiplicity == StructuralMultiplicity::Linear
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(invalid());
                }
            } else {
                let home = scalar_graph_input::aggregate_results::result_home(
                    optimized,
                    value.place,
                    plan,
                )?;
                if home.structural_type() != extent.field_type
                    || home.multiplicity() == StructuralMultiplicity::Linear
                    || home.has_claims()
                    || !home.qualifications().is_empty()
                    || !home.projected_qualifications().is_empty()
                    || home.layout.shape() != extent.shape
                {
                    return Err(invalid());
                }
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}
