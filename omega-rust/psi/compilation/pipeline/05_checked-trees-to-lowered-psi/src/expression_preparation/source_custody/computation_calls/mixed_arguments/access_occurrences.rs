//! Replay ordered observations using the same authored places as call operands.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{BorrowAccessKind, BorrowCallFact, CheckedTrees};

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;

/// Each result is the first access-row position of an authored formal actual.
/// Scalar actuals may contribute zero, one, or several observation rows.
///
/// A row may also end before a later row begins: a copied read ends at once,
/// and a loan formed inside a nested call that returns a plain copy ends when
/// that call returns (`borrow/accesses/collection.rs` in stage 04). Ended rows
/// no longer alias a later exclusive loan on the same root; a loan held
/// through invocation still aliases every row that is live beside it.
pub(crate) fn rejoin(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
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
        // Each pending expression carries the access its own named place
        // records: a read, unless a nested borrow forms a loan of that place.
        let mut pending = vec![(*argument, true, false, BorrowAccessKind::Read)];
        let mut active = Vec::new();
        while let Some((expression, direct_argument, exiting, access)) = pending.pop() {
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
            pending.push((expression, direct_argument, true, access.clone()));
            let mut children = Vec::new();
            let mut children_access = BorrowAccessKind::Read;
            let (named, kind) = match table.expression(expression) {
                ExpressionNode::Borrow(borrow) if direct_argument => {
                    (Some(borrow.target), borrow_access_kind(borrow.access))
                }
                ExpressionNode::Name(_) | ExpressionNode::StructLiteral(_)
                    if validation::scalar_case_constructor(checked, expression).is_some() =>
                {
                    let constructor = validation::scalar_case_constructor(checked, expression)
                        .ok_or(LoweringError::Unsupported(
                            "computed case lost its source constructor",
                        ))?;
                    children.extend(
                        constructor
                            .fields
                            .into_iter()
                            .map(|(_, expression, _)| expression),
                    );
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Name(_) => (Some(expression), access),
                ExpressionNode::Member(_) => (Some(expression), access),
                // This follows borrow/accesses/read.rs: a nested call's own
                // borrow formation also has a separate call row. In the
                // enclosing roster the borrowed place is a loan with the
                // borrow's access, followed by its selector's reads.
                ExpressionNode::Borrow(borrow) => {
                    children.push(borrow.target);
                    children_access = borrow_access_kind(borrow.access);
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
                    // A resolved static qualifier names the callee's owner;
                    // it is not storage observed while evaluating the call.
                    // Unknown selections keep the receiver on the replay path.
                    let no_runtime_receiver = checked
                        .machine_holding_state(call.target_symbol)
                        .and_then(|owner| {
                            checked
                                .machine_states(owner)
                                .iter()
                                .find(|state| state.symbol == call.target_symbol)
                                .map(|state| {
                                    checked.call_has_no_runtime_receiver(call, owner, state)
                                })
                        })
                        .unwrap_or(false);
                    if call.receiver.is_valid() && !no_runtime_receiver {
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
                ExpressionNode::ArrayLiteral(elements) => {
                    let values = table.expression_handles(*elements);
                    if values.len() != elements.count() as usize {
                        return unsupported("computed array has stale scalar observations");
                    }
                    children.extend_from_slice(values);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::StructLiteral(literal) => {
                    let fields = table.struct_fields(literal.fields);
                    if fields.len() != literal.fields.count() as usize {
                        return unsupported("computed construction has stale field observations");
                    }
                    children.extend(fields.iter().map(|field| field.value));
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Indexed(indexed)
                    if is_array_value_projection(checked, indexed.collection) =>
                {
                    // Admitted array-value projections have only closed literal
                    // leaves, so the collection contributes no observations.
                    // Array source replay independently validates every sibling;
                    // only the selector contributes reads here.
                    children.push(indexed.index);
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Indexed(indexed) => {
                    // The captured place is one observation, followed by this
                    // selector's reads (or range endpoints). Its full path
                    // already includes nested collection projections, so do
                    // not invent extra reads by walking that collection again.
                    children.push(indexed.index);
                    (Some(expression), access)
                }
                ExpressionNode::Range(range) => {
                    if range.start.is_valid() {
                        children.push(range.start);
                    }
                    if range.end.is_valid() {
                        children.push(range.end);
                    }
                    (None, BorrowAccessKind::Read)
                }
                ExpressionNode::Boolean(_)
                | ExpressionNode::Integer(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::String(_)
                | ExpressionNode::ZeroValue(_) => (None, BorrowAccessKind::Read),
            };
            if let Some(named) = named {
                if !table.expression_is_valid(named) {
                    return unsupported("computed shared borrow has a stale named observation");
                }
                // An attached self field can be captured at the field's root,
                // unlike a projection from an ordinary parameter. Reuse the
                // source rejoin instead of guessing capture identity from the
                // spelling of the final Name/Member chain.
                let captured = if matches!(table.expression(named), ExpressionNode::Indexed(indexed)
                    if matches!(table.expression(indexed.index), ExpressionNode::Range(_)))
                {
                    super::view_subslices::captured_range(checked, named)?
                } else {
                    crate::emission::call_source_custody::projected_receivers::source(
                        checked,
                        machine,
                        state,
                        statement as usize,
                        named,
                    )?
                    .captured_place()
                    .clone()
                };
                let row = rows.get(position).ok_or(LoweringError::Unsupported(
                    "computed shared borrow omits an authored observation",
                ))?;
                if !captured.root_symbol.is_valid()
                    || row.root_symbol != captured.root_symbol
                    || row.kind != kind
                    || borrow
                        .access_segments
                        .span(row.segments)
                        .is_none_or(|path| path != captured.segments)
                {
                    return unsupported(
                        "computed shared borrow substituted or reordered an observation",
                    );
                }
                // An observation ends after it begins and within this roster.
                // A direct borrow actual is delivered to the invocation, so
                // its loan never ends during operand evaluation.
                if row
                    .end_before_access
                    .is_some_and(|end| end <= position || end > rows.len())
                    || (direct_argument
                        && matches!(table.expression(expression), ExpressionNode::Borrow(_))
                        && row.end_before_access.is_some())
                {
                    return unsupported("computed shared borrow ends an observation out of order");
                }
                // The same rule as stage 04's roster rejoin: an exclusive loan
                // held through invocation is the only live row on its root.
                if kind.is_exclusive()
                    && row.end_before_access.is_none()
                    && rows
                        .iter()
                        .filter(|candidate| {
                            candidate.root_symbol == row.root_symbol
                                && !candidate
                                    .end_before_access
                                    .is_some_and(|end| end <= position)
                        })
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
                    .map(|child| (child, false, false, children_access.clone())),
            );
        }
    }
    if position != rows.len() {
        return unsupported("computed shared borrow has extra observation rows");
    }
    Ok(positions)
}

fn borrow_access_kind(access: language_core::ReferenceAccess) -> BorrowAccessKind {
    match access {
        language_core::ReferenceAccess::Shared => BorrowAccessKind::Read,
        language_core::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
        language_core::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
    }
}

fn is_array_value_projection(checked: &CheckedTrees, mut expression: ExpressionHandle) -> bool {
    let table = &checked.expression_table;
    let mut visited = Vec::new();
    while table.expression_is_valid(expression) && !visited.contains(&expression) {
        visited.push(expression);
        match table.expression(expression) {
            ExpressionNode::ArrayLiteral(_) => return true,
            ExpressionNode::Indexed(indexed) => expression = indexed.collection,
            _ => return false,
        }
    }
    false
}
