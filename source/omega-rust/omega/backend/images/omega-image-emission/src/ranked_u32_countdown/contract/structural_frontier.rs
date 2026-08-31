//! Ranked-header and backedge ownership-frontier equality.

use omega_machine_code::RankedU32CountdownMachineCodeRecord;
use psi_terminal::{StructuralAccess, StructuralMultiplicity};

pub(super) fn replay_structural_frontier(
    record: &RankedU32CountdownMachineCodeRecord,
) -> Option<()> {
    let component = &record.custody.ranked_scc;
    let covered = &component.covered_cyclic_edges[0];
    let structural = &record.structural_parameters[0];
    let [replay_structural] = record
        .custody
        .semantic_replay
        .machines
        .first()?
        .structural_parameters
        .as_slice()
    else {
        return None;
    };
    let header = record
        .custody
        .structural_frontiers
        .block_entry(component.header)?;
    let backedge = record
        .custody
        .structural_frontiers
        .edge_exit(covered.edge)?;
    let affine_owned = !replay_structural.is_self
        && replay_structural.multiplicity == StructuralMultiplicity::Affine
        && replay_structural.access == StructuralAccess::Owned;
    let persistent_receiver =
        replay_structural.is_self && replay_structural.access == StructuralAccess::MutableBorrow;
    let affine_frontier = matches!(header.owned_places(), [owned]
        if owned.place == structural.place
            && owned.multiplicity == StructuralMultiplicity::Affine);
    let receiver_frontier = header.owned_places().is_empty();
    (header == backedge
        && ((affine_owned && affine_frontier) || (persistent_receiver && receiver_frontier))
        && header.claims().is_empty()
        && header.partial_custody().is_empty()
        && structural.place == replay_structural.place
        && structural.structural_type == replay_structural.structural_type
        && structural.multiplicity == replay_structural.multiplicity
        && structural.access == replay_structural.access)
        .then_some(())
}
