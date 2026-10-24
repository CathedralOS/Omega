//! Ordered operands for an unconditional place evaluation.
//!
//! A loan keeps its pointee as storage, but its selectors are evaluated values.
//! An address evaluates its collection before its selector and a range's start
//! before its end (`wiki/spec/language/expressions.md`). A captured operand is
//! evaluated once into an ordinary inferred local. Reads, stores and hoisted
//! subjects capture every evaluated operand: later arms, payload receivers and
//! right-hand sides read the place again. An authored reference initializer
//! forms its loan once where it is written, so it captures an operand only
//! when that operand itself may write storage, keeping its effect a statement
//! in authored order, or when a later operand of the same address may write
//! what it read. Its other operands stay authored, and checking keeps their
//! declared ranges, guards and witnesses and names them in diagnostics. This
//! owner is intentionally not used for guarded operands: moving their
//! producers would change short-circuit order. Anonymous arithmetic keeps its
//! contextual landing boundary rather than acquiring a synthetic local's
//! scalar carrier.

use super::value_call_hoisting::expression_contains;
use crate::lowering::statement::hoist_temps::{hoist_into_temp, hoist_operand_into_temp};
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use symbol_resolved_trees::statement::Statement;

/// Which evaluated operands of a reference formation receive their own local.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectorCapture {
    /// An authored initializer: operands that may write, or that a later
    /// operand may write.
    BeforeWrites,
    /// A hoisted subject place read again by later arms: every operand.
    Every,
}

/// Prepare only a reference formation's unconditional address evaluation.
pub(super) fn capture_reference_initializer_selectors(
    lowerer: &mut Lowerer,
    initializer: ExpressionHandle,
    capture: SelectorCapture,
    hoisted: &mut Vec<Statement>,
) {
    capture_formation(
        lowerer,
        initializer,
        capture == SelectorCapture::Every,
        hoisted,
    );
}

/// Retain storage identity while evaluating its address operands in order.
pub(super) fn capture_address_operands(
    lowerer: &mut Lowerer,
    address: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    capture_operands(lowerer, address, true, hoisted)
}

/// A pending typed subject may become either a value snapshot or a storage
/// observation. Prepare the address operands of its projection now, before
/// later subjects execute. The typed owner still decides whether to copy the
/// completed value or lend the place. A produced root with no projection
/// already has that pending value binding and needs no additional local.
pub(super) fn capture_subject_address_operands(
    lowerer: &mut Lowerer,
    subject: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    match lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(subject)
    {
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) | ExpressionNode::Borrow(_) => {
            capture_address_operands(lowerer, subject, hoisted)
        }
        ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            capture_address_operands(lowerer, subject, hoisted)
        }
        _ => subject,
    }
}

/// `capture` says whether an operand evaluated at this point needs its own
/// local: the place is read again later, or a later operand of the same
/// address may write storage.
fn capture_formation(
    lowerer: &mut Lowerer,
    formation: ExpressionHandle,
    capture: bool,
    hoisted: &mut Vec<Statement>,
) {
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(formation)
        .clone();
    match node {
        ExpressionNode::Borrow(mut borrow) => {
            borrow.target = capture_operands(lowerer, borrow.target, capture, hoisted);
            set_expression(lowerer, formation, ExpressionNode::Borrow(borrow));
        }
        ExpressionNode::Cast(mut cast) if cast.form.is_recast() => {
            // The formation owner admits a direct recast of reference-typed
            // storage as well as a recast inside an explicit outer Borrow.
            cast.value = capture_operands(lowerer, cast.value, capture, hoisted);
            set_expression(lowerer, formation, ExpressionNode::Cast(cast));
        }
        _ => {}
    }
}

fn capture_operands(
    lowerer: &mut Lowerer,
    address: ExpressionHandle,
    capture: bool,
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
            member.receiver = capture_operands(lowerer, member.receiver, capture, hoisted);
            set_expression(lowerer, address, ExpressionNode::Member(member));
            address
        }
        ExpressionNode::Indexed(mut indexed) => {
            let selector_writes = operand_may_write(lowerer, indexed.index);
            indexed.collection = capture_operands(
                lowerer,
                indexed.collection,
                capture || selector_writes,
                hoisted,
            );
            indexed.index = capture_selector(lowerer, indexed.index, capture, hoisted);
            set_expression(lowerer, address, ExpressionNode::Indexed(indexed));
            address
        }
        ExpressionNode::Borrow(_) => {
            capture_formation(lowerer, address, capture, hoisted);
            address
        }
        ExpressionNode::Cast(mut cast) if cast.form.is_recast() => {
            cast.value = capture_operands(lowerer, cast.value, capture, hoisted);
            set_expression(lowerer, address, ExpressionNode::Cast(cast));
            address
        }
        // A captured computed root finishes before any later selector. Its
        // ordinary local retains the producer's actual type and custody; no
        // pointee read or initializer replay is introduced here.
        _ if capture || operand_may_write(lowerer, address) => {
            hoist_into_temp(lowerer, address, hoisted)
        }
        _ => address,
    }
}

fn capture_selector(
    lowerer: &mut Lowerer,
    selector: ExpressionHandle,
    capture: bool,
    hoisted: &mut Vec<Statement>,
) -> ExpressionHandle {
    // Literal-only operands read no storage to snapshot. Keep their operators
    // and landing boundaries with the ordinary numeric owner: anonymous trees
    // have no stored carrier yet, while suffixes retain their own carrier.
    // An inferred local here would either keep Unit or introduce premature
    // landing and change rational intermediates into integer operations.
    // This walk neither evaluates a value nor grants an operator's meaning.
    if is_numeric_literal_expression(lowerer, selector) {
        return selector;
    }
    let node = lowerer
        .symbol_resolved_trees
        .tables
        .bodies
        .expressions
        .expression(selector)
        .clone();
    match node {
        ExpressionNode::Range(mut range) => {
            let end_writes = range.end.is_valid() && operand_may_write(lowerer, range.end);
            if range.start.is_valid() {
                range.start =
                    capture_selector(lowerer, range.start, capture || end_writes, hoisted);
            }
            if range.end.is_valid() {
                range.end = capture_selector(lowerer, range.end, capture, hoisted);
            }
            set_expression(lowerer, selector, ExpressionNode::Range(range));
            selector
        }
        // Even a plain scalar place is a value snapshot here: a later operand
        // may write that place after this selector has evaluated.
        _ if capture || operand_may_write(lowerer, selector) => {
            hoist_operand_into_temp(lowerer, selector, hoisted)
        }
        _ => selector,
    }
}

/// A call may write through its arguments or a boundary, and an atomic event
/// may store. Every other operand form only reads.
fn operand_may_write(lowerer: &Lowerer, operand: ExpressionHandle) -> bool {
    expression_contains(lowerer, operand, &|node| {
        matches!(node, ExpressionNode::Call(_) | ExpressionNode::Atomic(_))
    })
}

fn is_numeric_literal_expression(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expressions.expression(expression) {
            ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {}
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Binary(binary) => {
                pending.push(binary.right);
                pending.push(binary.left);
            }
            _ => return false,
        }
    }
    true
}
