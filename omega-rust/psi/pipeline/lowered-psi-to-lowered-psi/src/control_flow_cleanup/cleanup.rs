//! Private control-flow cleanup rewrite.
//!
//! A conditional whose condition is a `BooleanConstant` result folds to the
//! selected successor edge as an unconditional `Jump`. Blocks the untaken arm
//! leaves unreachable are removed only when they carry nothing evidence
//! retains; a refusal leaves the conditional exactly as authored. Ranked
//! machines keep their ranking evidence over exact execution positions and
//! are never rewritten here.

use semantic_vocabulary::{BlockId, OperationId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{Block, OperationKind, OperationResult, TerminalMachine, Terminator};
use terminal_verifier::BlockLocalEvidence;

pub(super) fn cleanup(
    machine: &mut TerminalMachine,
    evidence: &BlockLocalEvidence,
    sidecar_operations: &BTreeSet<OperationId>,
    sidecar_values: &BTreeSet<ValueId>,
) {
    if machine.ranked_scc.is_some() {
        return;
    }
    // Literal conditions come from `BooleanConstant` operations only. Folded
    // leaves arrive here already rewritten by the constant-propagation rule;
    // no other binding form makes a branch trivially conditional.
    let mut literals = BTreeMap::new();
    for block in &machine.blocks {
        for operation in &block.operations {
            if let OperationKind::BooleanConstant { value } = &operation.kind
                && let Some(result) = operation.result.scalar()
            {
                literals.insert(result.id, *value);
            }
        }
    }
    let mut position = 0;
    while position < machine.blocks.len() {
        let (target, folded) = {
            let Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } = &machine.blocks[position].terminator
            else {
                position += 1;
                continue;
            };
            let Some(selected) = literals.get(condition).copied() else {
                position += 1;
                continue;
            };
            let (taken, untaken) = if selected {
                (when_true, when_false)
            } else {
                (when_false, when_true)
            };
            // Dropping the untaken edge identity would orphan any evidence row
            // naming it; the conditional then carries more than a choice.
            if evidence.edges.contains(&untaken.edge) {
                position += 1;
                continue;
            }
            (
                taken.target,
                Terminator::Jump {
                    edge: taken.edge,
                    target: taken.target,
                    arguments: taken.arguments.clone(),
                    erased_arguments: Vec::new(),
                    structural_arguments: taken.structural_arguments.clone(),
                    trivial_affine_discards: taken.trivial_affine_discards.clone(),
                    residual_affine_discards: Vec::new(),
                },
            )
        };
        let stranded = stranded_blocks(machine, position, target);
        if stranded
            .iter()
            .all(|id| removable(machine, *id, evidence, sidecar_operations, sidecar_values))
        {
            machine.blocks[position].terminator = folded;
            machine.blocks.retain(|block| !stranded.contains(&block.id));
        }
        position += 1;
    }
}

/// Blocks a fold at `position` to `target` would leave unreachable from the
/// machine entry under the current, already partially folded graph.
fn stranded_blocks(
    machine: &TerminalMachine,
    position: usize,
    target: BlockId,
) -> BTreeSet<BlockId> {
    let positions = machine
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.id, index))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::from([machine.entry]);
    let mut pending = vec![machine.entry];
    while let Some(id) = pending.pop() {
        let Some(&index) = positions.get(&id) else {
            continue;
        };
        let targets: Vec<BlockId> = if index == position {
            vec![target]
        } else {
            match &machine.blocks[index].terminator {
                Terminator::Jump { target, .. } => vec![*target],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.target, when_false.target],
                Terminator::StructuralCase { cases, .. } => {
                    cases.iter().map(|case| case.target).collect()
                }
                _ => Vec::new(),
            }
        };
        for target in targets {
            if reachable.insert(target) {
                pending.push(target);
            }
        }
    }
    machine
        .blocks
        .iter()
        .filter(|block| !reachable.contains(&block.id))
        .map(|block| block.id)
        .collect()
}

/// Whether dropping `block` preserves every retained identity: no
/// evidence-bound row, no static reach binding, and no structural result or
/// parameter whose machine-level place declaration would orphan.
fn removable(
    machine: &TerminalMachine,
    block_id: BlockId,
    evidence: &BlockLocalEvidence,
    sidecar_operations: &BTreeSet<OperationId>,
    sidecar_values: &BTreeSet<ValueId>,
) -> bool {
    let Some(block) = machine.blocks.iter().find(|block| block.id == block_id) else {
        return false;
    };
    removable_block(block, evidence, sidecar_operations, sidecar_values)
}

fn removable_block(
    block: &Block,
    evidence: &BlockLocalEvidence,
    sidecar_operations: &BTreeSet<OperationId>,
    sidecar_values: &BTreeSet<ValueId>,
) -> bool {
    if evidence.blocks.contains(&block.id) || !block.structural_parameters.is_empty() {
        return false;
    }
    for operation in &block.operations {
        if operation.static_reach_binding.is_some()
            || evidence.operations.contains(&operation.id)
            || sidecar_operations.contains(&operation.id)
            || matches!(operation.result, OperationResult::Structural(_))
        {
            return false;
        }
        if let Some(result) = operation.result.scalar()
            && (evidence.values.contains(&result.id) || sidecar_values.contains(&result.id))
        {
            return false;
        }
    }
    if block.parameters.iter().any(|parameter| {
        evidence.values.contains(&parameter.id) || sidecar_values.contains(&parameter.id)
    }) {
        return false;
    }
    !block
        .terminator
        .edges()
        .any(|edge| evidence.edges.contains(&edge))
}
