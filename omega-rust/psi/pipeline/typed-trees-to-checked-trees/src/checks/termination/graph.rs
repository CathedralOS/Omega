use typed_trees::statement::{StatementNode, TransitionTargetNode};

/// A transition edge between two states of a machine, identified by their
/// indices into `program.machine_states(machine)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StateEdge {
    pub(super) from: usize,
    pub(super) to: usize,
}

pub(super) fn machine_has_cycle(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> bool {
    let adjacency = machine_adjacency(program, machine);

    adjacency
        .iter()
        .enumerate()
        .any(|(start, edges)| edges.contains(&start) || reaches_self(start, &adjacency))
}

/// Build the per-state adjacency list of the machine's transition graph. The
/// index of each entry corresponds to the state's position in
/// `program.machine_states(machine)`.
pub(super) fn machine_adjacency(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<Vec<usize>> {
    let states = program.machine_states(machine);
    states
        .iter()
        .map(|state| outgoing_state_indices(program, machine, state.symbol))
        .collect()
}

/// Compute the strongly connected components of the machine transition graph
/// using an iterative Tarjan traversal. Each returned component is a list of
/// state indices; components are returned in reverse topological order, which
/// is irrelevant to callers that only care about cycle membership.
pub(super) fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let count = adjacency.len();
    let mut index_counter = 0usize;
    let mut indices = vec![usize::MAX; count];
    let mut low_links = vec![0usize; count];
    let mut on_stack = vec![false; count];
    let mut stack: Vec<usize> = Vec::new();
    let mut components: Vec<Vec<usize>> = Vec::new();

    // Iterative DFS frame: the node being explored and the next neighbour to
    // visit within its adjacency list.
    for root in 0..count {
        if indices[root] != usize::MAX {
            continue;
        }

        let mut work: Vec<(usize, usize)> = vec![(root, 0)];
        while let Some((node, next_neighbour)) = work.pop() {
            if next_neighbour == 0 {
                indices[node] = index_counter;
                low_links[node] = index_counter;
                index_counter += 1;
                stack.push(node);
                on_stack[node] = true;
            }

            let neighbours = &adjacency[node];
            if next_neighbour < neighbours.len() {
                let neighbour = neighbours[next_neighbour];
                // Resume the parent after this neighbour is handled.
                work.push((node, next_neighbour + 1));
                if indices[neighbour] == usize::MAX {
                    work.push((neighbour, 0));
                } else if on_stack[neighbour] {
                    low_links[node] = low_links[node].min(indices[neighbour]);
                }
                continue;
            }

            // All neighbours explored: fold child low-links into the parent.
            for &neighbour in neighbours {
                if on_stack[neighbour] {
                    low_links[node] = low_links[node].min(low_links[neighbour]);
                }
            }

            if low_links[node] == indices[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                components.push(component);
            }
        }
    }

    components
}

/// True when the component forms a cycle: either more than one state, or a
/// single state with a transition back to itself.
pub(super) fn component_is_cyclic(adjacency: &[Vec<usize>], component: &[usize]) -> bool {
    if component.len() > 1 {
        return true;
    }
    component
        .first()
        .is_some_and(|&node| adjacency[node].contains(&node))
}

/// All transition edges whose source and target both belong to `component`.
/// These are exactly the edges that participate in the cycle and across which
/// a measure must strictly decrease.
pub(super) fn cyclic_edges(adjacency: &[Vec<usize>], component: &[usize]) -> Vec<StateEdge> {
    let mut edges = Vec::new();
    for &from in component {
        for &to in &adjacency[from] {
            if component.contains(&to) {
                edges.push(StateEdge { from, to });
            }
        }
    }
    edges
}

fn reaches_self(start: usize, adjacency: &[Vec<usize>]) -> bool {
    let mut stack = adjacency[start].clone();
    let mut visited = vec![false; adjacency.len()];

    while let Some(index) = stack.pop() {
        if index == start {
            return true;
        }
        if visited[index] {
            continue;
        }
        visited[index] = true;
        stack.extend(adjacency[index].iter().copied());
    }

    false
}

fn outgoing_state_indices(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_symbol: symbols::SymbolHandle,
) -> Vec<usize> {
    let states = program.machine_states(machine);
    let Some(state) = states.iter().find(|state| state.symbol == state_symbol) else {
        return Vec::new();
    };
    let mut outgoing = Vec::new();

    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };

        if let Some(index) = target_state_index(
            program,
            machine,
            program.statement_table.transition_target(transition.target),
        ) {
            outgoing.push(index);
        }
        if let Some(index) = target_state_index(
            program,
            machine,
            program
                .statement_table
                .transition_target(transition.continuation),
        ) {
            outgoing.push(index);
        }
    }

    outgoing
}

fn target_state_index(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    target: &TransitionTargetNode,
) -> Option<usize> {
    let target_symbol = match target {
        TransitionTargetNode::Named { path, .. } => path.symbol,
        TransitionTargetNode::SelfTarget => return None,
        TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => return None,
    };

    named_transition_target_state_index(program, machine, target_symbol)
}

/// Resolve one named transition target to its state index inside `machine`.
/// A transition back to the machine's declared entry names the machine symbol,
/// while transitions to subordinate states name the state symbol. Ranking and
/// progress analysis must share this normalization so an entry back-edge is
/// not mistaken for a nested machine invocation.
pub(crate) fn named_transition_target_state_index(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    target_symbol: symbols::SymbolHandle,
) -> Option<usize> {
    if target_symbol == machine.symbol {
        let entry_name = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or_default();
        return program
            .machine_states(machine)
            .iter()
            .position(|state| state.name.as_str() == entry_name)
            .or_else(|| (!program.machine_states(machine).is_empty()).then_some(0));
    }

    program
        .machine_states(machine)
        .iter()
        .position(|state| state.symbol == target_symbol)
}
