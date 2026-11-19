use super::*;

use crate::TargetObservation;
use omega_typer::TypedTrees;
use std::cell::OnceCell;
use std::collections::{HashMap, VecDeque};

const CYCLIC_INVOCATION: &str = "a cyclic static invocation requires a checked termination proof";
const UNRESOLVED_CALLBACK_CLOSURE: &str = "the exact static callback has an unresolved call in its termination closure";

#[derive(Default)]
pub(crate) struct TerminationAdmission<'trees, 'text> {
    graph: OnceCell<LocalCallGraph<'trees>>,
    evidence: Vec<CheckedRecursiveComponent<'trees, 'text>>,
    valid_components: OnceCell<Vec<bool>>,
}

/// One checker judgment covers the joint ranking and each internal edge.
/// An individual body's proof cannot grant entry to a mutually recursive group.
pub struct CheckedRecursiveComponent<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    members: Vec<CheckedRecursiveMember<'trees>>,
    edges: Vec<CheckedRecursiveEdge<'trees>>,
    observations: Vec<TargetObservation>,
}

struct CheckedRecursiveMember<'trees> {
    signature: &'trees TypedSignature,
    body: TreeNodeId,
    application: RecursiveApplication,
}

struct CheckedRecursiveEdge<'trees> {
    caller: usize,
    callee: usize,
    call: &'trees TypedCall,
}

/// Runtime operands do not enter this key. The checked ranking applies to
/// all admitted runtime inputs of these exact closed static substitutions.
#[derive(Clone, Default, Debug, Eq, Hash, PartialEq)]
struct RecursiveApplication {
    arguments: Vec<ValueType>,
    enclosing: Vec<(DeclarationId, Vec<ValueType>)>,
}

impl RecursiveApplication {
    fn select(
        call: &TypedCall,
        instantiate: impl Fn(&ValueType) -> ValueType,
    ) -> Option<Self> {
        if call.evidence.is_some() {
            return None;
        }
        let arguments: Vec<_> = call.type_arguments.iter().map(&instantiate).collect();
        let enclosing: Vec<(_, Vec<_>)> = call
            .enclosing_arguments
            .iter()
            .filter(|(_, arguments)| !arguments.is_empty())
            .map(|(owner, arguments)| (owner.clone(), arguments.iter().map(&instantiate).collect()))
            .collect();
        if arguments
            .iter()
            .chain(enclosing.iter().flat_map(|(_, arguments)| arguments))
            .any(ValueType::depends_on_call_input)
        {
            return None;
        }
        Some(Self { arguments, enclosing })
    }

    fn instantiate(
        &self,
        caller: &DeclarationId,
        value: &ValueType,
    ) -> ValueType {
        let value = self
            .enclosing
            .iter()
            .fold(value.clone(), |value, (owner, arguments)| value.substitute(owner, arguments));
        value.substitute(caller, &self.arguments)
    }
}

impl CheckedRecursiveComponent<'_, '_> {
    fn covers(
        &self,
        callee: usize,
        call: &TypedCall,
        scope: &Scope<'_>,
    ) -> bool {
        if !matches!(scope, Scope::Body(_)) {
            return false;
        }
        let caller_application = match scope.application() {
            Some(selected) if Some(&selected.machine) == call.caller.as_ref() => {
                let Some(application) = RecursiveApplication::select(selected, ValueType::clone) else {
                    return false;
                };
                application
            }
            Some(_) => return false,
            None => RecursiveApplication::default(),
        };
        self.edges
            .iter()
            .any(|edge| edge.callee == callee && std::ptr::eq(edge.call, call) && self.members[edge.caller].application == caller_application)
    }
}

impl<'stage, 'text> crate::CheckedPreparationEvidence<'stage, 'text> {
    /// Include the checker's joint ranking for these exact closed bodies and
    /// internal calls. Each independent recursive component needs its own proof.
    pub fn with_recursive_component(
        mut self,
        component: CheckedRecursiveComponent<'stage, 'text>,
    ) -> Self {
        self.termination.evidence.push(component);
        self
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
        let graph = self.local_call_graph();
        let component = graph.components[index];
        let internal_call = call
            .caller
            .as_ref()
            .and_then(|caller| self.signatures().get(caller))
            .is_some_and(|&(caller, signature)| graph.components[caller] == component && execution.active.contains(&signature.node));
        let reentry = execution.active.contains(&signature.node);
        let edge = (internal_call || reentry).then_some((call, scope));
        if !graph.is_cyclic(index) && reentry {
            return Err(CYCLIC_INVOCATION.into());
        }
        self.admit_selected_termination(
            index,
            signature,
            tree,
            || RecursiveApplication::select(call, |argument| scope.instantiate(argument)),
            edge,
            execution,
        )
        .map(|_| ())
    }

    pub(super) fn admit_callback_termination(
        &self,
        index: usize,
        signature: &TypedSignature,
        tree: &TypedTree<'_>,
        arguments: &[ValueType],
        execution: &Execution<'_>,
    ) -> Result<(), EvaluationError> {
        if arguments.iter().any(ValueType::depends_on_call_input) || tree.resolved().file.node_id(signature.node.node) != signature.node {
            return Err(UNRESOLVED_CALLBACK_CLOSURE.into());
        }
        let graph = self.local_call_graph();
        let mut pending = vec![(
            index,
            RecursiveApplication {
                arguments: arguments.to_vec(),
                enclosing: vec![],
            },
        )];
        let mut checked = HashSet::new();
        while let Some((index, application)) = pending.pop() {
            if !checked.insert((index, application.clone())) {
                continue;
            }
            let signature = self
                .trees
                .signatures
                .get(index)
                .ok_or(UNRESOLVED_CALLBACK_CLOSURE)?;
            let tree = self.tree(signature.node)?;
            let proof = self.admit_selected_termination(index, signature, &tree, || Some(application.clone()), None, execution)?;
            if let Some(proof) = proof {
                // The checker has already validated every internal edge. Walk
                // each member once, following the closed calls leaving it.
                for member in &proof.members {
                    let &(member_index, _) = self
                        .signatures()
                        .get(&member.signature.machine)
                        .ok_or(UNRESOLVED_CALLBACK_CLOSURE)?;
                    checked.insert((member_index, member.application.clone()));
                    self.visit_callback_member(graph, member_index, member.signature, &member.application, execution, &mut pending)?;
                }
            } else {
                self.visit_callback_member(graph, index, signature, &application, execution, &mut pending)?;
            }
        }
        Ok(())
    }

    fn visit_callback_member(
        &self,
        graph: &LocalCallGraph<'_>,
        index: usize,
        signature: &TypedSignature,
        application: &RecursiveApplication,
        execution: &Execution<'_>,
        pending: &mut Vec<(usize, RecursiveApplication)>,
    ) -> Result<(), EvaluationError> {
        if execution.active.contains(&signature.node) {
            return Err(CYCLIC_INVOCATION.into());
        }
        let tree = self.tree(signature.node)?;
        let file = tree.resolved().file;
        if tree.resolved().declaration_at(signature.node.node) != Some(&signature.machine)
            || file.child(signature.node.node, NodeKind::Body).is_none()
            || file.generic_parameters(signature.node.node).count() != application.arguments.len()
            || file.nodes[signature.node.node].has_public_modifier() && !signature.terminates
            || self
                .machine_effects(index)
                .is_none_or(|effects| !effects.invokes.is_empty())
        {
            return Err(UNRESOLVED_CALLBACK_CLOSURE.into());
        }
        let edges = graph.edges[index]
            .iter()
            .map(|edge| (Some(edge.callee), edge.call));
        let external = graph.external_calls[index].iter().map(|&call| (None, call));
        for (callee, call) in edges.chain(external) {
            if call.node.file != signature.node.file
                || call.machine.kind != NodeKind::Machine
                || selected_call(&tree, call.node.node, call.use_site)? != Some(call)
            {
                return Err(UNRESOLVED_CALLBACK_CLOSURE.into());
            }
            let selected =
                RecursiveApplication::select(call, |argument| application.instantiate(&signature.machine, argument)).ok_or(UNRESOLVED_CALLBACK_CLOSURE)?;
            // A provider-controlled edge can change the recursive graph.
            // This closure is valid only for direct local implementations.
            let mut trait_scopes = selected
                .enclosing
                .iter()
                .filter(|(owner, _)| owner.kind == NodeKind::Trait);
            let trait_arguments = trait_scopes
                .next()
                .map_or(&[][..], |(_, arguments)| arguments.as_slice());
            let ambiguous_trait_scope = trait_scopes.next().is_some();
            if self
                .trees
                .boundary_calls
                .iter()
                .any(|(site, _)| *site == call.site)
                || self.boundary_slots.iter().any(|slot| {
                    slot.requirement.declaration == call.machine
                        && slot.arguments == selected.arguments
                        && (ambiguous_trait_scope || slot.requirement.trait_arguments == trait_arguments)
                })
            {
                return Err(UNRESOLVED_CALLBACK_CLOSURE.into());
            }
            if call.layout_query.is_some() {
                // A typed core layout projection does not enter a machine body.
                continue;
            }
            let callee = callee.ok_or(UNRESOLVED_CALLBACK_CLOSURE)?;
            if graph.components[callee] != graph.components[index] {
                pending.push((callee, selected));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments, reason = "a direct back edge needs its original typed occurrence and scope")]
    fn admit_selected_termination(
        &self,
        index: usize,
        signature: &TypedSignature,
        tree: &TypedTree<'_>,
        application: impl FnOnce() -> Option<RecursiveApplication>,
        edge: Option<(&TypedCall, &Scope<'_>)>,
        execution: &Execution<'_>,
    ) -> Result<Option<&CheckedRecursiveComponent<'_, '_>>, EvaluationError> {
        let file = tree.resolved().file;
        let ranked = file.nodes[signature.node.node]
            .children
            .iter()
            .any(|&node| file.nodes[node].kind == NodeKind::Terminates && file.nodes[node].termination_clause() != Some(omega_parser::TerminationClause::Bare));
        let graph = self.local_call_graph();
        let cyclic = graph.is_cyclic(index);
        if !cyclic && !ranked {
            return Ok(None);
        }
        let validity = self.termination.valid_components.get_or_init(|| {
            self.termination
                .evidence
                .iter()
                .map(|proof| self.check_recursive_component(proof, graph))
                .collect()
        });
        let mut changed_selection = false;
        let mut wrong_edge = false;
        if let Some(application) = application() {
            for (proof, &valid) in self.termination.evidence.iter().zip(validity) {
                if !valid {
                    continue;
                }
                let Some(member) = proof
                    .members
                    .iter()
                    .position(|member| std::ptr::eq(member.signature, signature) && member.application == application)
                else {
                    continue;
                };
                if edge.is_some_and(|(call, scope)| !proof.covers(member, call, scope)) {
                    wrong_edge = true;
                    continue;
                }
                if !proof.observations.iter().all(|used| {
                    self.target
                        .is_some_and(|target| target.matches_observation(used))
                }) {
                    changed_selection = true;
                    continue;
                }
                for observation in &proof.observations {
                    self.record_observation(observation.clone());
                }
                return Ok(Some(proof));
            }
        }
        if changed_selection {
            return Err("the checked termination proof depends on a different target selection".into());
        }
        if wrong_edge {
            return Err(CYCLIC_INVOCATION.into());
        }
        if cyclic {
            let cycle = graph.cycle_from(index);
            return Err(self.call_refusal(CYCLIC_INVOCATION, &execution.chain, &cycle));
        }
        Err("the selected ranked body requires a checked termination proof".into())
    }

    fn local_call_graph(&self) -> &LocalCallGraph<'_> {
        self.termination
            .graph
            .get_or_init(|| LocalCallGraph::new(self.trees, self.signatures()))
    }

    fn check_recursive_component(
        &self,
        proof: &CheckedRecursiveComponent<'_, '_>,
        graph: &LocalCallGraph<'_>,
    ) -> bool {
        if !std::ptr::eq(proof.trees, self.trees) {
            return false;
        }
        let signatures = self.signatures();
        let Some(first) = proof.members.first() else { return false };
        let package = &first.signature.machine.package;
        let Some(&(first, _)) = signatures.get(&first.signature.machine) else {
            return false;
        };
        let component = graph.components[first];
        let mut declared_members = vec![false; graph.edges.len()];
        let mut body_ends = Vec::with_capacity(proof.members.len());
        let mut member_by_application = HashMap::with_capacity(proof.members.len());
        let mut declaration_count = 0;
        for (member_index, member) in proof.members.iter().enumerate() {
            let Some(&(index, signature)) = signatures.get(&member.signature.machine) else {
                return false;
            };
            if graph.components[index] != component
                || &signature.machine.package != package
                || !std::ptr::eq(signature, member.signature)
                || member_by_application
                    .insert((index, &member.application), member_index)
                    .is_some()
            {
                return false;
            }
            let Ok(tree) = self.tree(signature.node) else { return false };
            let file = tree.resolved().file;
            let Some(body) = file.child(signature.node.node, NodeKind::Body) else {
                return false;
            };
            if member.body != file.node_id(body)
                || !file.nodes[signature.node.node].children.iter().any(|&node| {
                    file.nodes[node].kind == NodeKind::Terminates && file.nodes[node].termination_clause() != Some(omega_parser::TerminationClause::Bare)
                })
            {
                return false;
            }
            declaration_count += usize::from(!std::mem::replace(&mut declared_members[index], true));
            body_ends.push((index, file.nodes[body].end));
        }
        if declaration_count != graph.component_sizes[component] {
            return false;
        }

        let mut remaining = HashMap::with_capacity(proof.edges.len());
        for edge in &proof.edges {
            if remaining.insert((edge.caller, edge.call.node), edge).is_some() {
                return false;
            }
        }
        let mut forward = vec![Vec::new(); proof.members.len()];
        let mut reverse = vec![Vec::new(); proof.members.len()];
        for (caller, (member, &(index, body_end))) in proof.members.iter().zip(&body_ends).enumerate() {
            for edge in graph.edges[index]
                .iter()
                .filter(|edge| graph.components[edge.callee] == component)
            {
                let call = edge.call;
                if call.use_site != CallUse::Execution || call.node.file != member.body.file || !(member.body.node..body_end).contains(&call.node.node) {
                    return false;
                }
                let Some(application) = RecursiveApplication::select(call, |argument| {
                    member
                        .application
                        .instantiate(&member.signature.machine, argument)
                }) else {
                    return false;
                };
                let Some(&callee) = member_by_application.get(&(edge.callee, &application)) else {
                    return false;
                };
                if !remaining
                    .remove(&(caller, call.node))
                    .is_some_and(|checked| checked.callee == callee && std::ptr::eq(checked.call, call))
                {
                    return false;
                }
                forward[caller].push(callee);
                reverse[callee].push(caller);
            }
        }
        remaining.is_empty() && reaches_every_member(&forward) && reaches_every_member(&reverse)
    }
}

fn reaches_every_member(edges: &[Vec<usize>]) -> bool {
    let mut visited = vec![false; edges.len()];
    let mut pending = vec![0];
    while let Some(member) = pending.pop() {
        if !std::mem::replace(&mut visited[member], true) {
            pending.extend(&edges[member]);
        }
    }
    visited.into_iter().all(|reached| reached)
}

pub(crate) type SignatureIndex<'trees> = HashMap<&'trees DeclarationId, (usize, &'trees TypedSignature)>;

struct LocalCall<'trees> {
    callee: usize,
    call: &'trees TypedCall,
}

struct LocalCallGraph<'trees> {
    edges: Vec<Vec<LocalCall<'trees>>>,
    external_calls: Vec<Vec<&'trees TypedCall>>,
    components: Vec<usize>,
    component_sizes: Vec<usize>,
}

impl<'trees> LocalCallGraph<'trees> {
    fn is_cyclic(
        &self,
        index: usize,
    ) -> bool {
        self.component_sizes[self.components[index]] > 1 || self.edges[index].iter().any(|edge| edge.callee == index)
    }

    fn new(
        trees: &'trees TypedTrees<'_>,
        signatures: &SignatureIndex<'trees>,
    ) -> Self {
        let size = trees.signatures.len();
        let mut edges: Vec<Vec<_>> = (0..size).map(|_| Vec::new()).collect();
        let mut external_calls: Vec<Vec<_>> = (0..size).map(|_| Vec::new()).collect();
        let mut reverse_edges: Vec<Vec<_>> = (0..size).map(|_| Vec::new()).collect();
        for call in &trees.calls {
            if !matches!(call.use_site, CallUse::Execution | CallUse::Fact) {
                continue;
            }
            let Some((caller, _)) = call.caller.as_ref().and_then(|caller| signatures.get(caller)) else {
                continue;
            };
            let Some((callee, _)) = signatures.get(&call.machine) else {
                external_calls[*caller].push(call);
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
            external_calls,
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
pub(crate) mod tests;
