//! Current integer equality composes through authenticated operand constructors.
//!
//! Copies already publish live ordering equations. Querying another projection
//! may use those equations for its selectors without renaming storage, replaying
//! an initializer, or changing write/join identity. The equality leaves use the
//! existing bounded difference graph with exact identity, so congruence cannot
//! recursively manufacture its own premises.

use super::{Operand, Relation};

mod difference_graph;
use difference_graph::DifferenceGraph;

struct IndexedOrderGraph<'relations> {
    operands: Vec<&'relations Operand>,
    indexed_relations: Vec<(usize, usize, i64)>,
    same_cache: Vec<Option<bool>>,
}

impl<'relations> IndexedOrderGraph<'relations> {
    fn new(relations: &'relations [Relation]) -> Self {
        let mut operands: Vec<&'relations Operand> = Vec::new();
        let mut indexed_relations = Vec::with_capacity(relations.len());
        for relation in relations {
            let left_idx = operands
                .iter()
                .position(|existing| *existing == &relation.left)
                .unwrap_or_else(|| {
                    let idx = operands.len();
                    operands.push(&relation.left);
                    idx
                });
            let right_idx = operands
                .iter()
                .position(|existing| *existing == &relation.right)
                .unwrap_or_else(|| {
                    let idx = operands.len();
                    operands.push(&relation.right);
                    idx
                });
            if let Some(floor) = relation.minimum_difference() {
                indexed_relations.push((left_idx, right_idx, floor));
            }
        }
        let count = operands.len();
        Self {
            operands,
            indexed_relations,
            same_cache: vec![None; count * count],
        }
    }
}

/// One immutable relation set supplies all equality queries in an arithmetic
/// operation. The exact difference graph is built only if constructor
/// congruence cannot decide a query, then reused by its child and sibling
/// queries. No results cross a write, join, rebinding or environment change.
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
        let graph = self
            .order_graph
            .get_or_insert_with(|| IndexedOrderGraph::new(relations));
        let differences = &mut self.differences;
        let n = graph.operands.len();
        if n == 0 {
            return vec![(value, 0)];
        }
        let value_idx = graph.operands.iter().position(|existing| *existing == value);
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
            for (slot_idx, &(known_idx, _, _)) in dist
                .iter()
                .enumerate()
                .skip(checked_dist_len[op_idx])
            {
                let is_same = if known_idx == op_idx {
                    true
                } else if known_idx == n {
                    if let Some(cached) = same_with_value[op_idx] {
                        cached
                    } else {
                        let result = Self::compare_with_differences(
                            relations,
                            differences,
                            value,
                            graph.operands[op_idx],
                            0,
                        );
                        same_with_value[op_idx] = Some(result);
                        result
                    }
                } else {
                    let cell = known_idx * n + op_idx;
                    if let Some(cached) = same_cache[cell] {
                        cached
                    } else {
                        let result = Self::compare_with_differences(
                            relations,
                            differences,
                            graph.operands[known_idx],
                            graph.operands[op_idx],
                            0,
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
        Self::compare_with_differences(self.relations, &mut self.differences, left, right, 0)
    }

    fn same_children(
        relations: &'relations [Relation],
        differences: &mut Option<DifferenceGraph<'relations>>,
        left: &[Operand],
        right: &[Operand],
        depth: usize,
    ) -> bool {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left, right)| {
                Self::compare_with_differences(relations, differences, left, right, depth + 1)
            })
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
        let constructed = match (left, right) {
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
                    && Self::compare_with_differences(
                        relations,
                        differences,
                        left,
                        right,
                        depth + 1,
                    )
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
                    && Self::compare_with_differences(
                        relations,
                        differences,
                        left_collection,
                        right_collection,
                        depth + 1,
                    )
                    && Self::compare_with_differences(
                        relations,
                        differences,
                        left_index,
                        right_index,
                        depth + 1,
                    )
            }
            (Operand::CollectionLength(left), Operand::CollectionLength(right)) => {
                Self::compare_with_differences(relations, differences, left, right, depth + 1)
            }
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
                    && Self::same_children(relations, differences, left, right, depth)
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
                    && Self::same_children(relations, differences, left, right, depth)
            }
            _ => false,
        };
        if constructed {
            return true;
        }
        let differences = differences.get_or_insert_with(|| DifferenceGraph::new(relations));
        differences.mutual_nonnegative_gap(left, right)
    }
}
