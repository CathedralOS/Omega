//! Expression-label arm shapes the call-contract and domain renderers share:
//! every arm below renders identically given a child labeler. Arms whose
//! label carries renderer-specific context (names, struct literals, the
//! `result` binder) stay with their own match.

use arena::HandleSpan;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, MatchPattern, TableAtomicExpression, TableBinaryExpression,
    TableBorrowExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMatchExpression, TableMemberExpression, TableRangeExpression, TableUnaryExpression,
    display_name_path,
};

pub(super) fn match_label(
    program: &TypedTrees,
    dispatch: &TableMatchExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    let arms = program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .map(|arm| {
            let pattern = match arm.pattern {
                MatchPattern::Value(value) => render(value),
                MatchPattern::Wildcard => "_".to_owned(),
            };
            format!("{pattern} -> {}", render(arm.value))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("match {} {{ {arms} }}", render(dispatch.subject))
}

pub(super) fn atomic_label(
    atomic: &TableAtomicExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    format!("atomic[{:?}]({})", atomic.ordering, render(atomic.value))
}

pub(super) fn array_literal_label(
    program: &TypedTrees,
    values: HandleSpan<ExpressionHandle>,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    let values = program
        .expression_table
        .expression_handles(values)
        .iter()
        .map(|value| render(*value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

pub(super) fn binary_label(
    binary: &TableBinaryExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    format!(
        "{} {} {}",
        render(binary.left),
        binary.operator.display_name(),
        render(binary.right)
    )
}

pub(super) fn cast_label(
    program: &TypedTrees,
    cast: &TableCastExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    format!(
        "{} as {}",
        render(cast.value),
        display_name_path(
            program
                .expression_table
                .name_path_members(cast.target_label),
            "::",
        )
    )
}

pub(super) fn call_label(
    program: &TypedTrees,
    call: &TableCallExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .map(|argument| render(*argument))
        .collect::<Vec<_>>()
        .join(", ");
    if call.receiver.is_valid() {
        format!("{}.{}({arguments})", render(call.receiver), call.target)
    } else {
        format!("{}({arguments})", call.target)
    }
}

pub(super) fn indexed_label(
    indexed: &TableIndexedExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    format!("{}[{}]", render(indexed.collection), render(indexed.index))
}

pub(super) fn member_label(
    member: &TableMemberExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    format!("{}.{}", render(member.receiver), member.member)
}

pub(super) fn borrow_label(
    inner: &TableBorrowExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    let target = render(inner.target);
    match inner.access {
        language_semantics::ReferenceAccess::Shared => target,
        language_semantics::ReferenceAccess::Mutable => format!("mut {target}"),
        language_semantics::ReferenceAccess::WriteOnly => format!("write {target}"),
    }
}

pub(super) fn range_label(
    range: &TableRangeExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    match (range.start.is_valid(), range.end.is_valid()) {
        (true, true) => format!("{}..{}", render(range.start), render(range.end)),
        (true, false) => format!("{}..", render(range.start)),
        (false, true) => format!("..{}", render(range.end)),
        (false, false) => "..".to_owned(),
    }
}

pub(super) fn unary_label(
    unary: &TableUnaryExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    format!("{}{}", unary.operator.display_name(), render(unary.operand))
}
