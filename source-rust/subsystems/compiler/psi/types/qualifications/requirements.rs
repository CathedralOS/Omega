//! Requirements selected by structural and scalar domains.

use super::{ScalarDomainDeclaration, StructuralDomainDeclaration};
use semantic_vocabulary::{DomainSemanticId, ScalarDomainId, StructuralDomainId};

trait RequirementNode {
    type Carrier: Copy + Eq + Ord;

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

fn closure<'a, Node: RequirementNode>(
    root: &Node,
    rows: &'a [Node],
) -> Option<Closure<'a, Node>> {
    let nodes = walk(root, rows, |_| false)?;
    let complete = nodes.iter().all(|node| node.required().is_some());
    Some(Closure { nodes, complete })
}

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

// Prepare one answer per declaration. Resolve each required identity
// once; missing/ambiguous edges and cycles cannot satisfy the query.
fn requirements_satisfy<Node: RequirementNode>(
    rows: &[Node],
    local: impl Fn(&Node) -> bool,
) -> Vec<bool> {
    let mut index: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(position, row)| ((row.carrier(), row.semantic_domain()), position))
        .collect();
    index.sort_unstable_by_key(|row| row.0);
    let mut answers: Vec<_> = rows.iter().map(&local).collect();
    let mut remaining = vec![0usize; rows.len()];
    let mut dependents = vec![Vec::new(); rows.len()];
    for (position, row) in rows.iter().enumerate() {
        let own = index
            .binary_search_by_key(&(row.carrier(), row.semantic_domain()), |row| row.0)
            .expect("the index contains each input declaration");
        if (own > 0 && index[own - 1].0 == index[own].0)
            || index
                .get(own + 1)
                .is_some_and(|next| next.0 == index[own].0)
        {
            answers[position] = false;
        }
        let Some(required) = row
            .required()
            .filter(|required| required.windows(2).all(|pair| pair[0] < pair[1]))
        else {
            answers[position] = false;
            continue;
        };
        for required in required {
            let key = (row.carrier(), *required);
            let Ok(found) = index.binary_search_by_key(&key, |row| row.0) else {
                answers[position] = false;
                continue;
            };
            if (found > 0 && index[found - 1].0 == key)
                || index.get(found + 1).is_some_and(|next| next.0 == key)
            {
                answers[position] = false;
                continue;
            }
            let dependency = index[found].1;
            remaining[position] += 1;
            dependents[dependency].push(position);
        }
    }
    let mut pending: std::collections::VecDeque<_> = remaining
        .iter()
        .enumerate()
        .filter_map(|(position, count)| (*count == 0).then_some(position))
        .collect();
    while let Some(position) = pending.pop_front() {
        for dependent in &dependents[position] {
            answers[*dependent] &= answers[position];
            remaining[*dependent] -= 1;
            if remaining[*dependent] == 0 {
                pending.push_back(*dependent);
            }
        }
    }
    for (answer, remaining) in answers.iter_mut().zip(remaining) {
        *answer &= remaining == 0;
    }
    answers
}

impl StructuralDomainDeclaration {
    /// Prepare which already held memberships have no content facts
    /// to invalidate. This is not authority to establish a membership.
    pub fn content_independent_memberships(domains: &[Self]) -> Vec<StructuralDomainId> {
        let mut identities: Vec<_> = domains.iter().map(|row| row.id).collect();
        identities.sort_unstable();
        if identities.windows(2).any(|pair| pair[0] == pair[1]) {
            return Vec::new();
        }
        let answers = requirements_satisfy(domains, |row| {
            row.complete_predicate_theory
                && row.predicate.as_ref().is_some_and(|predicate| {
                    matches!(
                        predicate.proposition,
                        semantic_vocabulary::Proposition::Truth
                    )
                })
                && row.byte_predicate.is_none()
                && row.content_projection.is_none()
        });
        let mut held: Vec<_> = domains
            .iter()
            .zip(answers)
            .filter_map(|(row, holds)| holds.then_some(row.id))
            .collect();
        held.sort_unstable();
        held
    }
}

macro_rules! requirement_queries {
    ($declaration:ty, $id:ty) => {
        impl $declaration {
    pub fn authorizes_boundary_requirement(
        &self,
        requirement: crate::TerminalIdentityIndex,
        declaration: Option<&crate::NominalDeclarationIdentity>,
    ) -> bool {
        self.establishment_routes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|route| {
                matches!(route, crate::StructuralEstablishmentRoute::BoundaryRequirement {
                requirement_identity: authorized,
                declaration: owner,
            } if *authorized == requirement && Some(owner) == declaration)
            })
    }

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

            pub fn missing_membership_requirements<'a>(
                &self,
                domains: &'a [Self],
                supplied: &[$id],
            ) -> Option<Vec<&'a Self>> {
                let supplied = supplied_closures(self, domains, supplied, |row| row.id)?;
                let nodes = walk(self, domains, |domain| {
                    supplied.iter().any(|closure| {
                        closure
                            .nodes
                            .iter()
                            .any(|row| row.semantic_domain() == domain)
                    })
                })?;
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
