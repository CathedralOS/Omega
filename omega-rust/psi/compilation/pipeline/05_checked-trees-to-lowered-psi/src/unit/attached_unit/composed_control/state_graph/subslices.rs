//! Source-bound exclusive view windows evaluated only on their selected edge.
//!
//! A transfer retains its root and authored range; the endpoints are replayed
//! at the edge's `TransitionArgument` subslice site by the shared
//! `view_ranges` replay, which also emits the Terminal range. This module only
//! resolves the transfer's source place in the state's frontier.
use super::super::super::super::StructuralParameterDeclaration;
use super::super::super::view_ranges::{self, ViewRangeSite};
use super::super::super::{
    PlaceId, StructuralPlaceDeclaration, ValueDeclaration, allocate_dense, place_id, unsupported,
};
use super::super::catalogs::ComposedCatalogs;
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::view_subslice::ViewFamily;
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{CheckedStorageRoot, CheckedStructuralControlTransferSourcePlan};

/// The root, family, field projection and authored range of a subslice
/// transfer. `path` is empty unless the range views a fixed-array field
/// whole before narrowing it, mirroring the argument plan's contract.
pub(super) fn transfer_range(
    source: &CheckedStructuralControlTransferSourcePlan,
) -> Option<(
    CheckedStorageRoot,
    ViewFamily,
    &[checked_trees::CheckedUnitStructuralPathSegment],
    ExpressionHandle,
    &str,
)> {
    match source {
        CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            storage_type_identity,
        } => Some((
            *root,
            ViewFamily::Bytes,
            &[],
            *expression,
            storage_type_identity.as_str(),
        )),
        CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
            root,
            path,
            expression,
            storage_type_identity,
        } => Some((
            *root,
            ViewFamily::Elements,
            path,
            *expression,
            storage_type_identity.as_str(),
        )),
        _ => None,
    }
}

/// Emit one edge's subslice transfer into `destination`. A parameter root
/// narrows the state's own view parameter; a view-local root narrows the view
/// the state body published for that local. A range over a fixed-array field
/// (`self.items[a..b]`) names no established view, so the edge first
/// establishes the field's whole element view into a private temporary and
/// narrows that — the same `EstablishElementView` an `EstablishViewSubslice`
/// binding emits, sequenced inside the edge so only the selected arm
/// performs it. `view_identity` is the target parameter's narrowed view
/// identity, which the whole view's declaration shares.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    argument_ordinal: u32,
    transfer: &CheckedStructuralControlTransferSourcePlan,
    _state_parameters: &[StructuralParameterDeclaration],
    _evaluation: &Evaluation,
    destination: PlaceId,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    catalogs: &mut ComposedCatalogs<'_>,
    places: &mut Vec<StructuralPlaceDeclaration>,
    view_identity: &str,
    access: checked_trees::CheckedStructuralAccess,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let Some((root, family, collection, expression, storage_type_identity)) =
        transfer_range(transfer)
    else {
        return unsupported("Unit graph transfer is not a view subslice");
    };
    let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
        source: match family {
            ViewFamily::Bytes => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root,
                    expression,
                    start: None,
                    end: None,
                    storage_type_identity: storage_type_identity.to_owned(),
                }
            }
            ViewFamily::Elements => {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                    root,
                    expression,
                    start: None,
                    end: None,
                    storage_type_identity: storage_type_identity.to_owned(),
                }
            }
        },
        path: collection.to_vec(),
        access,
        type_identity: view_identity.to_owned(),
    };
    let storage = view_ranges::storage(
        checked,
        state.state,
        statement_ordinal,
        &argument,
        &catalogs.structural_types,
    )?;
    let whole = storage
        .requires_establishment()
        .then(|| allocate_dense(&mut catalogs.next_place).map(place_id))
        .transpose()?;
    let (source, establishment) = storage.bind(checked, state.state, bindings, whole)?;
    if let Some(establishment) = establishment {
        places.push(view_ranges::establish(&establishment, operations));
    }
    let site = ViewRangeSite {
        state: state.state,
        statement: statement_ordinal,
        site: checked_trees::CheckedSubsliceSite::TransitionArgument { argument_ordinal },
        expression,
        retained: None,
    };
    view_ranges::emit(
        checked,
        site,
        source,
        collection,
        source.structural_type,
        destination,
        bindings,
        values,
        next_value,
        operations,
    )
}
