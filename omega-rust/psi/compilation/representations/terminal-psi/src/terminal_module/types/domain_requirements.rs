//! Borrowed, carrier-bound traversal of retained domain prerequisites.
//!
//! Semantic-domain coordinates belong to one module. The current carrier and
//! a unique matching declaration resolve an edge; another presentation or
//! another artifact cannot lend a declaration merely by reusing its number.
//! These queries describe obligations and supplied implications. Predicate
//! proofs, establishment authorization and occurrence custody keep their owners.

use super::{ScalarDomainDeclaration, StructuralDomainDeclaration};
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, StructuralDomainId};

trait RequirementNode {
    type Carrier: Copy + Eq;

    fn semantic_domain(&self) -> DomainSemanticId;
    fn carrier(&self) -> Self::Carrier;
    fn required(&self) -> Option<&[DomainSemanticId]>;
}

impl RequirementNode for StructuralDomainDeclaration {
    type Carrier = semantic_vocabulary::StructuralTypeId;

    fn semantic_domain(&self) -> DomainSemanticId {
        self.semantic_domain
    }
    fn carrier(&self) -> Self::Carrier {
        self.carrier
    }
    fn required(&self) -> Option<&[DomainSemanticId]> {
        self.required_memberships.as_deref()
    }
}

impl RequirementNode for ScalarDomainDeclaration {
    type Carrier = semantic_vocabulary::ScalarType;

    fn semantic_domain(&self) -> DomainSemanticId {
        self.semantic_domain
    }
    fn carrier(&self) -> Self::Carrier {
        self.carrier
    }
    fn required(&self) -> Option<&[DomainSemanticId]> {
        self.required_memberships.as_deref()
    }
}

fn resolve<Node: RequirementNode>(
    rows: &[Node],
    domain: DomainSemanticId,
    carrier: Node::Carrier,
) -> Option<&Node> {
    let mut matches = rows
        .iter()
        .filter(|row| row.semantic_domain() == domain && row.carrier() == carrier);
    let row = matches.next()?;
    matches.next().is_none().then_some(row)
}

struct Closure<'a, Node> {
    nodes: Vec<&'a Node>,
    complete: bool,
}

/// Iteration keeps an untrusted chain from consuming the native call stack.
/// Unknown leaves make the closure incomplete; they never denote no duties.
fn closure<'a, Node: RequirementNode>(root: &Node, rows: &'a [Node]) -> Option<Closure<'a, Node>> {
    let carrier = root.carrier();
    let mut pending = vec![(root.semantic_domain(), false)];
    let mut active = Vec::new();
    let mut visited = Vec::new();
    let mut nodes = Vec::new();
    let mut complete = true;
    while let Some((domain, completed)) = pending.pop() {
        if completed {
            if active.pop() != Some(domain) {
                return None;
            }
            visited.push(domain);
            continue;
        }
        if active.contains(&domain) {
            return None;
        }
        if visited.contains(&domain) {
            continue;
        }
        let row = resolve(rows, domain, carrier)?;
        nodes.push(row);
        active.push(domain);
        pending.push((domain, true));
        if let Some(required) = row.required() {
            if !required.windows(2).all(|pair| pair[0] < pair[1]) {
                return None;
            }
            pending.extend(required.iter().rev().map(|domain| (*domain, false)));
        } else {
            complete = false;
        }
    }
    Some(Closure { nodes, complete })
}

fn missing<'a, Node: RequirementNode>(
    root: &Node,
    rows: &'a [Node],
    supplied: &[DomainSemanticId],
) -> Option<Vec<&'a Node>> {
    let supplied = supplied
        .iter()
        .map(|domain| closure(resolve(rows, *domain, root.carrier())?, rows))
        .collect::<Option<Vec<_>>>()?;
    let mut pending = vec![(root.semantic_domain(), false)];
    let mut active = Vec::new();
    let mut visited = Vec::new();
    let mut missing = Vec::new();
    while let Some((domain, completed)) = pending.pop() {
        if completed {
            if active.pop() != Some(domain) {
                return None;
            }
            visited.push(domain);
            continue;
        }
        if supplied.iter().any(|closure| {
            closure
                .nodes
                .iter()
                .any(|row| row.semantic_domain() == domain)
        }) {
            continue;
        }
        if active.contains(&domain) {
            return None;
        }
        if visited.contains(&domain) {
            continue;
        }
        let row = resolve(rows, domain, root.carrier())?;
        let required = row.required()?;
        if !required.windows(2).all(|pair| pair[0] < pair[1]) {
            return None;
        }
        missing.push(row);
        active.push(domain);
        pending.push((domain, true));
        pending.extend(required.iter().rev().map(|domain| (*domain, false)));
    }
    Some(missing)
}

impl StructuralDomainDeclaration {
    pub fn has_no_required_memberships(&self) -> bool {
        self.required_memberships
            .as_ref()
            .is_some_and(Vec::is_empty)
    }

    pub fn required_memberships_are_well_formed(&self, domains: &[Self]) -> bool {
        closure(self, domains).is_some()
    }

    pub fn required_domain_closure<'a>(&self, domains: &'a [Self]) -> Option<Vec<&'a Self>> {
        let closure = closure(self, domains)?;
        closure.complete.then_some(closure.nodes)
    }

    /// Rows whose local predicate and direct origin are still owed. Already
    /// supplied descendant memberships cover their retained prerequisites.
    pub fn missing_membership_requirements<'a>(
        &self,
        domains: &'a [Self],
        supplied: &[StructuralDomainId],
    ) -> Option<Vec<&'a Self>> {
        let supplied = supplied
            .iter()
            .map(|id| {
                let mut matches = domains
                    .iter()
                    .filter(|row| row.id == *id && row.carrier == self.carrier);
                let row = matches.next()?;
                matches.next().is_none().then_some(row.semantic_domain)
            })
            .collect::<Option<Vec<_>>>()?;
        missing(self, domains, &supplied)
    }
}

impl ScalarDomainDeclaration {
    pub fn has_no_required_memberships(&self) -> bool {
        self.required_memberships
            .as_ref()
            .is_some_and(Vec::is_empty)
    }

    pub fn required_memberships_are_well_formed(&self, domains: &[Self]) -> bool {
        closure(self, domains).is_some()
    }

    pub fn required_domain_closure<'a>(&self, domains: &'a [Self]) -> Option<Vec<&'a Self>> {
        let closure = closure(self, domains)?;
        closure.complete.then_some(closure.nodes)
    }

    pub fn missing_membership_requirements<'a>(
        &self,
        domains: &'a [Self],
        supplied: &[ScalarDomainId],
    ) -> Option<Vec<&'a Self>> {
        let supplied = supplied
            .iter()
            .map(|id| {
                let mut matches = domains
                    .iter()
                    .filter(|row| row.id == *id && row.carrier == self.carrier);
                let row = matches.next()?;
                matches.next().is_none().then_some(row.semantic_domain)
            })
            .collect::<Option<Vec<_>>>()?;
        missing(self, domains, &supplied)
    }
}
