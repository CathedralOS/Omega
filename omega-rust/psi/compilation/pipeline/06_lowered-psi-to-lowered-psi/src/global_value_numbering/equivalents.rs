//! Duplicate removal across dominator-equivalent scalar computations.
//!
//! Scanning in reverse postorder meets every dominating definition before the
//! blocks it dominates, so a single pass suffices: each eligible operation's
//! operands are first resolved through duplicates already collapsed to their
//! canonical survivors, then matched against the surviving operations seen so
//! far. The canonical survivor is the first match in scan order, and a
//! duplicate's uses substitute the surviving result once — survivors are never
//! remapped, so the substitution is never transitive.
//!
//! Blocks unreachable from the entry never enter the scan order and are left
//! untouched; module validation already rejects them on the public entrance.

use crate::retained_identities::proof_values::{
    call_erased_arguments, crash_continuations, retain_crash_routes, retain_erased_arguments,
    retain_proposition,
};
use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{OperationKind, TerminalMachine, Terminator, ValueDeclaration};

/// One surviving computation later duplicates may equal: the operation kind
/// with operands resolved through already-collapsed duplicates, the block that
/// must dominate a candidate, and the result declaration that must match.
struct Leader {
    kind: OperationKind,
    block: BlockId,
    result: ValueDeclaration,
}

pub(super) fn deduplicate(
    machine: &mut TerminalMachine,
    source_calls: &[lowered_psi::LoweredSourceCallOccurrence],
    retained_values: &BTreeSet<ValueId>,
) {
    // Ranking evidence names exact value identities over the covered cyclic
    // components: it is proof and termination custody, not a use list the
    // substitution can rewrite. Covered member blocks keep their exact
    // contents; duplicates outside them still collapse.
    let coverage = crate::retained_identities::ranked_coverage::ranked_coverage(machine);
    let mut retained_values = retained_values.clone();
    retained_values.extend(coverage.values.iter().copied());
    for proposition in &machine.contract.requires {
        retain_proposition(proposition, &mut retained_values);
    }
    for clause in &machine.contract.ensures {
        retain_proposition(&clause.proposition, &mut retained_values);
    }
    for guarantee in &machine.contract.outcome_specific_ensures {
        retain_proposition(&guarantee.proposition, &mut retained_values);
    }
    retain_crash_routes(&machine.contract.crash_routes, &mut retained_values);
    let operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    for operation in &operations {
        retain_crash_routes(crash_continuations(&operation.kind), &mut retained_values);
        retain_erased_arguments(call_erased_arguments(&operation.kind), &mut retained_values);
    }
    for block in &machine.blocks {
        if let Terminator::Crash { site_guard, .. } = &block.terminator {
            for term in site_guard {
                retain_proposition(term.proposition(), &mut retained_values);
            }
        }
    }
    for occurrence in source_calls {
        if crate::retained_identities::contains_source_call(machine, occurrence) {
            retained_values.extend(
                occurrence
                    .source_values_before_call
                    .iter()
                    .map(|value| value.id),
            );
        }
    }
    let order = reverse_postorder(machine);
    let dominators = dominators(machine);
    let mut representative: BTreeMap<ValueId, ValueId> = BTreeMap::new();
    let mut removed = BTreeSet::new();
    let mut leaders: Vec<Leader> = Vec::new();
    for block_id in order {
        let Some(block) = machine.blocks.iter().find(|block| block.id == block_id) else {
            continue;
        };
        for operation in &block.operations {
            let Some(result) = operation.result.scalar() else {
                continue;
            };
            // A static reach binder position is semantic call evidence carried
            // by the operation row; removal would orphan it.
            if operation.static_reach_binding.is_some()
                || !terminal_semantics::is_unconditionally_total_scalar(&operation.kind)
            {
                continue;
            }
            let mut kind = operation.kind.clone();
            kind.map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
            // A value named by proof or custody sidecars keeps its own
            // identity; it may still canonicalize a later duplicate. A
            // covered member's operation stays exact, and a result used
            // inside a member block is never substituted away.
            if !retained_values.contains(&result.id)
                && !coverage.blocks.contains(&block_id)
                && !coverage.member_uses.contains(&result.id)
                && let Some(leader) = leaders.iter().find(|leader| {
                    leader.kind == kind
                        && dominators
                            .get(&block_id)
                            .is_some_and(|blocks| blocks.contains(&leader.block))
                        && leader.result.scalar_type == result.scalar_type
                        && leader.result.qualifications == result.qualifications
                })
            {
                representative.insert(result.id, leader.result.id);
                removed.insert(operation.id);
                continue;
            }
            leaders.push(Leader {
                kind,
                block: block_id,
                result,
            });
        }
    }
    if removed.is_empty() {
        return;
    }
    for block in &mut machine.blocks {
        block
            .operations
            .retain(|operation| !removed.contains(&operation.id));
        for operation in &mut block.operations {
            operation
                .kind
                .map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
        }
        block
            .terminator
            .map_scalar_uses(&mut |value| representative.get(&value).copied().unwrap_or(value));
    }
}

/// Reachable blocks in reverse postorder: every dominator precedes each block
/// it dominates, so the leader scan meets a dominating definition first.
fn reverse_postorder(machine: &TerminalMachine) -> Vec<BlockId> {
    let successors = machine
        .blocks
        .iter()
        .map(|block| {
            let targets = match &block.terminator {
                Terminator::Jump { target, .. } => vec![*target],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.target, when_false.target],
                Terminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| case.target).collect()
                }
                Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::Crash { .. }
                | Terminator::ExternalCompletion { .. } => Vec::new(),
            };
            (block.id, targets)
        })
        .collect::<BTreeMap<_, _>>();
    let mut entered = BTreeSet::from([machine.entry]);
    let mut finished = Vec::new();
    let mut pending = vec![(machine.entry, 0usize)];
    while let Some((block, position)) = pending.last_mut() {
        let Some(target) = successors
            .get(block)
            .and_then(|targets| targets.get(*position))
            .copied()
        else {
            finished.push(*block);
            pending.pop();
            continue;
        };
        *position += 1;
        if entered.insert(target) {
            pending.push((target, 0));
        }
    }
    finished.reverse();
    finished
}

/// Dominator sets over the reachable graph: a block dominates another only
/// when every path from the entry passes through it.
fn dominators(machine: &TerminalMachine) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let blocks = machine
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut predecessors = blocks
        .iter()
        .map(|block| (*block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &machine.blocks {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. }
            | Terminator::ExternalCompletion { .. } => Vec::new(),
        };
        for target in targets {
            if let Some(incoming) = predecessors.get_mut(&target) {
                incoming.insert(block.id);
            }
        }
    }
    let mut dominators = blocks
        .iter()
        .map(|block| {
            (
                *block,
                if *block == machine.entry {
                    BTreeSet::from([*block])
                } else {
                    blocks.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in blocks
            .iter()
            .copied()
            .filter(|block| *block != machine.entry)
        {
            let Some(incoming) = predecessors.get(&block) else {
                continue;
            };
            let Some(first) = incoming.iter().next() else {
                continue;
            };
            let mut common = dominators
                .get(first)
                .cloned()
                .unwrap_or_else(|| BTreeSet::from([*first]));
            for predecessor in incoming.iter().skip(1) {
                let predecessor_dominators = dominators
                    .get(predecessor)
                    .cloned()
                    .unwrap_or_else(|| BTreeSet::from([*predecessor]));
                common.retain(|candidate| predecessor_dominators.contains(candidate));
            }
            common.insert(block);
            if dominators.get(&block) != Some(&common) {
                dominators.insert(block, common);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}
