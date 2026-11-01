//! Source-bound exclusive view windows evaluated only on their selected edge.
//!
//! A transfer carries the same range argument a call or binding does, its
//! endpoints keyed at the edge's `TransitionArgument` subslice site; the
//! shared `view_ranges` emission evaluates them and narrows the view. This
//! module only resolves the transfer's source place in the state's frontier.
use super::super::super::super::StructuralParameterDeclaration;
use super::super::super::view_ranges::{self, ViewRangeSite};
use super::super::super::{PlaceId, StructuralPlaceDeclaration, ValueDeclaration, unsupported};
use super::super::catalogs::ComposedCatalogs;
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::Evaluation;
use crate::unit::attached_unit::composed_control::emission::{FrameValues, StateFrame};
use checked_trees::CheckedStructuralControlTransferSourcePlan;

/// The range argument a subslice transfer narrows on its edge. Its `path` is
/// empty unless the range views a fixed-array field whole before narrowing
/// it, the same contract a call argument's range carries.
pub(super) fn transfer_range(
    source: &CheckedStructuralControlTransferSourcePlan,
) -> Option<&checked_trees::CheckedUnitStructuralArgumentPlan> {
    match source {
        CheckedStructuralControlTransferSourcePlan::ViewSubslice(argument) => Some(argument),
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
/// performs it.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    statement_ordinal: u32,
    transfer: &CheckedStructuralControlTransferSourcePlan,
    state_parameters: &[StructuralParameterDeclaration],
    evaluation: &mut Evaluation,
    destination: PlaceId,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    catalogs: &mut ComposedCatalogs<'_>,
    places: &mut Vec<StructuralPlaceDeclaration>,
    owner: symbols::SymbolHandle,
    erased_parameters: &[ValueDeclaration],
    sequence: &mut super::super::emission::OperationSequence,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let Some(argument) = transfer_range(transfer) else {
        return unsupported("Unit graph transfer is not a view subslice");
    };
    let range = view_ranges::range(argument).ok_or(LoweringError::Unsupported(
        "Unit graph subslice transfer lost its range",
    ))?;
    let (source, establishment) = view_ranges::stage_source(
        identities,
        checked,
        state.state,
        statement_ordinal,
        argument,
        catalogs.structural_types.as_ref(),
        bindings,
        &mut catalogs.next_place,
        values,
        next_value,
        operations,
    )?;
    if let Some(declaration) = establishment {
        places.push(declaration);
    }
    let site = ViewRangeSite {
        state: state.state,
        statement: statement_ordinal,
        range: range.endpoints(),
    };

    let source_value_count = values.len();
    let result_type = source.structural_type;
    StateFrame::new(checked, owner, state, erased_parameters).evaluate(
        catalogs,
        state_parameters,
        sequence,
        FrameValues {
            source_value_count,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
        },
        |frame| {
            let (mut environment, emission) = frame.split();
            view_ranges::emit(
                identities,
                checked,
                site,
                source,
                result_type,
                destination,
                emission.evaluation,
                source_value_count,
                emission.values,
                emission.next_value,
                emission.next_block,
                emission.next_edge,
                emission.operations,
                emission.calls,
                &mut environment,
            )
        },
    )
}
