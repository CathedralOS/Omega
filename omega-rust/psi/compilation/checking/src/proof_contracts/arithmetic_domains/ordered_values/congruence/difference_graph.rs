//! Exact ordering leaves for one immutable symbolic comparison batch.
//!
//! Recursive operand equality is needed to build the vertex roster once. The
//! bounded difference walk then reads compact edge indices and numeric gaps;
//! repeated queries from one vertex borrow its completed row. Operands remain
//! borrowed from the caller's live relations, so this scratch cannot outlive
//! their write invalidation or state-edge substitution.
//!
//! Keep relation order and the original vertex-count round bound. Positive
//! cycles and i64 overflow make those observable precision choices. Unequal
//! relations contribute vertices to that bound but never ordering edges.

use super::super::{Operand, Relation};

struct Edge {
    source: usize,
    destination: usize,
    minimum_difference: i64,
}

pub(super) struct DifferenceGraph<'relations> {
    operands: Vec<&'relations Operand>,
    edges: Vec<Edge>,
    has_outgoing: Vec<bool>,
    has_incoming: Vec<bool>,
    component: Vec<u32>,
    gaps: Vec<Option<Vec<Option<i64>>>>,
}

impl<'relations> DifferenceGraph<'relations> {
    pub(super) fn new(relations: &'relations [Relation]) -> Self {
        phase_profile::count("05.ordered_relations.graphs", 1);
        let mut operands = Vec::new();
        let mut edges = Vec::new();
        for relation in relations {
            let destination = retain_operand(&mut operands, &relation.left);
            let source = retain_operand(&mut operands, &relation.right);
            if let Some(minimum_difference) = relation.minimum_difference() {
                edges.push(Edge {
                    source,
                    destination,
                    minimum_difference,
                });
            }
        }
        let len = operands.len();
        let mut has_outgoing = vec![false; len];
        let mut has_incoming = vec![false; len];
        let mut component: Vec<u32> = (0..len as u32).collect();
        for edge in &edges {
            has_outgoing[edge.source] = true;
            has_incoming[edge.destination] = true;
            let mut root_a = edge.source as u32;
            while component[root_a as usize] != root_a {
                component[root_a as usize] = component[component[root_a as usize] as usize];
                root_a = component[root_a as usize];
            }
            let mut root_b = edge.destination as u32;
            while component[root_b as usize] != root_b {
                component[root_b as usize] = component[component[root_b as usize] as usize];
                root_b = component[root_b as usize];
            }
            if root_a != root_b {
                component[root_b as usize] = root_a;
            }
        }
        for index in 0..len {
            let mut root = index as u32;
            while component[root as usize] != root {
                root = component[root as usize];
            }
            component[index] = root;
        }
        let gaps = vec![None; len];
        phase_profile::count("05.ordered_relations.vertices", operands.len() as u64);
        phase_profile::count("05.ordered_relations.edges", edges.len() as u64);
        Self {
            operands,
            edges,
            has_outgoing,
            has_incoming,
            component,
            gaps,
        }
    }

    pub(super) fn nonnegative_gap(&mut self, from: &Operand, to: &Operand) -> bool {
        if from == to {
            return true;
        }
        // An observation absent from every edge endpoint has no ordering path.
        // Constructor congruence is the caller's separate judgment; it never
        // participates in this exact-identity graph or supplies its own premise.
        let Some(source) = self.operands.iter().position(|operand| *operand == from) else {
            return false;
        };
        if !self.has_outgoing[source] {
            return false;
        }
        let Some(destination) = self.operands.iter().position(|operand| *operand == to) else {
            return false;
        };
        if !self.has_incoming[destination] || self.component[source] != self.component[destination]
        {
            return false;
        }
        if self.gaps[source].is_none() {
            phase_profile::count("05.ordered_relations.rows", 1);
            let target_component = self.component[source];
            let mut gaps: Vec<Option<i64>> = vec![None; self.operands.len()];
            gaps[source] = Some(0);
            for _ in 1..self.operands.len() {
                let mut improved = false;
                for edge in &self.edges {
                    if self.component[edge.source] != target_component {
                        continue;
                    }
                    let Some(gap) =
                        gaps[edge.source].and_then(|gap| gap.checked_add(edge.minimum_difference))
                    else {
                        continue;
                    };
                    if gaps[edge.destination].is_none_or(|known| known < gap) {
                        gaps[edge.destination] = Some(gap);
                        improved = true;
                    }
                }
                if !improved {
                    break;
                }
            }
            self.gaps[source] = Some(gaps);
        }
        self.gaps[source]
            .as_ref()
            .is_some_and(|gaps| gaps[destination].is_some_and(|gap| gap >= 0))
    }
}

fn retain_operand<'relations>(
    operands: &mut Vec<&'relations Operand>,
    operand: &'relations Operand,
) -> usize {
    match operands.iter().position(|existing| *existing == operand) {
        Some(position) => position,
        None => {
            operands.push(operand);
            operands.len() - 1
        }
    }
}

#[cfg(test)]
mod tests;
