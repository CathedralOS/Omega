//! Current integer equality composes through authenticated operand constructors.
//!
//! Copies already publish live ordering equations. Querying another projection
//! may use those equations for its selectors without renaming storage, replaying
//! an initializer, or changing write/join identity. The equality leaves use the
//! existing bounded difference graph with exact identity. A query-local finite
//! closure then composes those independent equalities with the same constructor
//! rules: a copied compound value need not have the same outer constructor as
//! its receiving place. No terms or order premises are synthesized.

use super::{Operand, Relation};

mod difference_graph;
use difference_graph::DifferenceGraph;

struct IndexedOrderGraph<'relations> {
    operands: Vec<&'relations Operand>,
    indexed_relations: Vec<(usize, usize, i64)>,
    same_cache: Vec<Option<bool>>,
}

impl<'relations> IndexedOrderGraph<'relations> {
    fn new(differences: &DifferenceGraph<'relations>) -> Self {
        let operands = differences.operands().to_vec();
        let indexed_relations = differences.indexed_relations().collect();
        let count = operands.len();
        Self {
            operands,
            indexed_relations,
            same_cache: vec![None; count * count],
        }
    }
}

const MAXIMUM_TERMS: usize = 256;
const MAXIMUM_COMPARISONS: usize = 65_536;

pub(in crate::proof_contracts::arithmetic_domains) fn same_value(
    relations: &[Relation],
    left: &Operand,
    right: &Operand,
) -> bool {
    EqualityQuery::new(relations, left, right).same_value(left, right)
}

/// One immutable relation set owns its exact ordering graph. Direct constructor
/// comparisons and bounded congruence closure share the lazily prepared rows;
/// no cache survives a write, join or rebinding of that relation set.
pub(in crate::proof_contracts::arithmetic_domains) struct ValueComparison<'relations> {
    relations: &'relations [Relation],
    differences: Option<DifferenceGraph<'relations>>,
    order_graph: Option<IndexedOrderGraph<'relations>>,
}

impl<'relations> ValueComparison<'relations> {
    pub(in crate::proof_contracts::arithmetic_domains) fn new(
        relations: &'relations [Relation],
    ) -> Self {
        Self {
            relations,
            differences: None,
            order_graph: None,
        }
    }

    pub(in crate::proof_contracts::arithmetic_domains) fn ordered_gaps(
        &mut self,
        value: &'relations Operand,
    ) -> Vec<(&'relations Operand, i64)> {
        if self.relations.is_empty() {
            return vec![(value, 0)];
        }
        let relations = self.relations;
        let differences = &mut self.differences;
        let graph = self.order_graph.get_or_insert_with(|| {
            IndexedOrderGraph::new(
                differences.get_or_insert_with(|| DifferenceGraph::new(relations)),
            )
        });
        let n = graph.operands.len();
        if n == 0 {
            return vec![(value, 0)];
        }
        let value_idx = graph
            .operands
            .iter()
            .position(|existing| *existing == value);
        let round_bound = if value_idx.is_some() { n } else { n + 1 };
        let start_idx = value_idx.unwrap_or(n);
        let mut dist: Vec<(usize, &'relations Operand, i64)> = vec![(start_idx, value, 0)];
        let mut dist_slot: Vec<Option<usize>> = vec![None; n];
        let mut checked_dist_len: Vec<usize> = vec![0; n];
        if let Some(idx) = value_idx {
            dist_slot[idx] = Some(0);
            checked_dist_len[idx] = 1;
        }
        let mut same_with_value: Vec<Option<bool>> = if value_idx.is_none() {
            vec![None; n]
        } else {
            Vec::new()
        };

        let resolve_slot = |op_idx: usize,
                            dist: &[(usize, &'relations Operand, i64)],
                            dist_slot: &mut [Option<usize>],
                            checked_dist_len: &mut [usize],
                            same_cache: &mut [Option<bool>],
                            same_with_value: &mut [Option<bool>],
                            differences: &mut Option<DifferenceGraph<'relations>>|
         -> Option<usize> {
            if let Some(slot) = dist_slot[op_idx] {
                return Some(slot);
            }
            for (slot_idx, &(known_idx, _, _)) in
                dist.iter().enumerate().skip(checked_dist_len[op_idx])
            {
                let is_same = if known_idx == op_idx {
                    true
                } else if known_idx == n {
                    if let Some(cached) = same_with_value[op_idx] {
                        cached
                    } else {
                        let result = Self::same_value_with_differences(
                            relations,
                            differences,
                            value,
                            graph.operands[op_idx],
                        );
                        same_with_value[op_idx] = Some(result);
                        result
                    }
                } else {
                    let cell = known_idx * n + op_idx;
                    if let Some(cached) = same_cache[cell] {
                        cached
                    } else {
                        let result = Self::same_value_with_differences(
                            relations,
                            differences,
                            graph.operands[known_idx],
                            graph.operands[op_idx],
                        );
                        same_cache[cell] = Some(result);
                        same_cache[op_idx * n + known_idx] = Some(result);
                        result
                    }
                };
                if is_same {
                    dist_slot[op_idx] = Some(slot_idx);
                    checked_dist_len[op_idx] = dist.len();
                    return Some(slot_idx);
                }
            }
            checked_dist_len[op_idx] = dist.len();
            None
        };

        for _ in 1..round_bound {
            let mut improved = false;
            for &(left_idx, right_idx, floor) in &graph.indexed_relations {
                let Some(right_slot) = resolve_slot(
                    right_idx,
                    &dist,
                    &mut dist_slot,
                    &mut checked_dist_len,
                    &mut graph.same_cache,
                    &mut same_with_value,
                    differences,
                ) else {
                    continue;
                };
                let Some(gap) = dist[right_slot].2.checked_add(floor) else {
                    continue;
                };
                match resolve_slot(
                    left_idx,
                    &dist,
                    &mut dist_slot,
                    &mut checked_dist_len,
                    &mut graph.same_cache,
                    &mut same_with_value,
                    differences,
                ) {
                    Some(left_slot) if dist[left_slot].2 >= gap => {}
                    Some(left_slot) => {
                        dist[left_slot].2 = gap;
                        improved = true;
                    }
                    None => {
                        let new_slot = dist.len();
                        dist.push((left_idx, graph.operands[left_idx], gap));
                        dist_slot[left_idx] = Some(new_slot);
                        checked_dist_len[left_idx] = dist.len();
                        improved = true;
                    }
                }
            }
            if !improved {
                break;
            }
        }
        dist.into_iter()
            .map(|(_, operand, gap)| (operand, gap))
            .collect()
    }

    pub(in crate::proof_contracts::arithmetic_domains) fn same_value(
        &mut self,
        left: &Operand,
        right: &Operand,
    ) -> bool {
        Self::same_value_with_differences(self.relations, &mut self.differences, left, right)
    }

    fn same_value_with_differences(
        relations: &'relations [Relation],
        differences: &mut Option<DifferenceGraph<'relations>>,
        left: &Operand,
        right: &Operand,
    ) -> bool {
        if Self::compare_with_differences(relations, differences, left, right, 0) {
            return true;
        }
        let differences = differences.get_or_insert_with(|| DifferenceGraph::new(relations));
        Equalities::prepare([left, right], differences)
            .is_some_and(|equalities| equalities.same_value(left, right))
    }

    fn compare(&mut self, left: &Operand, right: &Operand, depth: usize) -> bool {
        Self::compare_with_differences(self.relations, &mut self.differences, left, right, depth)
    }

    fn compare_with_differences(
        relations: &'relations [Relation],
        differences: &mut Option<DifferenceGraph<'relations>>,
        left: &Operand,
        right: &Operand,
        depth: usize,
    ) -> bool {
        if left == right {
            return true;
        }
        if depth >= 128 {
            return false;
        }
        if same_constructor(left, right, |left, right| {
            Self::compare_with_differences(relations, differences, left, right, depth + 1)
        }) {
            return true;
        }
        differences
            .get_or_insert_with(|| DifferenceGraph::new(relations))
            .mutual_nonnegative_gap(left, right)
    }
}

fn same_constructor(
    left: &Operand,
    right: &Operand,
    mut child: impl FnMut(&Operand, &Operand) -> bool,
) -> bool {
    fn same_children(
        left: &[Operand],
        right: &[Operand],
        child: &mut impl FnMut(&Operand, &Operand) -> bool,
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| child(left, right))
    }
    match (left, right) {
        (Operand::Place { root: left, .. }, Operand::Place { root: right, .. }) => {
            left.is_valid() && left == right
        }
        (Operand::Integer(left, _), Operand::Integer(right, _)) => left
            .value_bignum()
            .zip(right.value_bignum())
            .is_some_and(|(left, right)| left == right),
        (
            Operand::Field {
                receiver: left,
                field: left_field,
                primitive: left_primitive,
                ..
            },
            Operand::Field {
                receiver: right,
                field: right_field,
                primitive: right_primitive,
                ..
            },
        ) => {
            left_field.is_valid()
                && left_field == right_field
                && left_primitive == right_primitive
                && child(left, right)
        }
        (
            Operand::Indexed {
                collection: left_collection,
                index: left_index,
                primitive: left_primitive,
            },
            Operand::Indexed {
                collection: right_collection,
                index: right_index,
                primitive: right_primitive,
            },
        ) => {
            left_primitive == right_primitive
                && child(left_collection, right_collection)
                && child(left_index, right_index)
        }
        (Operand::CollectionLength(left), Operand::CollectionLength(right)) => child(left, right),
        (
            Operand::Call {
                target: left_target,
                arguments: left,
            },
            Operand::Call {
                target: right_target,
                arguments: right,
            },
        ) => {
            left_target.is_valid()
                && left_target == right_target
                && same_children(left, right, &mut child)
        }
        (
            Operand::Binary {
                operator: left_operator,
                primitive: left_primitive,
                domain: left_domain,
                operands: left,
            },
            Operand::Binary {
                operator: right_operator,
                primitive: right_primitive,
                domain: right_domain,
                operands: right,
            },
        ) => {
            left_operator == right_operator
                && left_primitive == right_primitive
                && left_domain == right_domain
                && same_children(left, right, &mut child)
        }
        _ => false,
    }
}

/// One immutable ordering query owns its equality work. The common direct proof
/// remains first; only a failure prepares the finite closure, once per walk.
/// In particular, the outer difference graph must not rebuild this roster for
/// each candidate comparison or use that comparison to seed its own equations.
pub(super) struct EqualityQuery<'terms> {
    comparison: ValueComparison<'terms>,
    roots: [&'terms Operand; 2],
    prepared: PreparedEqualities<'terms>,
}

enum PreparedEqualities<'terms> {
    Unprepared,
    Unavailable,
    Known(Equalities<'terms>),
}

impl<'terms> EqualityQuery<'terms> {
    pub(super) fn new(
        relations: &'terms [Relation],
        left: &'terms Operand,
        right: &'terms Operand,
    ) -> Self {
        Self {
            comparison: ValueComparison::new(relations),
            roots: [left, right],
            prepared: PreparedEqualities::Unprepared,
        }
    }

    pub(super) fn same_value(&mut self, left: &Operand, right: &Operand) -> bool {
        if self.comparison.compare(left, right, 0) {
            return true;
        }
        if matches!(self.prepared, PreparedEqualities::Unprepared) {
            let differences = self
                .comparison
                .differences
                .get_or_insert_with(|| DifferenceGraph::new(self.comparison.relations));
            self.prepared = match Equalities::prepare(self.roots, differences) {
                Some(equalities) => PreparedEqualities::Known(equalities),
                None => PreparedEqualities::Unavailable,
            };
        }
        match &self.prepared {
            PreparedEqualities::Known(equalities) => equalities.same_value(left, right),
            _ => false,
        }
    }
}

struct KnownTerm<'terms> {
    operand: &'terms Operand,
    children: Vec<usize>,
}

struct Equalities<'terms> {
    terms: Vec<KnownTerm<'terms>>,
    classes: Vec<usize>,
}

impl<'terms> Equalities<'terms> {
    fn prepare<'relations: 'terms>(
        roots: [&'terms Operand; 2],
        differences: &mut DifferenceGraph<'relations>,
    ) -> Option<Self> {
        let mut terms = Vec::new();
        for root in roots {
            collect_term(&mut terms, root, 0)?;
        }
        let mut endpoints = Vec::new();
        // This is the exact same first-occurrence roster used by the
        // difference solver. Roots and child terms retain their existing
        // insertion order; only duplicate endpoint lookups disappear.
        for &operand in differences.operands() {
            endpoints.push(collect_term(&mut terms, operand, 0)?);
        }
        let mut comparisons = MAXIMUM_COMPARISONS;
        let mut reachable = vec![Vec::new(); terms.len()];
        for (source_vertex, &source) in endpoints.iter().enumerate() {
            for (destination_vertex, &destination) in endpoints.iter().enumerate() {
                if !consume_comparison(&mut comparisons) {
                    return None;
                }
                // Only exact ordering endpoints seed equivalence. Cached rows
                // retain the original relation order, finite round bound and
                // checked gaps; constructor closure cannot add ordering edges.
                if differences.mutual_nonnegative_gap_indexed(source_vertex, destination_vertex) {
                    reachable[source].push(destination);
                }
            }
        }
        let mut classes: Vec<_> = (0..terms.len()).collect();
        // Only the independent EXACT-identity graph seeds equality. Re-running
        // ordering with the classes below would let a requested comparison
        // help establish its own premise. The request itself adds no edge.
        let mut seeded = false;
        for &left in &endpoints {
            for &right in &reachable[left] {
                if reachable[right].contains(&left)
                    && merge_classes(&mut classes, left, right).is_some()
                {
                    seeded = true;
                }
            }
        }
        if !seeded {
            return None;
        }
        let mut uses = vec![Vec::new(); terms.len()];
        for (parent, term) in terms.iter().enumerate() {
            for &child in &term.children {
                let parents = &mut uses[classes[child]];
                if !parents.contains(&parent) {
                    parents.push(parent);
                }
            }
        }
        let mut pending: Vec<_> = (0..terms.len()).collect();
        let mut queued = vec![true; terms.len()];
        let mut cursor = 0;
        while cursor < pending.len() {
            let left = pending[cursor];
            cursor += 1;
            queued[left] = false;
            for right in 0..terms.len() {
                if classes[left] == classes[right] {
                    continue;
                }
                if !consume_comparison(&mut comparisons) {
                    return None;
                }
                if !same_constructor(terms[left].operand, terms[right].operand, |_, _| true)
                    || !terms[left]
                        .children
                        .iter()
                        .zip(&terms[right].children)
                        .all(|(&left, &right)| classes[left] == classes[right])
                {
                    continue;
                }
                let (retained, removed) = merge_classes(&mut classes, left, right)?;
                for parent in std::mem::take(&mut uses[removed]) {
                    if !uses[retained].contains(&parent) {
                        uses[retained].push(parent);
                    }
                }
                // Only constructors using a changed class can gain a new
                // proof. Revisit those parents, not every term at every round.
                for &parent in &uses[retained] {
                    if !queued[parent] {
                        queued[parent] = true;
                        pending.push(parent);
                    }
                }
            }
        }
        Some(Self { terms, classes })
    }

    fn same_value(&self, left: &Operand, right: &Operand) -> bool {
        term_index(&self.terms, left)
            .zip(term_index(&self.terms, right))
            .is_some_and(|(left, right)| self.classes[left] == self.classes[right])
    }
}

fn term_index(terms: &[KnownTerm<'_>], operand: &Operand) -> Option<usize> {
    terms.iter().position(|term| term.operand == operand)
}

fn collect_term<'terms>(
    terms: &mut Vec<KnownTerm<'terms>>,
    operand: &'terms Operand,
    depth: usize,
) -> Option<usize> {
    if let Some(index) = term_index(terms, operand) {
        return Some(index);
    }
    if terms.len() >= MAXIMUM_TERMS || depth >= 128 {
        return None;
    }
    let index = terms.len();
    terms.push(KnownTerm {
        operand,
        children: Vec::new(),
    });
    let children: Vec<&Operand> = match operand {
        Operand::Place { .. } | Operand::Integer(..) | Operand::CompletedCall { .. } => Vec::new(),
        Operand::Field { receiver, .. } | Operand::CollectionLength(receiver) => vec![receiver],
        Operand::Indexed {
            collection, index, ..
        } => vec![collection, index],
        Operand::Call { arguments, .. }
        | Operand::Binary {
            operands: arguments,
            ..
        } => arguments.iter().collect(),
    };
    for child in children {
        let child = collect_term(terms, child, depth + 1)?;
        terms[index].children.push(child);
    }
    Some(index)
}

fn merge_classes(classes: &mut [usize], left: usize, right: usize) -> Option<(usize, usize)> {
    let left = classes[left];
    let right = classes[right];
    if left == right {
        return None;
    }
    let (retained, removed) = (left.min(right), left.max(right));
    for class in classes {
        if *class == removed {
            *class = retained;
        }
    }
    Some((retained, removed))
}

fn consume_comparison(remaining: &mut usize) -> bool {
    if *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    true
}
