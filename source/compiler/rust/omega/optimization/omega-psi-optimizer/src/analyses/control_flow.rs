use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{AnalysisKind, AnalysisSet};
use omega_optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use omega_terminal_abstract_operations::TerminalAbstractOperation as O;
use psi_core::{BlockId, MachineId};

use super::AnalysisProduct;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExitKind {
    Normal,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockControlFlow {
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
pub struct ControlFlowAnalysis {
    pub functions: Vec<FunctionControlFlow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominatorAnalysis {
    pub functions: Vec<(MachineId, Vec<(BlockId, Vec<BlockId>)>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StronglyConnectedComponentAnalysis {
    pub functions: Vec<(MachineId, Vec<Vec<BlockId>>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopRegion {
    /// Natural-loop header when one node dominates every entry. `None` marks
    /// an irreducible region with multiple entries.
    pub header: Option<BlockId>,
    pub blocks: Vec<BlockId>,
    pub irreducible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopAnalysis {
    pub functions: Vec<(MachineId, Vec<LoopRegion>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallGraphAnalysis {
    pub callees: Vec<(MachineId, Vec<MachineId>)>,
    pub components: Vec<Vec<MachineId>>,
    pub recursive_components: Vec<Vec<MachineId>>,
}

pub fn analysis_dependencies(kind: AnalysisKind) -> Option<AnalysisSet> {
    match kind {
        AnalysisKind::ControlFlowGraph | AnalysisKind::CallGraph => Some(AnalysisSet::default()),
        AnalysisKind::Dominators
        | AnalysisKind::PostDominators
        | AnalysisKind::StronglyConnectedComponents => {
            Some(AnalysisSet::new([AnalysisKind::ControlFlowGraph]))
        }
        AnalysisKind::LoopForest => Some(AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::StronglyConnectedComponents,
        ])),
        _ => None,
    }
}

pub fn compute_analysis(unit: &PsiOptimizationUnit, kind: AnalysisKind) -> Option<AnalysisProduct> {
    match kind {
        AnalysisKind::ControlFlowGraph => {
            Some(AnalysisProduct::ControlFlowGraph(control_flow(unit)))
        }
        AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators(unit, false))),
        AnalysisKind::PostDominators => {
            Some(AnalysisProduct::PostDominators(dominators(unit, true)))
        }
        AnalysisKind::StronglyConnectedComponents => Some(
            AnalysisProduct::StronglyConnectedComponents(block_components(unit)),
        ),
        AnalysisKind::LoopForest => Some(AnalysisProduct::LoopForest(loops(unit))),
        AnalysisKind::CallGraph => Some(AnalysisProduct::CallGraph(call_graph(unit))),
        _ => None,
    }
}

fn control_flow(unit: &PsiOptimizationUnit) -> ControlFlowAnalysis {
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

fn dominators(unit: &PsiOptimizationUnit, reverse: bool) -> DominatorAnalysis {
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

fn fixed_point_dominators(
    function: &FunctionControlFlow,
    reverse: bool,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let rows = function
        .blocks
        .iter()
        .map(|block| (block.block, block))
        .collect::<BTreeMap<_, _>>();
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

fn block_components(unit: &PsiOptimizationUnit) -> StronglyConnectedComponentAnalysis {
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

fn loops(unit: &PsiOptimizationUnit) -> LoopAnalysis {
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

fn call_graph(unit: &PsiOptimizationUnit) -> CallGraphAnalysis {
    let mut graph = unit
        .functions
        .iter()
        .map(|function| (function.machine, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for function in &unit.functions {
        let callees = graph.get_mut(&function.machine).unwrap();
        for operation in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
        {
            match operation {
                O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. }
                | O::Call { callee, .. } => callees.push(*callee),
                _ => {}
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
    CallGraphAnalysis {
        callees: graph.into_iter().collect(),
        components,
        recursive_components,
    }
}

fn strongly_connected_components<T>(graph: &BTreeMap<T, Vec<T>>) -> Vec<Vec<T>>
where
    T: Copy + Ord,
{
    struct Tarjan<T> {
        next: usize,
        indices: BTreeMap<T, usize>,
        lowlinks: BTreeMap<T, usize>,
        stack: Vec<T>,
        on_stack: BTreeSet<T>,
        components: Vec<Vec<T>>,
    }

    fn visit<T>(node: T, graph: &BTreeMap<T, Vec<T>>, state: &mut Tarjan<T>)
    where
        T: Copy + Ord,
    {
        let index = state.next;
        state.next += 1;
        state.indices.insert(node, index);
        state.lowlinks.insert(node, index);
        state.stack.push(node);
        state.on_stack.insert(node);
        for successor in graph.get(&node).into_iter().flatten().copied() {
            if !graph.contains_key(&successor) {
                continue;
            }
            if !state.indices.contains_key(&successor) {
                visit(successor, graph, state);
                state
                    .lowlinks
                    .insert(node, state.lowlinks[&node].min(state.lowlinks[&successor]));
            } else if state.on_stack.contains(&successor) {
                state
                    .lowlinks
                    .insert(node, state.lowlinks[&node].min(state.indices[&successor]));
            }
        }
        if state.lowlinks[&node] == state.indices[&node] {
            let mut component = Vec::new();
            loop {
                let member = state.stack.pop().expect("SCC root remains on stack");
                state.on_stack.remove(&member);
                component.push(member);
                if member == node {
                    break;
                }
            }
            component.sort_unstable();
            state.components.push(component);
        }
    }

    let mut state = Tarjan {
        next: 0,
        indices: BTreeMap::new(),
        lowlinks: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        components: Vec::new(),
    };
    for node in graph.keys().copied() {
        if !state.indices.contains_key(&node) {
            visit(node, graph, &mut state);
        }
    }
    state.components.sort();
    state.components
}
