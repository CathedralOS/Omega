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
    #[cfg(test)]
    component: Vec<u32>,
    core_component: Vec<u32>,
    core_component_edges: Vec<Vec<usize>>,
    #[cfg(test)]
    gaps: Vec<Option<Vec<Option<i64>>>>,
    core_gaps: Vec<Option<Vec<Option<i64>>>>,
}

impl<'relations> DifferenceGraph<'relations> {
    /// Exact endpoints in their original first-occurrence order, including
    /// vertices introduced only by nonordering relations. Consumers retain
    /// these indices instead of rediscovering each endpoint in an inner loop.
    pub(super) fn operands(&self) -> &[&'relations Operand] {
        &self.operands
    }

    pub(super) fn indexed_relations(&self) -> impl Iterator<Item = (usize, usize, i64)> + '_ {
        self.edges
            .iter()
            .map(|edge| (edge.destination, edge.source, edge.minimum_difference))
    }

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
        for edge in &edges {
            has_outgoing[edge.source] = true;
            has_incoming[edge.destination] = true;
        }
        #[cfg(test)]
        let component = {
            let mut component: Vec<u32> = (0..len as u32).collect();
            for edge in &edges {
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
            component
        };
        // Any directed path between vertices that both have incoming and outgoing
        // edges stays entirely within the subgraph of vertices with both incoming
        // and outgoing edges (excluding pure sinks such as carrier ceilings and
        // pure sources). Unioning only core edges prevents sink ceilings from
        // merging independent equality pairs into a single component.
        let mut core_component: Vec<u32> = (0..len as u32).collect();
        for edge in &edges {
            if !(has_outgoing[edge.source]
                && has_incoming[edge.source]
                && has_outgoing[edge.destination]
                && has_incoming[edge.destination])
            {
                continue;
            }
            let mut root_a = edge.source as u32;
            while core_component[root_a as usize] != root_a {
                core_component[root_a as usize] =
                    core_component[core_component[root_a as usize] as usize];
                root_a = core_component[root_a as usize];
            }
            let mut root_b = edge.destination as u32;
            while core_component[root_b as usize] != root_b {
                core_component[root_b as usize] =
                    core_component[core_component[root_b as usize] as usize];
                root_b = core_component[root_b as usize];
            }
            if root_a != root_b {
                core_component[root_b as usize] = root_a;
            }
        }
        for index in 0..len {
            let mut root = index as u32;
            while core_component[root as usize] != root {
                root = core_component[root as usize];
            }
            core_component[index] = root;
        }
        let mut core_component_edges: Vec<Vec<usize>> = vec![Vec::new(); len];
        for (edge_index, edge) in edges.iter().enumerate() {
            if has_outgoing[edge.source]
                && has_incoming[edge.source]
                && has_outgoing[edge.destination]
                && has_incoming[edge.destination]
            {
                core_component_edges[core_component[edge.source] as usize].push(edge_index);
            }
        }
        let core_gaps = vec![None; len];
        phase_profile::count("05.ordered_relations.vertices", operands.len() as u64);
        phase_profile::count("05.ordered_relations.edges", edges.len() as u64);
        Self {
            operands,
            edges,
            has_outgoing,
            has_incoming,
            #[cfg(test)]
            component,
            core_component,
            core_component_edges,
            #[cfg(test)]
            gaps: vec![None; len],
            core_gaps,
        }
    }

    pub(super) fn mutual_nonnegative_gap(&mut self, left: &Operand, right: &Operand) -> bool {
        if left == right {
            return true;
        }
        let Some(source) = self.operands.iter().position(|operand| *operand == left) else {
            return false;
        };
        let Some(destination) = self.operands.iter().position(|operand| *operand == right) else {
            return false;
        };
        self.mutual_nonnegative_gap_indexed(source, destination)
    }

    /// Both indices belong to this graph's published endpoint roster. The
    /// equality closure already owns them; no symbolic lookup is needed here.
    pub(super) fn mutual_nonnegative_gap_indexed(
        &mut self,
        source: usize,
        destination: usize,
    ) -> bool {
        if source == destination {
            return true;
        }
        if !self.has_outgoing[source]
            || !self.has_incoming[source]
            || !self.has_outgoing[destination]
            || !self.has_incoming[destination]
            || self.core_component[source] != self.core_component[destination]
        {
            return false;
        }
        self.core_nonnegative_gap_indexed(source, destination)
            && self.core_nonnegative_gap_indexed(destination, source)
    }

    fn core_nonnegative_gap_indexed(&mut self, source: usize, destination: usize) -> bool {
        if self.core_gaps[source].is_none() {
            phase_profile::count("05.ordered_relations.rows", 1);
            let target_component = self.core_component[source] as usize;
            let component_edges = &self.core_component_edges[target_component];
            let mut gaps: Vec<Option<i64>> = vec![None; self.operands.len()];
            gaps[source] = Some(0);
            for _ in 1..self.operands.len() {
                let mut improved = false;
                for &edge_index in component_edges {
                    let edge = &self.edges[edge_index];
                    let Some(source_gap) = gaps[edge.source] else {
                        continue;
                    };
                    let Some(gap) = source_gap.checked_add(edge.minimum_difference) else {
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
            self.core_gaps[source] = Some(gaps);
        }
        self.core_gaps[source]
            .as_ref()
            .is_some_and(|gaps| gaps[destination].is_some_and(|gap| gap >= 0))
    }

    #[cfg(test)]
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
        if self.has_incoming[source] && self.has_outgoing[destination] {
            if self.core_component[source] != self.core_component[destination] {
                return false;
            }
            return self.core_nonnegative_gap_indexed(source, destination);
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
                    let Some(source_gap) = gaps[edge.source] else {
                        continue;
                    };
                    let Some(gap) = source_gap.checked_add(edge.minimum_difference) else {
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
