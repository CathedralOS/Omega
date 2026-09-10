//! Replay the ordered whole-place observations of a mixed shared-borrow call.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowAccessKind, BorrowCallFact, CheckedTrees};

use crate::{LoweringError, unsupported};

/// Each result is the first access-row position of an authored formal actual.
/// Scalar actuals may contribute zero, one, or several observation rows.
pub(super) fn rejoin(
    checked: &CheckedTrees,
    call: &BorrowCallFact,
    arguments: &[ExpressionHandle],
) -> Result<Vec<usize>, LoweringError> {
    let borrow = &checked.facts.borrow;
    let rows = borrow
        .argument_accesses
        .span(call.accesses)
        .ok_or(LoweringError::Unsupported(
            "computed shared borrow has a stale observation roster",
        ))?;
    let table = &checked.expression_table;
    let mut positions = Vec::with_capacity(arguments.len());
    let mut position = 0;
    for argument in arguments {
        positions.push(position);
        let mut pending = vec![(*argument, true, false)];
        let mut active = Vec::new();
        while let Some((expression, direct_argument, exiting)) = pending.pop() {
            if exiting {
                active.pop();
                continue;
            }
            if !table.expression_is_valid(expression) || active.contains(&expression) {
                return unsupported(
                    "computed shared borrow has a stale or cyclic source observation",
                );
            }
            active.push(expression);
            pending.push((expression, direct_argument, true));
            let mut children = Vec::new();
            let (named, kind) = match table.expression(expression) {
                ExpressionNode::Borrow(borrow) if direct_argument => (
                    Some(borrow.target),
                    match borrow.access {
                        language_core::ReferenceAccess::Shared => BorrowAccessKind::Read,
                        language_core::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
                        language_core::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
                    },
                ),
                ExpressionNode::Name(_) => (Some(expression), BorrowAccessKind::Read),
                ExpressionNode::Member(_) => (Some(expression), BorrowAccessKind::Read),
                // This follows borrow/accesses/read.rs: a nested call's own
                // borrow formation has a separate call row. Here its actuals
                // contribute reads to the enclosing argument's observation.
                ExpressionNode::Borrow(borrow) => {
                    children.push(borrow.target);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Atomic(atomic) => {
                    children.push(atomic.value);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Binary(binary) => {
                    children.extend([binary.left, binary.right]);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Match(dispatch) => {
                    children.push(dispatch.subject);
                    let arms = table.match_arms(dispatch.arms);
                    if arms.len() != dispatch.arms.len() {
                        return unsupported(
                            "computed shared borrow has stale dispatch observations",
                        );
                    }
                    for arm in arms {
                        if let checked_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                        {
                            children.push(pattern);
                        }
                        children.push(arm.value);
                    }
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Unary(unary) => {
                    children.push(unary.operand);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Cast(cast) => {
                    children.push(cast.value);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Call(call) => {
                    if call.receiver.is_valid() {
                        children.push(call.receiver);
                    }
                    let arguments = table.expression_handles(call.arguments);
                    if arguments.len() != call.arguments.count() as usize {
                        return unsupported(
                            "computed shared borrow has stale nested argument observations",
                        );
                    }
                    children.extend_from_slice(arguments);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ZeroValue(_) => (None, BorrowAccessKind::Read),
                _ => {
                    return unsupported(
                        "computed shared borrow requires whole named scalar observations",
                    );
                }
            };
            if let Some(named) = named {
                if !table.expression_is_valid(named) {
                    return unsupported("computed shared borrow has a stale named observation");
                }
                let mut receiver = named;
                let mut segments = Vec::new();
                let mut receivers = Vec::new();
                while let ExpressionNode::Member(member) = table.expression(receiver) {
                    if !member.member_symbol.is_valid()
                        || member.case_variant.is_some()
                        || !table.expression_is_valid(member.receiver)
                        || receivers.contains(&receiver)
                    {
                        return unsupported(
                            "computed argument has a stale or unsupported field observation",
                        );
                    }
                    receivers.push(receiver);
                    segments.push(facts::PlaceSegment::Field {
                        symbol: member.member_symbol,
                    });
                    receiver = member.receiver;
                }
                segments.reverse();
                let ExpressionNode::Name(name) = table.expression(receiver) else {
                    return unsupported("computed shared borrow requires a whole named referent");
                };
                let row = rows.get(position).ok_or(LoweringError::Unsupported(
                    "computed shared borrow omits an authored observation",
                ))?;
                if !name.symbol.is_valid()
                    || name.head_symbol != name.symbol
                    || name.members.count() != 1
                    || table.name_path_members(name.members).len() != 1
                    || row.root_symbol != name.symbol
                    || row.kind != kind
                    || borrow
                        .access_segments
                        .span(row.segments)
                        .is_none_or(|path| path != segments)
                {
                    return unsupported(
                        "computed shared borrow substituted or reordered an observation",
                    );
                }
                if kind.is_exclusive()
                    && rows
                        .iter()
                        .filter(|candidate| candidate.root_symbol == row.root_symbol)
                        .count()
                        != 1
                {
                    return unsupported("computed shared borrow aliases an exclusive observation");
                }
                position += 1;
            }
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|child| (child, false, false)),
            );
        }
    }
    if position != rows.len() {
        return unsupported("computed shared borrow has extra observation rows");
    }
    Ok(positions)
}
