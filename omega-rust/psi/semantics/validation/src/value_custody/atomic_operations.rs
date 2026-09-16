//! Which sealed atomic operation an assignment carrier denotes, and the
//! receiver polarity that operation demands of the target's root.
//!
//! The parser spells `place.store(v, ordering)`, the fetch family, swap, and
//! compare-exchange as an assignment whose value is an
//! [`ExpressionNode::Atomic`] carrier (tokens-to-syntax-trees
//! `atomic_lets.rs`). That spelling is a lowering convenience, not a custody
//! fact: every sealed atomic requirement takes a shared receiver
//! (`wiki/spec/language/concurrency.md`, "All receivers are shared"), so the
//! ordinary "assignment target must be mutable" rule would wrongly reject
//! `self.value.store(..)` through `&self`. This module recovers the operation
//! the carrier denotes and asks the access-plan vocabulary for its receiver
//! polarity, so the custody check demands exactly what the operation requires
//! rather than deciding per type name.
//!
//! Two guards keep this from widening ordinary writes through shared borrows:
//!
//! - only a carrier whose target is atomic storage qualifies, classified by
//!   builtin atom and never by spelling. `.store(..)` on a plain `u32` is not
//!   an atomic operation on an atomic cell, so it keeps the exclusive demand;
//! - a direct `self.value = v` on an atomic field is not a carrier and keeps
//!   the exclusive demand; ordinary assignment never becomes atomic.
//!
//! Placed atomic fields are the sibling case owned by `placed_views`, whose
//! per-operation permission rows already authorize the carrier.

use access_plans::{AccessOperation, AtomicAccessOperation, BorrowPolarity};
use language_core::atomic::AtomicOrderingPlan;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::TableAssignment;
use typed_trees::types::TypeReferenceNode;

/// The borrow polarity an assignment demands of its target's root: exclusive
/// for an ordinary write, or the carried atomic operation's own receiver
/// polarity when the assignment is the parser's carrier for that operation
/// on atomic storage.
pub(crate) fn assignment_receiver_polarity(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    assignment: &TableAssignment,
) -> BorrowPolarity {
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment.value)
    else {
        return BorrowPolarity::Exclusive;
    };
    let Some(operation) = carried_atomic_operation(program, atomic) else {
        return BorrowPolarity::Exclusive;
    };
    if !place_is_atomic_storage(program, machine, state, assignment.target) {
        return BorrowPolarity::Exclusive;
    }
    AccessOperation::Atomic(operation).receiver_polarity()
}

/// The sealed atomic operation an assignment-position carrier denotes.
/// `None` for a load (a load carrier is the assigned value, never an
/// operation on the target) or a read-modify-write whose interpreter model is
/// not one of the fetch families.
pub(crate) fn carried_atomic_operation(
    program: &TypedTrees,
    atomic: &TableAtomicExpression,
) -> Option<AtomicAccessOperation> {
    match atomic.ordering {
        AtomicOrderingPlan::Load(_) => None,
        AtomicOrderingPlan::Store(ordering) => Some(AtomicAccessOperation::Store(ordering)),
        AtomicOrderingPlan::ReadModifyWrite(ordering) => {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(atomic.value)
            else {
                return None;
            };
            Some(match binary.operator {
                BinaryOperator::Add => AtomicAccessOperation::FetchAdd(ordering),
                BinaryOperator::Subtract => AtomicAccessOperation::FetchSub(ordering),
                BinaryOperator::BitwiseXor => AtomicAccessOperation::FetchXor(ordering),
                BinaryOperator::BitwiseOr => AtomicAccessOperation::FetchOr(ordering),
                BinaryOperator::BitwiseAnd => AtomicAccessOperation::FetchAnd(ordering),
                _ => return None,
            })
        }
        AtomicOrderingPlan::Swap(ordering) => Some(AtomicAccessOperation::Swap(ordering)),
        AtomicOrderingPlan::CompareExchange { success, failure } => {
            Some(AtomicAccessOperation::CompareExchange { success, failure })
        }
        AtomicOrderingPlan::CompareExchangeOnce { success, failure } => {
            Some(AtomicAccessOperation::CompareExchangeOnce { success, failure })
        }
    }
}

/// Whether a place's declared type is one of the dedicated atomic core types.
/// Classification goes through the compiler-installed builtin atom, so a
/// package type that merely spells `Atomic...` does not qualify and an
/// ordinary integer is never implicitly atomic.
pub(crate) fn place_is_atomic_storage(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    place: ExpressionHandle,
) -> bool {
    let Some(declared) =
        crate::value_custody::places::declared_place_type(program, machine, state, place)
    else {
        return false;
    };
    match program.type_reference_table.type_reference(declared) {
        TypeReferenceNode::Named { symbol, .. } => program
            .symbols
            .builtin_type_atom(*symbol)
            .is_some_and(|atom| atom.is_atomic()),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
