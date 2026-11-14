/*
Control-flow graph, dominance, strongly connected block components, reducible
and irreducible loop-region classification, call-graph recursion components,
and shared analysis-root identity validation (`AnalysisRoot`,
`validate_analysis_root`, `validate_unit_identity`) for abstract-operation
units.
*/

use semantic_vocabulary::{BlockId, MachineId};
#[cfg(test)]
#[path = "control_flow_analyses_tests.rs"]
mod tests;
pub(crate) mod analysis_root {
    use optimization_core::OptimizationUnitIdentity;
    use optimization_unit::{PsiOptimizationUnit, recompute_psi_optimization_unit_identity};

    use crate::validation::ValidatedOptimizerCycleComponents;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AnalysisRoot {
        pub revision: OptimizationUnitIdentity,
        pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    }

    impl AnalysisRoot {
        pub(crate) const fn of(unit: &PsiOptimizationUnit) -> Self {
            Self {
                revision: unit.identity,
                terminal_psi: unit.psi,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum AnalysisRootError {
        StaleUnitIdentity {
            stored: OptimizationUnitIdentity,
            recomputed: OptimizationUnitIdentity,
        },
        TerminalIdentityMismatch,
        RevisionMismatch,
    }

    pub(crate) fn validate_unit_identity(
        unit: &PsiOptimizationUnit,
    ) -> Result<(), AnalysisRootError> {
        let recomputed = recompute_psi_optimization_unit_identity(unit);
        if recomputed != unit.identity {
            return Err(AnalysisRootError::StaleUnitIdentity {
                stored: unit.identity,
                recomputed,
            });
        }
        Ok(())
    }

    pub(crate) fn validate_analysis_root(
        unit: &PsiOptimizationUnit,
        custody: &ValidatedOptimizerCycleComponents,
        snapshots: &[AnalysisRoot],
    ) -> Result<(), AnalysisRootError> {
        validate_unit_identity(unit)?;
        if unit.psi != custody.terminal_psi()
            || unit.psi != custody.ranking_certificates().snapshot().terminal_psi
            || snapshots
                .iter()
                .any(|snapshot| snapshot.terminal_psi != unit.psi)
        {
            return Err(AnalysisRootError::TerminalIdentityMismatch);
        }
        if snapshots
            .iter()
            .any(|snapshot| snapshot.revision != unit.identity)
        {
            return Err(AnalysisRootError::RevisionMismatch);
        }
        Ok(())
    }
}
pub(crate) mod call_graph {
    use std::collections::BTreeMap;

    use optimization_unit::PsiOptimizationUnit;

    use super::CallGraphAnalysis;
    use crate::cycles::strongly_connected_components;

    pub(crate) fn call_graph(
        unit: &PsiOptimizationUnit,
    ) -> Result<CallGraphAnalysis, &'static str> {
        let callable_invocations =
            optimization_unit_semantics::callable_values::optimized_callable_invocations(unit)?;
        let mut unresolved_calls = Vec::new();
        let mut graph = unit
            .functions
            .iter()
            .map(|function| (function.machine, Vec::new()))
            .collect::<BTreeMap<_, _>>();
        for function in &unit.functions {
            let callees = graph.get_mut(&function.machine).unwrap();
            for operation in function.block_nodes().map(|node| &node.operation) {
                callees.extend(
                    operation
                        .cleanup_rosters()
                        .flat_map(|(_, actions)| actions)
                        .filter_map(|action| action.invoked_machine()),
                );
                for reference in unit.call_machine_references(operation) {
                    match reference {
                        optimization_unit::CallMachineReference::Declaration(machine) => {
                            callees.push(machine)
                        }
                        optimization_unit::CallMachineReference::Callable { callee, contract } => {
                            let operation = operation.psi_operation().ok_or(
                                "call graph runtime invocation lost its operation identity",
                            )?;
                            let row = callable_invocations
                                .as_ref()
                                .map(|summary| summary.operation(function.machine, operation))
                                .transpose()?
                                .flatten();
                            if let Some(row) = row {
                                callees.extend(row.selected_machines.iter().copied());
                            }
                            if row.is_none_or(|row| !row.targets_are_resolved()) {
                                unresolved_calls.push((
                                    function.machine,
                                    operation,
                                    callee,
                                    contract,
                                ));
                            }
                        }
                    }
                }
            }
            callees.sort_unstable();
            callees.dedup();
        }
        let components = strongly_connected_components(&graph);
        let recursive_components = components
            .iter()
            .filter(|component| {
                component.len() > 1
                    || component
                        .first()
                        .is_some_and(|machine| graph[machine].contains(machine))
            })
            .cloned()
            .collect();
        Ok(CallGraphAnalysis {
            callees: graph.into_iter().collect(),
            components,
            recursive_components,
            unresolved_calls,
        })
    }
}
pub(crate) mod components {
    use std::collections::BTreeSet;

    use optimization_unit::PsiOptimizationUnit;
    use semantic_vocabulary::MachineId;

    use super::{StronglyConnectedComponentAnalysis, graph::control_flow};
    use crate::cycles::strongly_connected_components;

    impl StronglyConnectedComponentAnalysis {
        pub(crate) fn cyclic_machines(&self, unit: &PsiOptimizationUnit) -> BTreeSet<MachineId> {
            unit.functions
                .iter()
                .filter(|function| {
                    let Some((_, components)) = self.function(function.machine) else {
                        return false;
                    };
                    components.iter().any(|component| {
                        component.len() > 1
                            || component.iter().any(|block| {
                                function.block(*block).is_some_and(|owner| {
                                    owner.successors().any(|edge| edge.target == *block)
                                })
                            })
                    })
                })
                .map(|function| function.machine)
                .collect()
        }
    }

    pub(crate) fn block_components(
        unit: &PsiOptimizationUnit,
    ) -> StronglyConnectedComponentAnalysis {
        let cfg = control_flow(unit);
        StronglyConnectedComponentAnalysis {
            functions: cfg
                .functions
                .iter()
                .map(|function| {
                    let graph = function
                        .blocks
                        .iter()
                        .map(|block| (block.block, block.successors.clone()))
                        .collect();
                    (function.machine, strongly_connected_components(&graph))
                })
                .collect(),
        }
    }
}
pub(crate) use crate::countdown_analyses::countdown_induction;
pub(crate) use crate::countdown_analyses::countdown_invariant_constant_placement;
pub(crate) use crate::countdown_analyses::countdown_invariant_constants;
pub(crate) mod dominance {
    use std::collections::{BTreeMap, BTreeSet};

    use optimization_unit::PsiOptimizationUnit;
    use semantic_vocabulary::BlockId;

    use super::{DominatorAnalysis, FunctionControlFlow, by_key, graph::control_flow};

    pub(crate) fn block_dominates(
        dominators: &[(BlockId, Vec<BlockId>)],
        dominator: BlockId,
        block: BlockId,
    ) -> bool {
        dominators
            .binary_search_by_key(&block, |(candidate, _)| *candidate)
            .is_ok_and(|index| dominators[index].1.binary_search(&dominator).is_ok())
    }

    pub(crate) fn dominators(unit: &PsiOptimizationUnit, reverse: bool) -> DominatorAnalysis {
        let cfg = control_flow(unit);
        DominatorAnalysis {
            functions: cfg
                .functions
                .iter()
                .map(|function| {
                    (
                        function.machine,
                        fixed_point_dominators(function, reverse)
                            .into_iter()
                            .map(|(block, set)| (block, set.into_iter().collect()))
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn fixed_point_dominators(
        function: &FunctionControlFlow,
        reverse: bool,
    ) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
        let rows = by_key(&function.blocks, |block| block.block);
        let considered = function
            .blocks
            .iter()
            .filter(|block| block.reachable)
            .map(|block| block.block)
            .collect::<BTreeSet<_>>();
        let roots = if reverse {
            function
                .blocks
                .iter()
                .filter(|block| block.reachable && !block.exits.is_empty())
                .map(|block| block.block)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::from([function.entry])
        };
        let mut result = considered
            .iter()
            .copied()
            .map(|block| {
                let initial = if roots.contains(&block) {
                    BTreeSet::from([block])
                } else {
                    considered.clone()
                };
                (block, initial)
            })
            .collect::<BTreeMap<_, _>>();
        loop {
            let mut changed = false;
            for block in considered
                .iter()
                .copied()
                .filter(|block| !roots.contains(block))
            {
                let adjacent = if reverse {
                    &rows[&block].successors
                } else {
                    &rows[&block].predecessors
                };
                let mut incoming = adjacent.iter().filter_map(|other| result.get(other));
                let mut next = incoming.next().cloned().unwrap_or_default();
                for set in incoming {
                    next = next.intersection(set).copied().collect();
                }
                next.insert(block);
                if result[&block] != next {
                    result.insert(block, next);
                    changed = true;
                }
            }
            if !changed {
                return result;
            }
        }
    }
}
pub(crate) mod graph {
    use std::collections::{BTreeMap, BTreeSet};

    use abstract_operations::AbstractOperation as O;
    use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};

    use super::{BlockControlFlow, ControlFlowAnalysis, ExitKind, FunctionControlFlow};

    pub(crate) fn control_flow(unit: &PsiOptimizationUnit) -> ControlFlowAnalysis {
        ControlFlowAnalysis {
            functions: unit.functions.iter().map(function_control_flow).collect(),
        }
    }

    fn function_control_flow(function: &PsiOptimizationFunction) -> FunctionControlFlow {
        // The representation owns edge construction, including case arms and
        // their bindings. Analyses must not infer a smaller graph from opcodes.
        let mut successors = function
            .blocks
            .iter()
            .map(|block| {
                (
                    block.id,
                    block
                        .successors()
                        .map(|edge| edge.target)
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut predecessors = function
            .blocks
            .iter()
            .map(|block| (block.id, Vec::new()))
            .collect::<BTreeMap<_, _>>();
        let mut exits = function
            .blocks
            .iter()
            .map(|block| {
                let exits = match block.nodes.last().map(|node| &node.operation) {
                    Some(
                        abstract_operations::return_or_return_unit_pattern!()
                        | O::ReturnStructural { .. },
                    ) => vec![ExitKind::Normal],
                    Some(O::Crash { .. }) => vec![ExitKind::Crash],
                    _ => Vec::new(),
                };
                (block.id, exits)
            })
            .collect::<BTreeMap<_, _>>();
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
}
pub(crate) mod loops {
    use std::collections::{BTreeMap, BTreeSet};

    use optimization_unit::PsiOptimizationUnit;

    use super::{LoopAnalysis, LoopRegion, dominance::fixed_point_dominators, graph::control_flow};
    use crate::cycles::strongly_connected_components;

    pub(crate) fn loops(unit: &PsiOptimizationUnit) -> LoopAnalysis {
        let cfg = control_flow(unit);
        LoopAnalysis {
            functions: cfg
                .functions
                .iter()
                .map(|function| {
                    let graph = function
                        .blocks
                        .iter()
                        .map(|block| (block.block, block.successors.clone()))
                        .collect::<BTreeMap<_, _>>();
                    let predecessors = function
                        .blocks
                        .iter()
                        .map(|block| (block.block, block.predecessors.clone()))
                        .collect::<BTreeMap<_, _>>();
                    let dominators = fixed_point_dominators(function, false);
                    let regions = strongly_connected_components(&graph)
                        .into_iter()
                        .filter(|component| {
                            component.len() > 1
                                || component
                                    .first()
                                    .is_some_and(|block| graph[block].contains(block))
                        })
                        .map(|component| {
                            let members = component.iter().copied().collect::<BTreeSet<_>>();
                            let entries = component
                                .iter()
                                .copied()
                                .filter(|block| {
                                    predecessors[block]
                                        .iter()
                                        .any(|predecessor| !members.contains(predecessor))
                                })
                                .collect::<Vec<_>>();
                            let header = component.iter().copied().find(|candidate| {
                                component.iter().all(|block| {
                                    dominators
                                        .get(block)
                                        .is_some_and(|set| set.contains(candidate))
                                })
                            });
                            LoopRegion {
                                header,
                                blocks: component,
                                irreducible: entries.len() > 1 || header.is_none(),
                            }
                        })
                        .collect();
                    (function.machine, regions)
                })
                .collect(),
        }
    }
}
pub(crate) use analysis_root::AnalysisRootError;
pub(crate) use analysis_root::{AnalysisRoot, validate_analysis_root, validate_unit_identity};
pub(crate) use call_graph::call_graph;
pub(crate) use components::block_components;

pub(crate) use countdown_induction::analyze_counted_loops;
pub(crate) use countdown_induction::{
    CountedLoopAnalysisError, UnsignedCountdownLoopSummary, ValidatedCountedLoopAnalysis,
};

pub(crate) use countdown_invariant_constant_placement::analyze_countdown_invariant_constant_placement;
pub(crate) use countdown_invariant_constant_placement::{
    CountdownInvariantConstantPlacementAnalysisError,
    ValidatedCountdownInvariantConstantPlacementAnalysis,
};

pub(crate) use countdown_invariant_constants::analyze_countdown_invariant_constants;
pub(crate) use countdown_invariant_constants::validate_moved_constant_locations;
pub(crate) use countdown_invariant_constants::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, ValidatedCountdownInvariantConstantAnalysis,
    expected_constant, invariant_constant_row, sole_provenance_node,
};
pub(crate) use dominance::block_dominates;
pub(crate) use dominance::dominators;
pub(crate) use graph::control_flow;
pub(crate) use loops::loops;

pub(crate) fn by_key<K: Ord, V>(
    rows: &[V],
    key: impl Fn(&V) -> K,
) -> std::collections::BTreeMap<K, &V> {
    rows.iter().map(|row| (key(row), row)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ExitKind {
    Normal,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BlockControlFlow {
    pub block: BlockId,
    pub predecessors: Vec<BlockId>,
    pub successors: Vec<BlockId>,
    pub exits: Vec<ExitKind>,
    pub reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionControlFlow {
    pub machine: MachineId,
    pub entry: BlockId,
    pub blocks: Vec<BlockControlFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ControlFlowAnalysis {
    pub functions: Vec<FunctionControlFlow>,
}

pub(crate) type BlockDominators = (BlockId, Vec<BlockId>);
pub(crate) type FunctionDominators = (MachineId, Vec<BlockDominators>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DominatorAnalysis {
    pub functions: Vec<FunctionDominators>,
}

fn machine_row<T>(rows: &[(MachineId, T)], machine: MachineId) -> Option<&(MachineId, T)> {
    rows.binary_search_by_key(&machine, |(candidate, _)| *candidate)
        .ok()
        .map(|index| &rows[index])
}

impl DominatorAnalysis {
    pub fn function(&self, machine: MachineId) -> Option<&FunctionDominators> {
        machine_row(&self.functions, machine)
    }

    pub fn rows(&self, machine: MachineId) -> &[BlockDominators] {
        self.function(machine)
            .map_or(&[], |(_, rows)| rows.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StronglyConnectedComponentAnalysis {
    pub functions: Vec<(MachineId, Vec<Vec<BlockId>>)>,
}

impl StronglyConnectedComponentAnalysis {
    pub fn function(&self, machine: MachineId) -> Option<&(MachineId, Vec<Vec<BlockId>>)> {
        machine_row(&self.functions, machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopRegion {
    pub header: Option<BlockId>,
    pub blocks: Vec<BlockId>,
    pub irreducible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoopAnalysis {
    pub functions: Vec<(MachineId, Vec<LoopRegion>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallGraphAnalysis {
    pub callees: Vec<(MachineId, Vec<MachineId>)>,
    pub components: Vec<Vec<MachineId>>,
    pub recursive_components: Vec<Vec<MachineId>>,
    pub unresolved_calls: Vec<(
        MachineId,
        semantic_vocabulary::OperationId,
        semantic_vocabulary::ValueId,
        semantic_vocabulary::CallableContractId,
    )>,
}
