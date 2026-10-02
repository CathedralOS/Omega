//! Source-bound exclusive view windows evaluated only on their selected edge.
//!
//! A transfer retains its root and authored range; the endpoints are replayed
//! at the edge's `TransitionArgument` subslice site by the shared
//! `view_ranges` replay, which also emits the Terminal range. This module only
//! resolves the transfer's source place in the state's frontier.
use super::super::super::super::StructuralParameterDeclaration;
use super::super::super::view_ranges::{self, ViewRangeSite, ViewRangeSource};
use super::super::super::{
    Operation, OperationKind, OperationResult, PlaceId, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, ValueDeclaration, allocate_dense, lookup_type_id,
    place_id, unsupported,
};
use super::super::catalogs::ComposedCatalogs;
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::view_subslice::ViewFamily;
use crate::expression_preparation::bindings::structural_paths::lower_structural_path;
use crate::expression_preparation::bindings::view_locals;
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
)> {
    match source {
        CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice { root, expression } => {
            Some((*root, ViewFamily::Bytes, &[], *expression))
        }
        CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
            root,
            path,
            expression,
        } => Some((*root, ViewFamily::Elements, path, *expression)),
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
    state_parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    destination: PlaceId,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
    catalogs: &mut ComposedCatalogs<'_>,
    places: &mut Vec<StructuralPlaceDeclaration>,
    view_identity: &str,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let Some((root, family, collection, expression)) = transfer_range(transfer) else {
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
            if collection.is_empty() {
                ViewRangeSource {
                    symbol,
                    place: parameter.place,
                    structural_type: parameter.structural_type,
                    family,
                }
            } else {
                establish_field_view(
                    catalogs,
                    evaluation,
                    parameter,
                    symbol,
                    collection,
                    family,
                    view_identity,
                    operations,
                    places,
                )?
            }
        }
        CheckedStorageRoot::ViewLocal { symbol } => {
            if !collection.is_empty() {
                return unsupported("Unit graph subslice view local lost its checked path");
            }
            let local = view_locals::resolve(&evaluation.view_locals, symbol)?;
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
        CheckedStorageRoot::OwnedLocal { .. } => {
            return unsupported("Unit graph subslice source is an owned local, not a view");
        }
    };
    let site = ViewRangeSite {
        state: state.state,
        statement: statement_ordinal,
        site: checked_trees::CheckedSubsliceSite::TransitionArgument { argument_ordinal },
        expression,
        retained: None,
    };
    if collection.is_empty() {
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
    } else {
        // The field range's `source` is the whole element view
        // `establish_field_view` minted below the parameter; `collection`
        // still names the authored path the endpoint replay checks.
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
}

/// The whole element view an edge establishes over the fixed-array field
/// `collection` below `parameter` before narrowing it. The checked plan
/// admitted this range only with the transition's own read access of the
/// ranged place in the borrow ledger, so the loan the emitted view names is
/// already the one the checker proved live.
#[allow(clippy::too_many_arguments)]
fn establish_field_view(
    catalogs: &mut ComposedCatalogs<'_>,
    evaluation: &Evaluation,
    parameter: &StructuralParameterDeclaration,
    symbol: symbols::SymbolHandle,
    collection: &[checked_trees::CheckedUnitStructuralPathSegment],
    family: ViewFamily,
    view_identity: &str,
    operations: &mut OperationBuffer,
    places: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<ViewRangeSource, LoweringError> {
    if family != ViewFamily::Elements
        || !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::Owned
        )
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return unsupported("Unit graph subslice field owner cannot lend a shared element view");
    }
    let structural_type: StructuralTypeId = lookup_type_id(&catalogs.type_ids, view_identity)?;
    let element = catalogs
        .structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            terminal_psi::StructuralTypeShape::ElementView { element }
                if declaration.id == structural_type =>
            {
                Some(element)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "Unit graph subslice field range lost its element view type",
        ))?;
    let whole = place_id(allocate_dense(&mut catalogs.next_place)?);
    let producer = operations.allocate();
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: whole,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishElementView {
            destination: whole,
            source: terminal_psi::StructuralArgument {
                place: evaluation.current_structural_place(parameter.place),
                path: lower_structural_path(collection)?,
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            element,
        },
    });
    places.push(StructuralPlaceDeclaration {
        id: whole,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    });
    Ok(ViewRangeSource {
        symbol,
        place: whole,
        structural_type,
        family,
    })
}
