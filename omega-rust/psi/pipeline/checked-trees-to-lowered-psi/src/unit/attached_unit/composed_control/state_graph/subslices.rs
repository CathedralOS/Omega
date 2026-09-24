//! Source-bound exclusive view windows evaluated only on their selected edge.
//!
//! A transfer retains its root and authored range; the endpoints are replayed
//! at the edge's `TransitionArgument` subslice site by the shared
//! `view_ranges` replay, which also emits the Terminal range. This module only
//! resolves the transfer's source place in the state's frontier.
use super::super::super::super::StructuralParameterDeclaration;
use super::super::super::view_ranges::{self, ViewRangeSite, ViewRangeSource};
use super::super::super::{PlaceId, StructuralPlaceDeclaration, ValueDeclaration, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::view_subslice::ViewFamily;
use crate::expression_preparation::bindings::view_locals::{self, ViewLocalBinding};
use checked_trees::expression::ExpressionHandle;
use checked_trees::{CheckedStorageRoot, CheckedStructuralControlTransferSourcePlan};

/// The root, family and authored range of a subslice transfer.
pub(super) fn transfer_range(
    source: &CheckedStructuralControlTransferSourcePlan,
) -> Option<(CheckedStorageRoot, ViewFamily, ExpressionHandle)> {
    match *source {
        CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { root, expression } => {
            Some((root, ViewFamily::Bytes, expression))
        }
        CheckedStructuralControlTransferSourcePlan::ElementViewSubslice { root, expression } => {
            Some((root, ViewFamily::Elements, expression))
        }
        _ => None,
    }
}

/// Emit one edge's subslice transfer into `destination`. A parameter root
/// narrows the state's own view parameter; a view-local root narrows the view
/// the state body published for that local.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    argument_ordinal: u32,
    transfer: &CheckedStructuralControlTransferSourcePlan,
    state_parameters: &[StructuralParameterDeclaration],
    view_locals: &[ViewLocalBinding],
    destination: PlaceId,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let Some((root, family, expression)) = transfer_range(transfer) else {
        return unsupported("Unit graph transfer is not a view subslice");
    };
    let source = match root {
        CheckedStorageRoot::Parameter { index } => {
            let parameter =
                state_parameters
                    .get(index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph subslice source descriptor disappeared",
                    ))?;
            let (_, authored) = crate::expression_preparation::source_custody::authored_state(
                checked,
                state.state,
            )?;
            let symbol = checked
                .state_parameters(authored)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "state subslice source has no authored parameter",
                ))?
                .symbol;
            ViewRangeSource {
                symbol,
                place: parameter.place,
                structural_type: parameter.structural_type,
                family,
            }
        }
        CheckedStorageRoot::ViewLocal { symbol } => {
            let local = view_locals::resolve(view_locals, symbol)?;
            if (family == ViewFamily::Bytes) != (local.carrier == view_locals::ViewCarrier::Bytes) {
                return unsupported("Unit graph subslice source local changed its view family");
            }
            ViewRangeSource {
                symbol,
                place: local.place,
                structural_type: local.structural_type,
                family,
            }
        }
    };
    view_ranges::emit(
        checked,
        ViewRangeSite {
            state: state.state,
            statement: statement_ordinal,
            site: checked_trees::CheckedSubsliceSite::TransitionArgument { argument_ordinal },
            expression,
            retained: None,
        },
        source,
        source.structural_type,
        destination,
        bindings,
        values,
        next_value,
        operations,
    )
}
