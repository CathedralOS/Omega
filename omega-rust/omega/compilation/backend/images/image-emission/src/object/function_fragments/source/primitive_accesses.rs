//! Join primitive observations and replacements to their exact graph occurrences.
//!
//! A logical referent access is not one fixed-width memory-footprint row:
//! traversal can read pointer slots, scalar references carry an address and a
//! runtime codec, and packed scalars can use several backing instructions.
//! The retained target, selection and physical audits own permission, geometry,
//! codec and instruction correctness. Publication accounts for the same source
//! operation here, without rebuilding their layout or def-use judgments.

use abstract_operations::AbstractOperation;
use target_operations::{TargetFunction, TargetUnitOperation};

pub(super) fn retained(source: &AbstractOperation, target: &TargetFunction) -> bool {
    match source {
        AbstractOperation::StorageAddress {
            psi_operation,
            result,
            source,
            path,
        } => {
            let mut queries = target
                .graph
                .operations()
                .filter_map(|operation| match operation {
                    TargetUnitOperation::StorageAddress {
                        psi_operation: retained,
                        result,
                        source,
                        path,
                        indices,
                    } if retained == psi_operation => Some((result, source, path, indices)),
                    _ => None,
                });
            matches!(queries.next(), Some((retained_result, retained_source, retained_path, indices))
                if retained_result == result && retained_source == source && retained_path == path
                    && super::structural_fields::retained_selectors(path, indices))
                && queries.next().is_none()
        }

        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            source,
            path,
        } => {
            let mut reads = target
                .graph
                .operations()
                .filter_map(|operation| match operation {
                    TargetUnitOperation::PrimitiveScalarRead {
                        psi_operation: retained,
                        result,
                        source,
                        path,
                        indices,
                    } if retained == psi_operation => Some((result, source, path, indices)),
                    _ => None,
                });
            matches!(reads.next(), Some((retained_result, retained_source, retained_path, indices))
                if retained_result == result && retained_source == source && retained_path == path
                    && super::structural_fields::retained_selectors(path, indices))
                && reads.next().is_none()
        }
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            path,
            value,
            ..
        } => {
            let mut stores = target
                .graph
                .operations()
                .filter_map(|operation| match operation {
                    TargetUnitOperation::WriteOnlyPrimitiveStore {
                        psi_operation: retained,
                        destination,
                        path,
                        source,
                        indices,
                        ..
                    } if retained == psi_operation => Some((destination, path, source, indices)),
                    _ => None,
                });
            matches!(stores.next(), Some((retained_destination, retained_path, stored, indices))
                if retained_destination == destination && retained_path == path
                    && stored.source_value() == value.value && stored.scalar_type() == value.scalar_type
                    && super::structural_fields::retained_selectors(path, indices))
                && stores.next().is_none()
        }
        _ => false,
    }
}
