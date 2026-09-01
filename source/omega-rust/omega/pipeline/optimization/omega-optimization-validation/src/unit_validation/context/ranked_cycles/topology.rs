//! Optimizer module role: semantic leaf. Independent SCC reconstruction from Terminal and current optimizer graphs.

use super::*;

pub(super) fn rederive_exact_components(
    module: &psi_terminal::TerminalModule,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<(MachineId, BTreeSet<BlockId>)>, OptimizationUnitValidationError> {
    let ranked = module
        .machines
        .iter()
        .filter_map(|machine| machine.ranked_scc.as_ref().map(|row| (machine, row)))
        .collect::<Vec<_>>();
    if ranked.is_empty() {
        return Ok(Vec::new());
    }
    let [(machine, ranked)] = ranked.as_slice() else {
        return Err(
            OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                machine: module.entry,
            },
        );
    };
    let terminal_components = cyclic_components(&terminal_graph(machine));
    let [terminal_component] = terminal_components.as_slice() else {
        return Err(
            OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                machine: machine.id,
            },
        );
    };
    if !terminal_component.contains(&ranked.header)
        || ranked.covered_cyclic_edges.iter().any(|edge| {
            !terminal_component.contains(&edge.source) || !terminal_component.contains(&edge.target)
        })
    {
        return Err(
            OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                machine: machine.id,
            },
        );
    }

    let function = unit
        .functions
        .iter()
        .find(|function| function.machine == machine.id)
        .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
            machine.id,
        ))?;
    let current_components = cyclic_components(&optimization_graph(function));
    if current_components.as_slice() != std::slice::from_ref(terminal_component) {
        return Err(
            OptimizationUnitValidationError::RankedCycleTopologyMismatch {
                machine: machine.id,
            },
        );
    }
    Ok(vec![(machine.id, terminal_component.clone())])
}

fn terminal_graph(machine: &psi_terminal::TerminalMachine) -> BTreeMap<BlockId, Vec<BlockId>> {
    machine
        .blocks
        .iter()
        .map(|block| {
            let successors = match &block.terminator {
                psi_terminal::Terminator::Jump { target, .. } => vec![*target],
                psi_terminal::Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![when_true.target, when_false.target],
                _ => Vec::new(),
            };
            (block.id, successors)
        })
        .collect()
}

fn optimization_graph(function: &PsiOptimizationFunction) -> BTreeMap<BlockId, Vec<BlockId>> {
    function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .into_iter()
                    .flat_map(|node| node.successors.iter().map(|edge| edge.target))
                    .collect(),
            )
        })
        .collect()
}

fn cyclic_components(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> Vec<BTreeSet<BlockId>> {
    strongly_connected_components(graph)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|block| graph[block].contains(block))
        })
        .map(|component| component.into_iter().collect())
        .collect()
}

fn strongly_connected_components(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
    let mut state = Tarjan::default();
    for block in graph.keys().copied() {
        if !state.indices.contains_key(&block) {
            visit(block, graph, &mut state);
        }
    }
    state.components.sort();
    state.components
}

#[derive(Default)]
struct Tarjan {
    next: usize,
    indices: BTreeMap<BlockId, usize>,
    lowlinks: BTreeMap<BlockId, usize>,
    stack: Vec<BlockId>,
    on_stack: BTreeSet<BlockId>,
    components: Vec<Vec<BlockId>>,
}

fn visit(block: BlockId, graph: &BTreeMap<BlockId, Vec<BlockId>>, state: &mut Tarjan) {
    let index = state.next;
    state.next += 1;
    state.indices.insert(block, index);
    state.lowlinks.insert(block, index);
    state.stack.push(block);
    state.on_stack.insert(block);
    for successor in graph[&block].iter().copied() {
        if !graph.contains_key(&successor) {
            continue;
        }
        if !state.indices.contains_key(&successor) {
            visit(successor, graph, state);
            state.lowlinks.insert(
                block,
                state.lowlinks[&block].min(state.lowlinks[&successor]),
            );
        } else if state.on_stack.contains(&successor) {
            state
                .lowlinks
                .insert(block, state.lowlinks[&block].min(state.indices[&successor]));
        }
    }
    if state.lowlinks[&block] == state.indices[&block] {
        let mut component = Vec::new();
        loop {
            let member = state.stack.pop().expect("SCC root remains on stack");
            state.on_stack.remove(&member);
            component.push(member);
            if member == block {
                break;
            }
        }
        component.sort_unstable();
        state.components.push(component);
    }
}
