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

/// One immutable relation set supplies all equality queries in an arithmetic
/// operation. The exact difference graph is built only if constructor
/// congruence cannot decide a query, then reused by its child and sibling
/// queries. No results cross a write, join, rebinding or environment change.
pub(in crate::proof_contracts::arithmetic_domains) struct ValueComparison<'relations> {
    relations: &'relations [Relation],
    differences: Option<DifferenceGraph<'relations>>,
}

impl<'relations> ValueComparison<'relations> {
    pub(in crate::proof_contracts::arithmetic_domains) fn new(
        relations: &'relations [Relation],
    ) -> Self {
        Self {
            relations,
            differences: None,
        }
    }

    pub(in crate::proof_contracts::arithmetic_domains) fn same_value(
        &mut self,
        left: &Operand,
        right: &Operand,
    ) -> bool {
        self.compare(left, right, 0)
    }

    fn same_children(&mut self, left: &[Operand], right: &[Operand], depth: usize) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.compare(left, right, depth + 1))
    }

    fn compare(&mut self, left: &Operand, right: &Operand, depth: usize) -> bool {
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
                    && self.compare(left, right, depth + 1)
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
                    && self.compare(left_collection, right_collection, depth + 1)
                    && self.compare(left_index, right_index, depth + 1)
            }
            (Operand::CollectionLength(left), Operand::CollectionLength(right)) => {
                self.compare(left, right, depth + 1)
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
                    && self.same_children(left, right, depth)
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
                    && self.same_children(left, right, depth)
            }
            _ => false,
        };
        if constructed {
            return true;
        }
        let differences = self
            .differences
            .get_or_insert_with(|| DifferenceGraph::new(self.relations));
        differences.nonnegative_gap(left, right) && differences.nonnegative_gap(right, left)
    }
}
