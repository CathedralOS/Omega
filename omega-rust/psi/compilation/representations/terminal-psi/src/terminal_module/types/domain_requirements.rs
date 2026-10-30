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

/// The prerequisite rows reachable from `root`, in pre-order and in ascending
/// prerequisite order at each node. Iteration keeps an untrusted chain from
/// consuming the native call stack. `prune` drops the subtree below a domain an
/// independent membership already supplies, before the cycle guards: a supplied
/// row closes its own chain rather than repeating it.
fn walk<'a, Node: RequirementNode>(
    root: &Node,
    rows: &'a [Node],
    mut prune: impl FnMut(DomainSemanticId) -> bool,
) -> Option<Vec<&'a Node>> {
    let carrier = root.carrier();
    let mut pending = vec![(root.semantic_domain(), false)];
    let mut active = Vec::new();
    let mut visited = Vec::new();
    let mut nodes = Vec::new();
    while let Some((domain, completed)) = pending.pop() {
        if completed {
            if active.pop() != Some(domain) {
                return None;
            }
            visited.push(domain);
            continue;
        }
        if prune(domain) {
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
        }
    }
    Some(nodes)
}

/// Unknown leaves make the closure incomplete; they never denote no duties.
fn closure<'a, Node: RequirementNode>(root: &Node, rows: &'a [Node]) -> Option<Closure<'a, Node>> {
    let nodes = walk(root, rows, |_| false)?;
    let complete = nodes.iter().all(|node| node.required().is_some());
    Some(Closure { nodes, complete })
}

/// Resolve each supplied local row id to that row's prerequisite closure. A
/// local id names one row of this module on this exact carrier.
fn supplied_closures<'a, Node: RequirementNode, Id: Copy + Eq>(
    root: &Node,
    rows: &'a [Node],
    supplied: &[Id],
    local_id: impl Fn(&Node) -> Id,
) -> Option<Vec<Closure<'a, Node>>> {
    supplied
        .iter()
        .map(|id| {
            let mut matches = rows
                .iter()
                .filter(|row| local_id(row) == *id && row.carrier() == root.carrier());
            let row = matches.next()?;
            matches.next().is_none().then_some(())?;
            closure(resolve(rows, row.semantic_domain(), root.carrier())?, rows)
        })
        .collect()
}

/// The four carrier-bound prerequisite queries. Both declaration kinds answer
/// them through the same traversal; only the local row-id type differs, so the
/// method bodies exist once.
macro_rules! requirement_queries {
    ($declaration:ty, $id:ty) => {
        impl $declaration {
            pub fn has_no_required_memberships(&self) -> bool {
                self.required().is_some_and(<[DomainSemanticId]>::is_empty)
            }

            pub fn required_memberships_are_well_formed(&self, domains: &[Self]) -> bool {
                closure(self, domains).is_some()
            }

            pub fn required_domain_closure<'a>(
                &self,
                domains: &'a [Self],
            ) -> Option<Vec<&'a Self>> {
                let closure = closure(self, domains)?;
                closure.complete.then_some(closure.nodes)
            }

            /// Rows whose local predicate and direct origin are still owed.
            /// Already supplied descendant memberships cover their retained
            /// prerequisites.
            pub fn missing_membership_requirements<'a>(
                &self,
                domains: &'a [Self],
                supplied: &[$id],
            ) -> Option<Vec<&'a Self>> {
                let supplied = supplied_closures(self, domains, supplied, |row| row.id)?;
                // A supplied row closes its own chain instead of repeating it.
                let nodes = walk(self, domains, |domain| {
                    supplied.iter().any(|closure| {
                        closure
                            .nodes
                            .iter()
                            .any(|row| row.semantic_domain() == domain)
                    })
                })?;
                // A row with no retained prerequisite list leaves the answer
                // unknown, never complete.
                nodes
                    .iter()
                    .all(|node| node.required().is_some())
                    .then_some(nodes)
            }
        }
    };
}

requirement_queries!(StructuralDomainDeclaration, StructuralDomainId);
requirement_queries!(ScalarDomainDeclaration, ScalarDomainId);
