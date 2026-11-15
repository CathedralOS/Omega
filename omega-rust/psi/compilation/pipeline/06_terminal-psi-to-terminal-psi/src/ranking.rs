/*
Rankings: whether a loop that promises to end has a measure that goes down.

A machine that calls itself and writes `terminates by remaining` is lowered
as a loop whose ranking says, for every block of the loop, which value is
its rank, and for every edge inside the loop, which value the rank becomes
and whether that is a strict decrease (`TerminalMachine::ranked_scc`). This
module judges that claim. Nothing in it trusts the producer: the cycle is
recomputed from the graph, every row is compared with what the graph owes,
and each comparison is a goal the one prover decides.

`wiki/spec/terminal-psi/control_flow.md`, "Safety and progress": "A ranking
declaration binds a well-founded relation, block ranks, and every derived
in-component edge. [...] Checking establishes well-foundedness and each
comparison". The judgment has a structural half and a proved half.

Structure (`admit`), refused whole when it fails:

- The ranked blocks are exactly one cycle of the graph and the edge rows are
  exactly that cycle's internal edges, in edge order: "The verifier derives
  each strongly connected component (SCC) from the actual graph" and
  "Missing, surplus, reordered, or substituted edge claims reject."
- A block's rank is a value in scope there: a machine formal, or a
  parameter of a block that dominates it. One fixed-width integer type at
  every row: "A rank's carrier is any fixed-width integer type".
- An edge's successor rank is its target's rank as that edge binds it:
  "Rank substitution follows the actual selected scalar and structural
  successor arguments." When the target's rank is the target's own
  parameter, that is the argument the edge passes for it; otherwise it is
  the same value, which the edge does not rebind.
- "Removing strict edges must leave an acyclic graph: every cycle encounters
  descent, not just the cycles selected by a traversal."
- A machine that publishes `Terminates` has no cycle without a ranking:
  `wiki/spec/language/termination.md`, "A cyclic terminating body must
  supply its witness".

Proof (`comparison_goal`, `floor_goal`), one certificate each:

- `RankComparison` on an edge: `successor_rank < source_rank` for a strict
  edge and `successor_rank <= source_rank` for a preserving one, decided at
  the edge, so a jump back has the branch decisions and the result equations
  of the iteration it leaves.
- `WellFoundedness`: the rank is bounded below by the least value of its
  carrier. With one carrier at every row and strict descent on every cycle,
  an execution cannot stay in the loop forever.

What is not judged here: that the calls a loop makes to other machines end.
Elaboration refuses a promise over a cycle no member ranks
(`typed-trees-to-terminal-psi`, `components`), and each ranked cycle is
judged in its own machine by this module.

Several values per rank (lexicographic ranks) are refused by name.
*/

use semantic_vocabulary::{
    BlockId, CycleComponentId, EdgeId, IntegerType, MachineId, ObligationId, Proposition,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    TerminalMachine, TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalRankedScc,
    Terminator,
};

use crate::graph::Graph;
use crate::refusal;

fn cycles(lowered: &TerminalMachine) -> &[TerminalNaturalCycle] {
    match &lowered.ranked_scc {
        Some(TerminalRankedScc::Natural(cycles)) => cycles,
        None => &[],
    }
}

fn identity(lowered: &TerminalMachine, cycle: &TerminalNaturalCycle) -> CycleComponentId {
    terminal_semantics::control_graph::control_cycle_identity(lowered, cycle)
}

/// The obligations the machine's ranking owes and where each is created:
/// one `WellFoundedness` per ranked cycle, at the entry of its first block,
/// and one `RankComparison` per edge row, on that edge.
pub(crate) fn owed(
    lowered: &TerminalMachine,
) -> impl Iterator<Item = (ObligationKind, ProgramPoint, Construct)> + '_ {
    cycles(lowered).iter().flat_map(move |cycle| {
        let component = identity(lowered, cycle);
        cycle
            .ranks
            .first()
            .map(|rank| {
                (
                    ObligationKind::WellFoundedness { component },
                    ProgramPoint::BlockEntry(rank.block),
                    Construct::Block(rank.block),
                )
            })
            .into_iter()
            .chain(cycle.edges.iter().map(move |edge| {
                (
                    ObligationKind::RankComparison {
                        component,
                        edge: edge.edge,
                    },
                    ProgramPoint::Edge(edge.edge),
                    Construct::Edge(edge.edge),
                )
            }))
    })
}

/// One rank of a cycle, `TerminalNaturalCycle::rank_values` of its row: its
/// single value and that value's carrier.
fn rank(
    values: Option<&[terminal_psi::TerminalIntegerRankValue]>,
    refuse: &impl Fn(RefusalReason, &str) -> Refusal,
) -> Checked<(ValueId, IntegerType)> {
    match values {
        Some([value]) => Ok((value.value, value.integer_type)),
        Some(_) => Err(refuse(
            RefusalReason::Unsupported,
            "contract lexicographic rank",
        )),
        None => Err(refuse(
            RefusalReason::ForeignIdentity,
            "a rank is no tuple of the cycle's fixed-width integer carrier",
        )),
    }
}

/// The rank of `block` in `cycle`.
fn block_rank(
    cycle: &TerminalNaturalCycle,
    block: BlockId,
    refuse: &impl Fn(RefusalReason, &str) -> Refusal,
) -> Checked<(ValueId, IntegerType)> {
    let row = cycle
        .ranks
        .iter()
        .find(|rank| rank.block == block)
        .ok_or_else(|| {
            refuse(
                RefusalReason::ForeignIdentity,
                "a ranked edge touches a block with no rank",
            )
        })?;
    rank(cycle.rank_values(row.values), refuse)
}

/// The scalar arguments `edge` passes, read from the terminator that owns
/// it. A dispatch arm binds its parameters from the payload and passes none.
fn arguments(lowered: &TerminalMachine, source: BlockId, edge: EdgeId) -> Option<&[ValueId]> {
    let block = lowered.blocks.iter().find(|block| block.id == source)?;
    match &block.terminator {
        Terminator::Jump {
            edge: jump,
            arguments,
            ..
        } if *jump == edge => Some(arguments),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .find(|successor| successor.edge == edge)
            .map(|successor| successor.arguments.as_slice()),
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .any(|case| case.edge == edge)
            .then_some(&[][..]),
        _ => None,
    }
}

/// Admits the machine's ranking before any of its comparisons is decided,
/// and refuses a terminating machine that leaves a cycle unranked.
pub(crate) fn admit(
    id: MachineId,
    lowered: &TerminalMachine,
    tables: &MachineTables,
    graph: &Graph,
) -> Checked<()> {
    let ranked = cycles(lowered);
    let terminates = lowered.contract.termination == semantic_vocabulary::Termination::Terminates;
    if ranked.is_empty() && (!terminates || lowered.blocks.is_empty()) {
        return Ok(());
    }
    // `prepare` has checked that every terminator's edge enters a block of
    // this machine, which is all the component walk asks of the graph.
    let actual = terminal_semantics::control_graph::cyclic_components(lowered);
    let mut claimed = vec![false; actual.len()];
    for cycle in ranked {
        let first = cycle.ranks.first().map(|rank| rank.block);
        let concerns = first.map_or(Construct::Machine, Construct::Block);
        let refuse = |reason, detail: &str| refusal(reason, id, concerns, detail);
        let foreign = |detail: &str| refuse(RefusalReason::ForeignIdentity, detail);
        let blocks: Vec<BlockId> = cycle.ranks.iter().map(|rank| rank.block).collect();
        let position = actual
            .iter()
            .position(|members| *members == blocks)
            .ok_or_else(|| foreign("a ranking names no cycle of the graph"))?;
        if std::mem::replace(&mut claimed[position], true) {
            return Err(foreign("two rankings name one cycle"));
        }
        // The cycle's internal edges, from the terminators themselves.
        let mut internal: Vec<(EdgeId, BlockId, BlockId)> = lowered
            .blocks
            .iter()
            .filter(|block| blocks.contains(&block.id))
            .flat_map(|block| {
                block
                    .terminator
                    .successor_edges()
                    .into_iter()
                    .filter(|(_, target)| blocks.contains(target))
                    .map(move |(edge, target)| (edge, block.id, target))
            })
            .collect();
        internal.sort_unstable_by_key(|(edge, _, _)| *edge);
        if !cycle
            .edges
            .iter()
            .map(|edge| (edge.edge, edge.source, edge.target))
            .eq(internal.iter().copied())
        {
            return Err(foreign("a ranking's edges are not its cycle's edges"));
        }
        let (_, carrier) = block_rank(cycle, blocks[0], &refuse)?;
        for block in &blocks {
            let (value, integer_type) = block_rank(cycle, *block, &refuse)?;
            let row = tables
                .values
                .get(value)
                .ok_or_else(|| foreign("a rank names no value"))?;
            if integer_type != carrier || row.scalar_type != ScalarType::Integer(carrier) {
                return Err(foreign("a rank is not declared at its cycle's carrier"));
            }
            let scoped = match row.origin {
                ValueOrigin::MachineParameter { .. } => true,
                ValueOrigin::BlockParameter { block: owner, .. } => graph.dominates(
                    ProgramPoint::BlockEntry(owner),
                    ProgramPoint::BlockEntry(*block),
                )?,
                ValueOrigin::MachineResult
                | ValueOrigin::Operation(_)
                | ValueOrigin::StorageContent { .. } => false,
            };
            if !scoped {
                return Err(refuse(
                    RefusalReason::Unsupported,
                    "contract block rank that is no parameter in scope",
                ));
            }
        }
        for edge in &cycle.edges {
            let (successor, integer_type) = rank(cycle.rank_values(edge.successor_ranks), &refuse)?;
            let (target_rank, _) = block_rank(cycle, edge.target, &refuse)?;
            let bound = lowered
                .blocks
                .iter()
                .find(|block| block.id == edge.target)
                .and_then(|block| {
                    block
                        .parameters
                        .iter()
                        .position(|parameter| parameter.id == target_rank)
                });
            let expected = match bound {
                Some(position) => arguments(lowered, edge.source, edge.edge)
                    .and_then(|arguments| arguments.get(position))
                    .copied(),
                None => Some(target_rank),
            };
            if integer_type != carrier || expected != Some(successor) {
                return Err(foreign(
                    "an edge's successor rank is not its target's rank under the edge's arguments",
                ));
            }
        }
        if !descends_on_every_cycle(cycle, &blocks) {
            return Err(foreign(
                "a ranking leaves a cycle with no strict descent on it",
            ));
        }
    }
    if terminates && let Some(position) = claimed.iter().position(|claimed| !claimed) {
        return Err(refusal(
            RefusalReason::Unsupported,
            id,
            actual[position]
                .first()
                .map_or(Construct::Machine, |block| Construct::Block(*block)),
            "contract termination promise over an unranked cycle",
        ));
    }
    Ok(())
}

/// Whether the cycle's edges that do not claim strict descent form no cycle
/// of their own: blocks are removed while one has no such edge entering it,
/// and a cycle is whatever is left.
fn descends_on_every_cycle(cycle: &TerminalNaturalCycle, blocks: &[BlockId]) -> bool {
    let preserving: Vec<(BlockId, BlockId)> = cycle
        .edges
        .iter()
        .filter(|edge| edge.comparison == TerminalNaturalRankComparison::Preserving)
        .map(|edge| (edge.source, edge.target))
        .collect();
    let mut left: Vec<BlockId> = blocks.to_vec();
    loop {
        let free = left.iter().position(|block| {
            !preserving
                .iter()
                .any(|(source, target)| target == block && left.contains(source))
        });
        match free {
            Some(position) => {
                left.swap_remove(position);
            }
            None => return left.is_empty(),
        }
    }
}

/// The ranked cycle `component` names in this machine, for the goal of
/// `obligation`.
fn cycle_of(
    lowered: &TerminalMachine,
    id: MachineId,
    obligation: ObligationId,
    component: CycleComponentId,
) -> Checked<&TerminalNaturalCycle> {
    cycles(lowered)
        .iter()
        .find(|cycle| identity(lowered, cycle) == component)
        .ok_or_else(|| {
            refusal(
                RefusalReason::ForeignIdentity,
                id,
                Construct::Obligation(obligation),
                "a termination obligation names no ranked cycle of its machine",
            )
        })
}

fn term(value: ValueId, carrier: IntegerType) -> ScalarTerm {
    ScalarTerm::value(value, ScalarType::Integer(carrier))
}

/// The goal of `RankComparison { component, edge }`: the rank the edge
/// arrives with against the rank of the block it leaves.
pub(crate) fn comparison_goal(
    lowered: &TerminalMachine,
    id: MachineId,
    obligation: ObligationId,
    component: CycleComponentId,
    edge: EdgeId,
) -> Checked<Proposition> {
    let cycle = cycle_of(lowered, id, obligation, component)?;
    let refuse =
        |reason, detail: &str| refusal(reason, id, Construct::Obligation(obligation), detail);
    let row = cycle
        .edges
        .iter()
        .find(|row| row.edge == edge)
        .ok_or_else(|| {
            refuse(
                RefusalReason::ForeignIdentity,
                "a rank comparison names no edge of its cycle",
            )
        })?;
    let (successor, carrier) = rank(cycle.rank_values(row.successor_ranks), &refuse)?;
    let (source, _) = block_rank(cycle, row.source, &refuse)?;
    let (successor, source) = (term(successor, carrier), term(source, carrier));
    Ok(match row.comparison {
        TerminalNaturalRankComparison::Strict => Proposition::LessThan(successor, source),
        TerminalNaturalRankComparison::Preserving => Proposition::LessOrEqual(successor, source),
    })
}

/// The goal of `WellFoundedness { component }`: the rank at the cycle's
/// first block is no less than the least value of the cycle's carrier.
pub(crate) fn floor_goal(
    lowered: &TerminalMachine,
    id: MachineId,
    obligation: ObligationId,
    component: CycleComponentId,
) -> Checked<Proposition> {
    let cycle = cycle_of(lowered, id, obligation, component)?;
    let refuse =
        |reason, detail: &str| refusal(reason, id, Construct::Obligation(obligation), detail);
    let first = cycle.ranks.first().ok_or_else(|| {
        refuse(
            RefusalReason::ForeignIdentity,
            "a ranked cycle ranks no block",
        )
    })?;
    let (value, carrier) = rank(cycle.rank_values(first.values), &refuse)?;
    let floor = carrier.minimum_value().ok_or_else(|| {
        refuse(
            RefusalReason::Unsupported,
            "contract rank of a carrier with no closed width",
        )
    })?;
    Ok(Proposition::LessOrEqual(
        ScalarTerm::Integer {
            value: floor,
            scalar_type: carrier,
        },
        term(value, carrier),
    ))
}

/// Whether `edge` claims strict descent in this machine's ranking, for the
/// wording of a failed comparison.
pub(crate) fn claims_strict(lowered: &TerminalMachine, edge: EdgeId) -> Option<bool> {
    cycles(lowered)
        .iter()
        .flat_map(|cycle| cycle.edges.iter())
        .find(|row| row.edge == edge)
        .map(|row| row.comparison == TerminalNaturalRankComparison::Strict)
}
