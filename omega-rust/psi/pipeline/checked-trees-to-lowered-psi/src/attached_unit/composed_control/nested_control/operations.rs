//! Independent admission and emission of control-state operation prefixes.

use super::*;

pub(super) fn validate(state: &checked_trees::CheckedComposedUnitControlStatePlan) -> bool {
    state
        .operations
        .iter()
        .enumerate()
        .all(|(statement_index, operation)| {
            let exact_coordinate = |coordinate: &checked_trees::CheckedUnitCallCoordinate| {
                usize::try_from(coordinate.statement_index).ok() == Some(statement_index)
                    && coordinate.call_ordinal == 0
            };
            match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    coordinate,
                    structural_arguments,
                    claim_transfers,
                    ..
                } => {
                    exact_coordinate(coordinate)
                        && structural_arguments.is_empty()
                        && claim_transfers.is_empty()
                }
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    exact_coordinate(coordinate)
                        && scalar_arguments.is_empty()
                        && structural_arguments.is_empty()
                        && completion_receipts.is_empty()
                }
                _ => false,
            }
        })
}
