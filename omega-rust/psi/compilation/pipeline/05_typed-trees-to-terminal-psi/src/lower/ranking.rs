/*
The ranking of a call cycle lowered as a loop.

A machine that calls itself and writes `terminates by remaining` promises
that the loop ends and names the measure that shows it. Lowering proves
nothing about that: it states the claim on the lowered form, in the row
Terminal Psi already has for it (`TerminalMachine::ranked_scc`), and mints
the obligations the checker decides.

`wiki/spec/terminal-psi/control_flow.md`, "Safety and progress": "A ranking
declaration binds a well-founded relation, block ranks, and every derived
in-component edge." and "One grouped control judgment covers each
reconstructed component, with a shared well-foundedness question and
comparison obligations for every internal edge." So for the loop's cycle of
blocks this module writes:

- one rank per block of the cycle: the head parameter that stands for the
  witness's subject. The member's body is lowered beneath its head, so that
  one value is in scope in every block of the cycle;
- one row per edge inside the cycle. An edge that stays beneath the head
  keeps the rank (`Preserving`, the same value). A jump back to the head is
  the recursive call: its successor rank is the argument the call passes for
  the subject, and it claims `Strict`, because the page asks that "Removing
  strict edges must leave an acyclic graph" and every cycle of this graph
  closes through such a jump;
- one `WellFoundedness` obligation for the cycle and one `RankComparison`
  per edge row. The checker states and proves their goals
  (`terminal-psi-to-terminal-psi`, `ranking`); nothing here says they hold.

Only a cycle of one machine is ranked: `components` refuses a witness on a
cycle of several machines by name.
*/

use arena::Arena;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType, ValueId};
use terminal_check_tables::obligations::{ObligationKind, ObligationRow};
use terminal_check_tables::{
    Checked, Construct, MachineTables, ProgramPoint, Refusal, RefusalReason,
};
use terminal_psi::{
    TerminalBlockNaturalRank, TerminalIntegerRankValue, TerminalMachine, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc, Terminator,
};

use crate::{MachineSources, SourceOrigin, Unpositioned};

/// The loop head a witness ranks: the head's block, and which of its
/// parameters stands for the witness's subject.
#[derive(Debug, Clone, Copy)]
pub(super) struct RankedHead {
    pub block: BlockId,
    pub subject: usize,
    pub rank: ValueId,
}

/// Each edge that leaves `block` for another block, with the scalar
/// arguments it passes. A dispatch arm binds its parameters from the payload
/// and passes none.
fn successors(block: &terminal_psi::Block) -> Vec<(EdgeId, BlockId, &[ValueId])> {
    match &block.terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            ..
        } => vec![(*edge, *target, arguments.as_slice())],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![
            (
                when_true.edge,
                when_true.target,
                when_true.arguments.as_slice(),
            ),
            (
                when_false.edge,
                when_false.target,
                when_false.arguments.as_slice(),
            ),
        ],
        Terminator::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| (case.edge, case.target, &[][..]))
            .collect(),
        Terminator::InvokeNoNormal { .. }
        | Terminator::ExternalCompletion { .. }
        | Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => Vec::new(),
    }
}

/// Writes the ranking of the one cycle `head` closes in `machine` and mints
/// its obligations. `None` when no path of the lowered body jumps back: the
/// graph then has no cycle and nothing is ranked.
pub(super) fn rank(
    machine: &TerminalMachine,
    head: RankedHead,
    tables: &mut MachineTables,
    sources: &mut MachineSources,
) -> Checked<Option<TerminalRankedScc>> {
    let internal = |detail: &str| Refusal {
        reason: RefusalReason::Unsupported,
        machine: machine.id,
        concerns: Construct::Block(head.block),
        detail: format!("completion {detail}"),
    };
    let Some(ScalarType::Integer(integer_type)) =
        tables.values.get(head.rank).map(|row| row.scalar_type)
    else {
        return Err(internal("a rank that is no integer value"));
    };
    let components = terminal_semantics::control_graph::cyclic_components(machine);
    let members = match components.as_slice() {
        [] => return Ok(None),
        [members] if members.contains(&head.block) => members,
        _ => {
            return Err(internal(
                "a ranked loop whose cycle is not its head's alone",
            ));
        }
    };
    let mut cycle = TerminalNaturalCycle {
        observations: Arena::new(),
        ranks: Vec::with_capacity(members.len()),
        edges: Vec::new(),
    };
    let observe = |cycle: &mut TerminalNaturalCycle, value| {
        cycle.observations.insert_many([TerminalIntegerRankValue {
            integer_type,
            value,
        }])
    };
    // `cyclic_components` lists a component's blocks in identity order.
    for block in members {
        let values = observe(&mut cycle, head.rank);
        cycle.ranks.push(TerminalBlockNaturalRank {
            block: *block,
            values,
        });
    }
    for block in &machine.blocks {
        if !members.contains(&block.id) {
            continue;
        }
        for (edge, target, arguments) in successors(block) {
            if !members.contains(&target) {
                continue;
            }
            let (successor, comparison) = if target == head.block {
                let argument = arguments
                    .get(head.subject)
                    .ok_or_else(|| internal("a jump back that passes no rank"))?;
                (*argument, TerminalNaturalRankComparison::Strict)
            } else {
                (head.rank, TerminalNaturalRankComparison::Preserving)
            };
            let successor_ranks = observe(&mut cycle, successor);
            cycle.edges.push(TerminalNaturalRankEdge {
                edge,
                source: block.id,
                target,
                successor_ranks,
                comparison,
            });
        }
    }
    cycle.edges.sort_unstable_by_key(|edge| edge.edge);
    let component = terminal_semantics::control_graph::control_cycle_identity(machine, &cycle);
    // The shared question is asked where the cycle's first block begins.
    // That is the head: its body's blocks are made after it.
    tables.obligations.push(ObligationRow {
        kind: ObligationKind::WellFoundedness { component },
        created_at: ProgramPoint::BlockEntry(members[0]),
    });
    sources
        .obligations
        .push(SourceOrigin::Unpositioned(Unpositioned::LoopHead).into());
    for edge in &cycle.edges {
        // A comparison is reported where its edge was written: the
        // recursive call, for a jump back.
        let source = *sources
            .edges
            .get(edge.edge)
            .ok_or_else(|| internal("a ranked edge has no source row"))?;
        tables.obligations.push(ObligationRow {
            kind: ObligationKind::RankComparison {
                component,
                edge: edge.edge,
            },
            created_at: ProgramPoint::Edge(edge.edge),
        });
        sources.obligations.push(source);
    }
    Ok(Some(TerminalRankedScc::Natural(vec![cycle])))
}
