//! Optimizer module role: algorithm leaf. Deterministic strongly connected component reconstruction.

use super::*;

pub(super) fn cyclic_components(graph: &BTreeMap<BlockId, Vec<BlockId>>) -> Vec<Vec<BlockId>> {
    strongly_connected_components(graph)
        .into_iter()
        .filter(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|block| graph[block].contains(block))
        })
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
