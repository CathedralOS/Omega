//! Ordered address operands for unconditional reference initializers.
//!
//! A loan keeps its pointee as storage, but its selectors are evaluated values.
//! Capture those values through ordinary inferred locals before establishing the
//! loan. Walking collections first snapshots earlier selectors before a later
//! selector can mutate their source. This owner is intentionally not used for
//! guarded operands: moving their producers would change short-circuit order.

use crate::lowering::statement::hoist_temps::hoist_into_temp;
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::statement::Statement;

/// Prepare only a reference formation's unconditional address evaluation.
pub(super) fn capture_reference_initializer_selectors(
    lowerer: &mut Lowerer,
    initializer: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(initializer)
        .clone();
    match node {
        ExpressionNode::Borrow(mut borrow) => {
            borrow.target = capture_address_operands(lowerer, borrow.target, hoisted);
            set_expression(lowerer, initializer, ExpressionNode::Borrow(borrow));
        }
        ExpressionNode::Cast(mut cast) if cast.form.is_recast() => {
            // The formation owner admits a direct recast of reference-typed
            // storage as well as a recast inside an explicit outer Borrow.
            cast.value = capture_address_operands(lowerer, cast.value, hoisted);
            set_expression(lowerer, initializer, ExpressionNode::Cast(cast));
        }
        _ => {}
    }
}

fn capture_address_operands(
    lowerer: &mut Lowerer,
    address: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(address)
        .clone();
    match node {
        ExpressionNode::Name(_) => address,
        ExpressionNode::Member(mut member) => {
            member.receiver = capture_address_operands(lowerer, member.receiver, hoisted);
            set_expression(lowerer, address, ExpressionNode::Member(member));
            address
        }
        ExpressionNode::Indexed(mut indexed) => {
            indexed.collection = capture_address_operands(lowerer, indexed.collection, hoisted);
            indexed.index = capture_selector(lowerer, indexed.index, hoisted);
            set_expression(lowerer, address, ExpressionNode::Indexed(indexed));
            address
        }
        ExpressionNode::Borrow(_) => {
            capture_reference_initializer_selectors(lowerer, address, hoisted);
            address
        }
        ExpressionNode::Cast(mut cast) if cast.form.is_recast() => {
            cast.value = capture_address_operands(lowerer, cast.value, hoisted);
            set_expression(lowerer, address, ExpressionNode::Cast(cast));
            address
        }
        // A computed address root must finish before its later selectors.
        // Its ordinary local retains the producer's actual type and custody;
        // no pointee read or initializer replay is introduced here.
        _ => hoist_into_temp(lowerer, address, hoisted),
    }
}

fn capture_selector(
    lowerer: &mut Lowerer,
    selector: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(selector)
        .clone();
    match node {
        ExpressionNode::Integer(_) => selector,
        ExpressionNode::Range(mut range) => {
            if range.start.is_valid() {
                range.start = capture_selector(lowerer, range.start, hoisted);
            }
            if range.end.is_valid() {
                range.end = capture_selector(lowerer, range.end, hoisted);
            }
            set_expression(lowerer, selector, ExpressionNode::Range(range));
            selector
        }
        // Even a plain scalar place is a value snapshot here: an outer
        // selector may write that place after this selector has evaluated.
        _ => hoist_into_temp(lowerer, selector, hoisted),
    }
}
