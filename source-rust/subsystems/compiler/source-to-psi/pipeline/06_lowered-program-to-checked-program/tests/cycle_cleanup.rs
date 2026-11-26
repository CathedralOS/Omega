//! A local that one iteration of a ranked cycle still uses when it jumps
//! back to the head must be disposed of on that back edge, once, because the
//! next iteration builds a new one. Stage 05 writes each disposal as a
//! cleanup action on the edge where the local dies, and the lowered-form
//! checker judges the program from those actions alone.
//!
//! The spec rules: a recursive call in terminal position continues the same
//! activation (`wiki/spec/language/termination.md`), and an affine value is
//! disposed of exactly once (`wiki/spec/language/ownership.md`,
//! "Construction and disposal order").
//!
//! `count` builds `lease` at the head of each iteration. The `n == 0` arm
//! does not use it, so it dies on the guard edge into that arm; the other
//! two arms read `lease.mark` and then jump back, so it dies on each back
//! edge, and not on the guard edges that lead to them.

#[path = "support/source_handoff.rs"]
mod program;

use program::Program;
use semantic_vocabulary::BlockId;
use terminal_check_tables::{Construct, RefusalSite};
use terminal_psi::{SuccessorEdge, TerminalAffineCleanupAction, TerminalMachine, Terminator};

const COUNT: &str = "data Lease dropped by finish { mark: u64; } \
    machine finish(l: Lease) { let Lease { mark as _ } = l; } \
    machine leaf(n: u64) -> u64 { n } \
    machine count(n: u64, m: u64) -> u64 terminates by n { \
        let lease: Lease = Lease { mark: 0 }; \
        transition { \
            n == 0 -> leaf(n), \
            n <= m -> count(n - 1, m +% lease.mark), \
            _ -> count(m, n +% lease.mark), \
        } \
    }";

/// The block that starts each iteration: the target of the back edges,
/// whose parameters are `n` and `m`.
fn head(machine: &TerminalMachine) -> BlockId {
    let found: Vec<_> = machine
        .blocks
        .iter()
        .filter(|block| block.parameters.len() == 2)
        .map(|block| block.id)
        .collect();
    let [head] = found.as_slice() else {
        panic!("one iteration head: {found:?}")
    };
    *head
}

/// The cleanup actions on each edge of `machine` that carries any, as
/// `(from, to, count)` in block order.
fn disposals(machine: &TerminalMachine) -> Vec<(BlockId, BlockId, usize)> {
    let mut found = Vec::new();
    for block in &machine.blocks {
        let mut edge = |target: BlockId, actions: &[TerminalAffineCleanupAction]| {
            if !actions.is_empty() {
                found.push((block.id, target, actions.len()));
            }
        };
        match &block.terminator {
            Terminator::Jump {
                target,
                cleanup_actions,
                ..
            } => edge(*target, cleanup_actions),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                edge(when_true.target, &when_true.cleanup_actions);
                edge(when_false.target, &when_false.cleanup_actions);
            }
            Terminator::Return {
                cleanup_actions, ..
            } => edge(block.id, cleanup_actions),
            _ => {}
        }
    }
    found
}

/// The back edges of the cycle: the jumps to `head` from inside the body.
fn back_edges(machine: &TerminalMachine) -> Vec<BlockId> {
    let head = head(machine);
    machine
        .blocks
        .iter()
        .filter(|block| {
            block.id.get() > head.get()
                && matches!(&block.terminator, Terminator::Jump { target, .. } if *target == head)
        })
        .map(|block| block.id)
        .collect()
}

/// The guard edge that leads to `arm`.
fn guard_into(machine: &mut TerminalMachine, arm: BlockId) -> &mut SuccessorEdge {
    machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|edge| edge.target == arm),
            _ => None,
        })
        .expect("a guard edge leads to the arm")
}

fn back_edge_cleanup(
    machine: &mut TerminalMachine,
    arm: BlockId,
) -> &mut Vec<TerminalAffineCleanupAction> {
    let block = machine
        .blocks
        .iter_mut()
        .find(|block| block.id == arm)
        .unwrap();
    let Terminator::Jump {
        cleanup_actions, ..
    } = &mut block.terminator
    else {
        unreachable!()
    };
    cleanup_actions
}

/// `lease` is disposed of once on each way out of an iteration: on the guard
/// edge into the `n == 0` arm and on each of the two back edges.
#[test]
fn each_way_out_of_an_iteration_disposes_of_the_local_once() {
    let program = Program::of(COUNT);
    let machine = program.machine("count");
    let backs = back_edges(machine);
    let [first, second] = backs[..] else {
        panic!("two back edges: {backs:?}")
    };
    let head = head(machine);
    let disposed = disposals(machine);
    assert_eq!(disposed.len(), 3, "{disposed:?}");
    assert!(disposed.contains(&(first, head, 1)), "{disposed:?}");
    assert!(disposed.contains(&(second, head, 1)), "{disposed:?}");
    program.judged(|_| {}).accepted();
}

/// Without the disposal on the first back edge, that iteration would leave
/// its `lease` behind when the next one builds another. The checker
/// computes its own disposal schedule and refuses a lowered program whose
/// edge carries fewer actions than that schedule, at that back edge.
#[test]
fn a_back_edge_that_does_not_dispose_of_the_local_is_refused() {
    let program = Program::of(COUNT);
    let first = back_edges(program.machine("count"))[0];
    let refusal = program
        .judged_in("count", |machine| {
            back_edge_cleanup(machine, first).clear();
        })
        .refused();
    assert_eq!(
        refusal.detail,
        "component cleanup exceeds its supplied action bound in machine `count`"
    );
    let edge = match &program
        .machine("count")
        .blocks
        .iter()
        .find(|block| block.id == first)
        .unwrap()
        .terminator
    {
        Terminator::Jump { edge, .. } => *edge,
        _ => unreachable!(),
    };
    assert!(
        matches!(refusal.site, RefusalSite::Machine(_, Construct::Edge(at)) if at == edge),
        "{refusal:?}"
    );
}

/// The first back edge's disposal copied onto the guard edge that leads to
/// it would dispose of `lease` twice on that path, and dispose of it before
/// the arm reads `lease.mark`. The checker refuses an edge disposal that
/// differs from its own schedule.
#[test]
fn a_back_edge_disposal_copied_onto_its_guard_edge_is_refused() {
    let program = Program::of(COUNT);
    let first = back_edges(program.machine("count"))[0];
    let refusal = program
        .judged_in("count", |machine| {
            let actions = back_edge_cleanup(machine, first).clone();
            guard_into(machine, first).cleanup_actions = actions;
        })
        .refused();
    assert_eq!(
        refusal.detail,
        "normal edge affine disposal differs from its canonical schedule in machine `count`"
    );
}
