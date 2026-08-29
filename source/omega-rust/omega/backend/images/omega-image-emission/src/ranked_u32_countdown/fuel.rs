//! Exact nine-row logical-fuel replay over target-decoded byte spans.

use omega_machine_code::{
    NativeFuelAttribution, NativeFuelSite, RankedU32CountdownMachineCodeRecord,
};
use psi_terminal::TerminalRankedGuard;

use super::layout::RankedCountdownLayout;

pub(super) fn replay_ranked_countdown_fuel(
    record: &RankedU32CountdownMachineCodeRecord,
    actual: &[NativeFuelAttribution],
    layout: RankedCountdownLayout,
) -> bool {
    let graph = record.custody.graph;
    let covered = &record.custody.ranked_scc.covered_cyclic_edges[0];
    let TerminalRankedGuard::UnsignedParameterPositive {
        edge: guard_edge, ..
    } = covered.guard;
    let expected = [
        (NativeFuelSite::Edge(graph.preheader_edge), layout.preheader),
        (NativeFuelSite::Operation(graph.zero_operation), layout.zero),
        (
            NativeFuelSite::Operation(graph.compare_operation),
            layout.compare,
        ),
        (NativeFuelSite::Edge(guard_edge), layout.positive_edge),
        (NativeFuelSite::Operation(graph.one_operation), layout.one),
        (
            NativeFuelSite::Operation(graph.subtract_operation),
            layout.subtract,
        ),
        (NativeFuelSite::Edge(covered.edge), layout.backedge),
        (
            NativeFuelSite::Edge(graph.false_exit_edge),
            layout.false_exit,
        ),
        (NativeFuelSite::Edge(graph.return_edge), layout.returned),
    ];
    let schedule = record.custody.fixed_fuel.schedule();
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .enumerate()
            .all(|(ordinal, (actual, (site, span)))| {
                actual.schedule == schedule
                    && actual.site == site
                    && actual.units == 1
                    && actual.operation_ordinal == ordinal
                    && actual.code_offset == span.offset
                    && actual.byte_count == span.byte_count
            })
}
