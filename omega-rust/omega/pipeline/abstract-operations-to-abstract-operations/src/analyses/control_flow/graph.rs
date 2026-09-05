//! Reachability and canonical predecessor/successor construction.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperation as O;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};

use super::{BlockControlFlow, ControlFlowAnalysis, ExitKind, FunctionControlFlow};

pub(in crate::analyses) fn control_flow(unit: &PsiOptimizationUnit) -> ControlFlowAnalysis {
    ControlFlowAnalysis {
        functions: unit.functions.iter().map(function_control_flow).collect(),
    }
}

fn function_control_flow(function: &PsiOptimizationFunction) -> FunctionControlFlow {
    let mut successors = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut predecessors = successors.clone();
    let mut exits = function
        .blocks
        .iter()
        .map(|block| (block.id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &function.blocks {
        if let Some(operation) = block.nodes.last().map(|node| &node.operation) {
            match operation {
                O::Jump { target, .. } => successors.get_mut(&block.id).unwrap().push(*target),
                O::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    successors
                        .get_mut(&block.id)
                        .unwrap()
                        .extend([when_true.target, when_false.target]);
                }
                O::Return { .. } | O::ReturnUnit { .. } | O::ReturnStructural { .. } => {
                    exits.get_mut(&block.id).unwrap().push(ExitKind::Normal);
                }
                O::Crash { .. } => exits.get_mut(&block.id).unwrap().push(ExitKind::Crash),
                _ => {}
            }
        }
    }
    for (source, targets) in &successors {
        for target in targets {
            if let Some(incoming) = predecessors.get_mut(target) {
                incoming.push(*source);
            }
        }
    }
    for values in successors.values_mut().chain(predecessors.values_mut()) {
        values.sort_unstable();
        values.dedup();
    }
    for values in exits.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    FunctionControlFlow {
        machine: function.machine,
        entry: function.entry,
        blocks: function
            .blocks
            .iter()
            .map(|block| BlockControlFlow {
                block: block.id,
                predecessors: predecessors.remove(&block.id).unwrap_or_default(),
                successors: successors.remove(&block.id).unwrap_or_default(),
                exits: exits.remove(&block.id).unwrap_or_default(),
                reachable: reachable.contains(&block.id),
            })
            .collect(),
    }
}
