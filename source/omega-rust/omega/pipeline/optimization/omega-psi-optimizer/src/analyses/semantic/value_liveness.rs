use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::PsiOptimizationUnit;
use psi_core::{BlockId, MachineId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeLiveness {
    pub node: u32,
    pub entry: Vec<ValueId>,
    pub exit: Vec<ValueId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueLivenessBlock {
    pub machine: MachineId,
    pub block: BlockId,
    pub entry: Vec<ValueId>,
    pub exit: Vec<ValueId>,
    pub nodes: Vec<NodeLiveness>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueLivenessAnalysis {
    pub blocks: Vec<ValueLivenessBlock>,
}

pub(in crate::analyses) fn value_liveness(unit: &PsiOptimizationUnit) -> ValueLivenessAnalysis {
    let mut output = Vec::new();
    for function in &unit.functions {
        let successors = function
            .blocks
            .iter()
            .map(|block| {
                let targets = block
                    .nodes
                    .last()
                    .map(|node| match &node.operation {
                        O::Jump { target, .. } => vec![*target],
                        O::Conditional {
                            when_true,
                            when_false,
                            ..
                        } => vec![when_true.target, when_false.target],
                        _ => Vec::new(),
                    })
                    .unwrap_or_default();
                (block.id, targets)
            })
            .collect::<BTreeMap<_, _>>();
        let mut live_entry = function
            .blocks
            .iter()
            .map(|block| (block.id, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        let mut live_exit = live_entry.clone();
        loop {
            let mut changed = false;
            for block in function.blocks.iter().rev() {
                let next_exit = successors[&block.id]
                    .iter()
                    .filter_map(|successor| live_entry.get(successor))
                    .flat_map(|set| set.iter().copied())
                    .collect::<BTreeSet<_>>();
                let mut next_entry = next_exit.clone();
                for node in block.nodes.iter().rev() {
                    for definition in &node.definitions {
                        next_entry.remove(&definition.value);
                    }
                    next_entry.extend(node.uses.iter().map(|use_site| use_site.value));
                }
                for parameter in &block.parameters {
                    next_entry.remove(&parameter.value);
                }
                if live_exit[&block.id] != next_exit {
                    live_exit.insert(block.id, next_exit);
                    changed = true;
                }
                if live_entry[&block.id] != next_entry {
                    live_entry.insert(block.id, next_entry);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for block in &function.blocks {
            let mut live = live_exit[&block.id].clone();
            let mut rows = Vec::with_capacity(block.nodes.len());
            for (node_index, node) in block.nodes.iter().enumerate().rev() {
                let exit = live.iter().copied().collect();
                for definition in &node.definitions {
                    live.remove(&definition.value);
                }
                live.extend(node.uses.iter().map(|use_site| use_site.value));
                rows.push(NodeLiveness {
                    node: u32::try_from(node_index).expect("optimization node index is u32"),
                    entry: live.iter().copied().collect(),
                    exit,
                });
            }
            rows.reverse();
            output.push(ValueLivenessBlock {
                machine: function.machine,
                block: block.id,
                entry: live_entry[&block.id].iter().copied().collect(),
                exit: live_exit[&block.id].iter().copied().collect(),
                nodes: rows,
            });
        }
    }
    ValueLivenessAnalysis { blocks: output }
}
