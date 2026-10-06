//! Carry each authored state's exclusive record loans into the private blocks
//! the scalar graph splits that state into.
//!
//! A non-entry state binds its explicit `&mut` record formal as a block
//! parameter of its source block. Terminal treats that parameter as an
//! edge-scoped name: it lives only while an edge carries it onward. The scalar
//! graph evaluates a state's exit arguments, guard chains, call continuations
//! and computation rows in private blocks beneath the source block, and those
//! blocks read the formal by its source-block name. When some edge already
//! transfers the formal to another state, Terminal keeps the name available
//! until that transfer consumes it. When none does — the state is the last to
//! use the loan, as in `third(main, index) { transition .. { true ->
//! (main.table[2]) _ -> (main.table[3]) } }` — every private read is a use
//! after the name ended, and verification rightly refuses it.
//!
//! The Unit state graph solved the same split with
//! `crate::emission::private_borrow_transport`: each private block that reads
//! the loan, or leads to a block that does, receives it as its own structural
//! parameter and reads it by that fresh name. This pass applies that same rule
//! to the scalar graph, once every block is emitted and the authored formals
//! are bound, so every producer of private blocks shares one transfer rule.
//!
//! A state's private region is every block its source block reaches without
//! entering another authored state's block. A private block reached from two
//! regions — the shared return completion is one — cannot receive one state's
//! loan on the other state's edges, so a region that would forward a loan into
//! such a block refuses instead of inventing an argument its other arrivals
//! lack.

use super::{Block, BlockId, LoweringError, Terminator, scalar_source_block, unsupported};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralTypeDeclaration;

pub(super) fn forward_private_state_borrows(
    blocks: &mut [Block],
    identity_base: u64,
    authored_states: usize,
    structural_types: &[StructuralTypeDeclaration],
    next_place: &mut u64,
) -> Result<(), LoweringError> {
    let roots = (0..authored_states)
        .map(|position| scalar_source_block(identity_base, position))
        .collect::<BTreeSet<_>>();
    let positions = blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id, position))
        .collect::<BTreeMap<_, _>>();
    let regions = roots
        .iter()
        .map(|root| (*root, private_region(*root, blocks, &positions, &roots)))
        .collect::<Vec<_>>();
    let mut arrivals = BTreeMap::<BlockId, usize>::new();
    for (_, region) in &regions {
        for block in region {
            *arrivals.entry(*block).or_default() += 1;
        }
    }
    for (root, region) in regions {
        let Some(&root_position) = positions.get(&root) else {
            continue;
        };
        if blocks[root_position].structural_parameters.is_empty() || region.is_empty() {
            continue;
        }
        let members = std::iter::once(root_position)
            .chain(region.iter().map(|block| positions[block]))
            .collect::<Vec<_>>();
        let mut state_blocks = members
            .iter()
            .map(|position| blocks[*position].clone())
            .collect::<Vec<_>>();
        // The module derives every block-parameter place declaration from the
        // final block rosters, so the transfer's own declaration list is not
        // needed here.
        let mut declarations = Vec::new();
        crate::emission::private_borrow_transport::forward_private_state_borrows(
            root,
            &mut state_blocks,
            structural_types,
            next_place,
            &mut declarations,
        )?;
        for (position, forwarded) in members.into_iter().zip(state_blocks) {
            let gained = forwarded.structural_parameters.len()
                != blocks[position].structural_parameters.len();
            if gained && arrivals.get(&forwarded.id).is_some_and(|count| *count > 1) {
                return unsupported(
                    "scalar graph private block shared by several states cannot receive one state's exclusive loan",
                );
            }
            blocks[position] = forwarded;
        }
    }
    Ok(())
}

/// Every block `root` reaches without entering an authored state's block,
/// the root itself excluded.
fn private_region(
    root: BlockId,
    blocks: &[Block],
    positions: &BTreeMap<BlockId, usize>,
    roots: &BTreeSet<BlockId>,
) -> BTreeSet<BlockId> {
    let mut region = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(current) = pending.pop() {
        let Some(&position) = positions.get(&current) else {
            continue;
        };
        for target in successors(&blocks[position].terminator) {
            if !roots.contains(&target) && region.insert(target) {
                pending.push(target);
            }
        }
    }
    region
}

fn successors(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Jump { target, .. } => vec![*target],
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => vec![when_true.target, when_false.target],
        Terminator::StructuralCase { cases, .. } => cases.iter().map(|case| case.target).collect(),
        Terminator::Return { .. }
        | Terminator::ReturnUnit { .. }
        | Terminator::ReturnStructural { .. }
        | Terminator::Crash { .. } => Vec::new(),
    }
}
