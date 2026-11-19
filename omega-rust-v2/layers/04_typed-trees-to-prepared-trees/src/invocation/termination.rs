use super::*;

use omega_typer::TypedTrees;
use std::cell::OnceCell;
use std::collections::{HashMap, VecDeque};

const CYCLIC_INVOCATION: &str = "a cyclic static invocation requires a checked termination proof";

#[derive(Default)]
pub(crate) struct TerminationAdmission<'trees, 'text> {
    graph: OnceCell<LocalCallGraph<'trees>>,
    evidence: Option<CheckedRecursiveBody<'trees, 'text>>,
}

/// The checker grants re-entry only for the body and edges it checked. These
/// references cannot be reused with another typing pass, even if the new
/// package has the same declaration and node IDs.
struct CheckedRecursiveBody<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    signature: &'trees TypedSignature,
    body: TreeNodeId,
    edges: Vec<&'trees TypedCall>,
}

impl CheckedRecursiveBody<'_, '_> {
    fn covers(
        &self,
        call: &TypedCall,
        tree: &TypedTree<'_>,
    ) -> bool {
        let file = tree.resolved().file;
        call.use_site == CallUse::Execution
            && call.node.file == self.body.file
            && (self.body.node..file.nodes[self.body.node].end).contains(&call.node.node)
            && self.edges.iter().any(|edge| std::ptr::eq(*edge, call))
    }
}

impl ConstantEvaluation<'_, '_> {
    pub(super) fn admit_termination(
        &self,
        index: usize,
        signature: &TypedSignature,
        call: &TypedCall,
        scope: &Scope<'_>,
        tree: &TypedTree<'_>,
        execution: &Execution<'_>,
    ) -> Result<(), EvaluationError> {
        let file = tree.resolved().file;
        let declaration = signature.node.node;
        let ranked = file.nodes[declaration]
            .children
            .iter()
            .any(|&node| file.nodes[node].kind == NodeKind::Terminates && file.nodes[node].termination_clause() != Some(omega_parser::TerminationClause::Bare));
        let checked_body = self.termination.evidence.as_ref().filter(|evidence| {
            ranked
                && std::ptr::eq(evidence.trees, self.trees)
                && std::ptr::eq(evidence.signature, signature)
                && file
                    .child(declaration, NodeKind::Body)
                    .map(|body| file.node_id(body))
                    == Some(evidence.body)
        });
        let graph = self
            .termination
            .graph
            .get_or_init(|| LocalCallGraph::new(self.trees, self.signatures()));
        let component = graph.components[index];
        let self_calls = graph.edges[index].iter().filter(|edge| edge.callee == index);
        let checked_cycle =
            checked_body.filter(|evidence| graph.component_sizes[component] == 1 && self_calls.clone().all(|edge| evidence.covers(edge.call, tree)));
        if (graph.component_sizes[component] > 1 || self_calls.count() > 0) && checked_cycle.is_none() {
            let cycle = graph.cycle_from(index);
            return Err(self.call_refusal(CYCLIC_INVOCATION, &execution.chain, &cycle));
        }
        if ranked && checked_body.is_none() {
            return Err("the selected ranked body requires a checked termination proof".into());
        }
        if execution.active.contains(&signature.node) && (!matches!(scope, Scope::Body(_)) || !checked_cycle.is_some_and(|body| body.covers(call, tree))) {
            return Err(CYCLIC_INVOCATION.into());
        }
        Ok(())
    }
}

pub(crate) type SignatureIndex<'trees> = HashMap<&'trees DeclarationId, (usize, &'trees TypedSignature)>;

struct LocalCall<'trees> {
    callee: usize,
    call: &'trees TypedCall,
}

struct LocalCallGraph<'trees> {
    edges: Vec<Vec<LocalCall<'trees>>>,
    components: Vec<usize>,
    component_sizes: Vec<usize>,
}

impl<'trees> LocalCallGraph<'trees> {
    fn new(
        trees: &'trees TypedTrees<'_>,
        signatures: &SignatureIndex<'trees>,
    ) -> Self {
        let size = trees.signatures.len();
        let mut edges: Vec<Vec<_>> = (0..size).map(|_| Vec::new()).collect();
        let mut reverse_edges: Vec<Vec<_>> = (0..size).map(|_| Vec::new()).collect();
        for call in &trees.calls {
            if !matches!(call.use_site, CallUse::Execution | CallUse::Fact) {
                continue;
            }
            let Some((caller, _)) = call.caller.as_ref().and_then(|caller| signatures.get(caller)) else {
                continue;
            };
            let Some((callee, _)) = signatures.get(&call.machine) else {
                continue;
            };
            edges[*caller].push(LocalCall { callee: *callee, call });
            reverse_edges[*callee].push(*caller);
        }

        let mut finished = Vec::with_capacity(size);
        let mut visited = vec![false; size];
        let mut open = Vec::new();
        for root in 0..size {
            if std::mem::replace(&mut visited[root], true) {
                continue;
            }
            open.push((root, edges[root].iter()));
            while let Some((machine, remaining)) = open.last_mut() {
                if let Some(edge) = remaining.next() {
                    if !std::mem::replace(&mut visited[edge.callee], true) {
                        open.push((edge.callee, edges[edge.callee].iter()));
                    }
                } else {
                    finished.push(*machine);
                    open.pop();
                }
            }
        }

        // Traverse reversed calls in finish order to assign the mutually
        // reachable machines. Both traversals use heap storage.
        let mut components = vec![usize::MAX; size];
        let mut component_sizes = Vec::new();
        let mut pending = Vec::new();
        for root in finished.into_iter().rev() {
            if components[root] != usize::MAX {
                continue;
            }
            let component = component_sizes.len();
            let mut count = 0;
            components[root] = component;
            pending.push(root);
            while let Some(machine) = pending.pop() {
                count += 1;
                for &caller in &reverse_edges[machine] {
                    if components[caller] == usize::MAX {
                        components[caller] = component;
                        pending.push(caller);
                    }
                }
            }
            component_sizes.push(count);
        }
        Self {
            edges,
            components,
            component_sizes,
        }
    }

    /// Recover one cycle for the refusal. The invocation itself has not run.
    fn cycle_from(
        &self,
        start: usize,
    ) -> Vec<&'trees TypedCall> {
        let component = self.components[start];
        let mut previous = vec![None; self.edges.len()];
        let mut pending = VecDeque::from([start]);
        while let Some(machine) = pending.pop_front() {
            for edge in &self.edges[machine] {
                if edge.callee == start {
                    if machine == start && self.component_sizes[component] > 1 {
                        continue;
                    }
                    let mut cycle = vec![edge.call];
                    let mut caller = machine;
                    while let Some((parent, call)) = previous[caller] {
                        cycle.push(call);
                        caller = parent;
                    }
                    cycle.reverse();
                    return cycle;
                }
                if self.components[edge.callee] == component && previous[edge.callee].is_none() {
                    previous[edge.callee] = Some((machine, edge.call));
                    pending.push_back(edge.callee);
                }
            }
        }
        unreachable!("a recursive component contains a cycle through each member")
    }
}

#[cfg(test)]
mod tests;
