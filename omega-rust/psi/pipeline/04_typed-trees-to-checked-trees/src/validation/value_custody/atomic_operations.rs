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

use language_core::atomic::AtomicOrderingPlan;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::TableAssignment;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceNode;
use terminal_psi::access_plans::{AccessOperation, AtomicAccessOperation, BorrowPolarity};

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
pub fn place_is_atomic_storage(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    place: ExpressionHandle,
) -> bool {
    let Some(declared) = crate::validation::value_custody::places::declared_place_type(
        program, machine, state, place,
    ) else {
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

/// One atomic source carrier decoded into its sealed operation, the place it
/// accesses, and its authored operands.
///
/// The parser spells every atomic operation through an arithmetic-shaped
/// carrier (tokens-to-syntax-trees `atomic_lets.rs`): a load is the value
/// `Atomic { value: place, .. }`, and every writing operation is the
/// assignment `place = Atomic { .. }` whose `value` models the new resident in
/// terms of the observed prior. That model exists for the checked interpreter;
/// execution needs the operation it denotes. This decoder is the one owner of
/// that recovery, shared by checked planning and by lowering's independent
/// source correspondence, so the two cannot drift apart on which operand an
/// event reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicCarrierAccess {
    pub operation: AtomicAccessOperation,
    /// The accessed atomic place: the load's own operand, or the carrier
    /// assignment's target.
    pub place: ExpressionHandle,
    /// Authored operands in event order: the stored or swapped value, the
    /// fetch operand, or the compare-exchange expected and replacement.
    pub operands: Vec<ExpressionHandle>,
    /// The generated result name the observed prior binds; invalid for a
    /// load (whose value is the carrier itself) and a store.
    pub result: ExpressionHandle,
}

/// Decode a load carrier: `place.load(ordering)` in value position.
pub fn atomic_load_carrier(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<AtomicCarrierAccess> {
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(expression) else {
        return None;
    };
    let AtomicOrderingPlan::Load(ordering) = atomic.ordering else {
        return None;
    };
    (ordering.valid_for_load()
        && atomic.result_custody.is_valid_for(atomic.ordering)
        && !atomic.result_custody.requires_result_destination()
        && !atomic.result.is_valid())
    .then_some(AtomicCarrierAccess {
        operation: AtomicAccessOperation::Load(ordering),
        place: atomic.value,
        operands: Vec::new(),
        result: ExpressionHandle::invalid(),
    })
}

/// Decode a writing carrier: `target = Atomic { .. }` for store, fetch,
/// swap, or decisive compare-exchange. The arithmetic model must be exactly
/// the desugar's shape, with every observed-prior placeholder naming the
/// result; any other value, an illegal ordering, or the single-attempt form
/// (whose `Uncommitted` outcome has no scalar prior) decodes to nothing.
pub fn atomic_assignment_carrier(
    program: &TypedTrees,
    assignment: &TableAssignment,
) -> Option<AtomicCarrierAccess> {
    let ExpressionNode::Atomic(atomic) = program.expression_table.expression(assignment.value)
    else {
        return None;
    };
    if !atomic.result_custody.is_valid_for(atomic.ordering)
        || atomic.result_custody.requires_result_destination()
    {
        return None;
    }
    let binary = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => Some(*binary),
        _ => None,
    };
    let prior = |expression| same_result_name(program, expression, atomic.result);
    let (operation, operands) = match atomic.ordering {
        AtomicOrderingPlan::Load(_) | AtomicOrderingPlan::CompareExchangeOnce { .. } => {
            return None;
        }
        AtomicOrderingPlan::Store(ordering) => {
            if !ordering.valid_for_store() || atomic.result.is_valid() {
                return None;
            }
            (AtomicAccessOperation::Store(ordering), vec![atomic.value])
        }
        AtomicOrderingPlan::Swap(ordering) => {
            if !atomic.result.is_valid() {
                return None;
            }
            (AtomicAccessOperation::Swap(ordering), vec![atomic.value])
        }
        AtomicOrderingPlan::ReadModifyWrite(ordering) => {
            let model = binary(atomic.value)?;
            if !prior(model.left) {
                return None;
            }
            let operation = match model.operator {
                BinaryOperator::Add => AtomicAccessOperation::FetchAdd(ordering),
                BinaryOperator::Subtract => AtomicAccessOperation::FetchSub(ordering),
                BinaryOperator::BitwiseXor => AtomicAccessOperation::FetchXor(ordering),
                BinaryOperator::BitwiseOr => AtomicAccessOperation::FetchOr(ordering),
                BinaryOperator::BitwiseAnd => AtomicAccessOperation::FetchAnd(ordering),
                _ => return None,
            };
            (operation, vec![model.right])
        }
        AtomicOrderingPlan::CompareExchange { success, failure } => {
            if !failure.valid_compare_exchange_failure(success) {
                return None;
            }
            // prior + (prior == expected) * (replacement - prior)
            let sum = binary(atomic.value)?;
            let product = binary(sum.right)?;
            let equal = binary(product.left)?;
            let difference = binary(product.right)?;
            if sum.operator != BinaryOperator::Add
                || product.operator != BinaryOperator::Multiply
                || equal.operator != BinaryOperator::Equal
                || difference.operator != BinaryOperator::Subtract
                || !prior(sum.left)
                || !prior(equal.left)
                || !prior(difference.right)
            {
                return None;
            }
            (
                AtomicAccessOperation::CompareExchange { success, failure },
                vec![equal.right, difference.left],
            )
        }
    };
    Some(AtomicCarrierAccess {
        operation,
        place: assignment.target,
        operands,
        result: atomic.result,
    })
}

/// Whether `expression` is a one-member name spelling the same member as
/// the carrier's result name. The desugar generates every prior placeholder
/// with the result binding's own name; binding that name to the actual local
/// is the consumer's separate check.
fn same_result_name(
    program: &TypedTrees,
    expression: ExpressionHandle,
    result: ExpressionHandle,
) -> bool {
    let member = |expression: ExpressionHandle| {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
            return None;
        };
        match program.expression_table.name_path_members(path.members) {
            [member] => Some(member.clone()),
            _ => None,
        }
    };
    matches!((member(expression), member(result)), (Some(left), Some(right)) if left == right)
}

#[cfg(test)]
mod tests;
