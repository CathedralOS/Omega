//! Current integer equality composes through authenticated operand constructors.
//!
//! Copies already publish live ordering equations. Querying another projection
//! may use those equations for its selectors without renaming storage, replaying
//! an initializer, or changing write/join identity. The equality leaves use the
//! existing bounded difference graph with exact identity, so congruence cannot
//! recursively manufacture its own premises.

use super::{Operand, Relation, ordered_gaps_by};

pub(super) fn same_value(relations: &[Relation], left: &Operand, right: &Operand) -> bool {
    compare(relations, left, right, 0)
}

fn compare(relations: &[Relation], left: &Operand, right: &Operand, depth: usize) -> bool {
    if left == right {
        return true;
    }
    if depth >= 128 {
        return false;
    }
    let child = |left: &Operand, right: &Operand| compare(relations, left, right, depth + 1);
    let same_children = |left: &[Operand], right: &[Operand]| {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| child(left, right))
    };
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
        ) => left_target.is_valid() && left_target == right_target && same_children(left, right),
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
                && same_children(left, right)
        }
        _ => false,
    };
    if constructed {
        return true;
    }
    let nonnegative_gap = |from: &Operand, to: &Operand| {
        ordered_gaps_by(relations, from, |left, right| left == right)
            .into_iter()
            .any(|(operand, gap)| operand == to && gap >= 0)
    };
    nonnegative_gap(left, right) && nonnegative_gap(right, left)
}
