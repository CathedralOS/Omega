//! Conditional completed-value transfers, keyed by retained selection captures.
//! Availability is conservative; residual custody remains in the receipts until
//! the actual death edge. Every candidate keeps its physical source home;
//! selected transfer and complementary disposal retain their exact occurrences.
//! A shared-reference member owns descriptor storage, not its referent. Its
//! carrier therefore needs the same selected-source receipt as other owned
//! storage; loan checking independently retains its complete backing frontier.
//! The occurrence frontier ends only at an authenticated receiving write, not
//! at inherited provenance or a sibling edge's establishment.
pub(super) use availability::{apply_availability, retire_source_storage};
pub(super) use captures::SelectionProduction;
pub(in crate::checks::multiplicity) use captures::evaluated_source_custody;
pub(in crate::checks) use operands::reachable_arms;
mod availability;
mod captures;
mod occurrences;
mod operands;
mod routes;
pub(super) use occurrences::SelectionOccurrences;
pub(in crate::checks::multiplicity) use routes::terminal_arm_route;

/// Exact descendant claims and the affine containing home share source
/// availability. A projected transfer makes its containing value unavailable
/// for whole use or ordinary cleanup; it does not transfer an ancestor claim.
fn source_frontier_matches(
    place: &super::linear_obligations::LinearPlace,
    root: facts::PlaceRoot,
    path: &[facts::PlaceSegment],
) -> bool {
    place.root == root
        && (place.path.starts_with(path)
            || (place.multiplicity == language_semantics::Multiplicity::Affine
                && path.starts_with(&place.path)))
}

/// Two canonical paths overlap when one reaches inside the other or both
/// name the same place; a dynamic index cannot be statically excluded from a
/// fixed element claim, so it conflicts on the element axis.
pub(super) fn place_paths_overlap(
    used: &[facts::PlaceSegment],
    consumed: &[facts::PlaceSegment],
) -> bool {
    used.iter().zip(consumed.iter()).all(|(used, consumed)| {
        used == consumed
            || (matches!(used, facts::PlaceSegment::Index { .. })
                && matches!(
                    consumed,
                    facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::Index { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                ))
    })
}

/// Before a named source moves, every previously completed receiver retaining
/// it must belong to its actual published loan occurrence. This is attribution,
/// not a second availability walk: ordered borrow-move checking owns live-loan
/// exclusion at the exact source path and operand cut, including projections.
fn source_outputs_are_attributed(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    source: symbols::SymbolHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let Some(machine) = program.machine_by_symbol(machine) else {
        return false;
    };
    let Some(state) = program.state_in_machine(machine, state) else {
        return false;
    };
    let Some(flow) = facts.flow.control.unique_state(machine.symbol, state.symbol) else {
        return false;
    };
    let order = program.statement_table.admitted_execution_order(state.statement_nodes);
    let Some(position) = order.position(statement) else {
        return false;
    };
    for &index in &order.indices()[..position] {
        let Some(statement) = program.statement_table.statement_at(state, index) else {
            return false;
        };
        if !crate::checks::borrows::retained_outputs_are_attributed(
            program, facts, machine, flow, index, statement, source, frames,
        ) {
            return false;
        }
    }
    true
}
