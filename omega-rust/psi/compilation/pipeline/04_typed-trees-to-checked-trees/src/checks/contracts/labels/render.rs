//! Expression-label arm shapes the call-contract and domain renderers share:
//! every arm below renders identically given a child labeler. Arms whose
//! label carries renderer-specific context (names, struct literals, the
//! `result` binder) stay with their own match.

use arena::HandleSpan;
use typed_trees::expression::{
    ExpressionHandle, MatchPattern, TableAtomicExpression, TableBinaryExpression,
    TableBorrowExpression, TableCallExpression, TableCastExpression, TableIndexedExpression,
    TableMatchExpression, TableMemberExpression, TableRangeExpression, TableUnaryExpression,
    display_name_path,
};

pub(super) fn match_label(
    program: &dyn ::validation::CheckingInput,
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
                MatchPattern::Destructure(_) => typed_trees::expression::display_match_pattern(
                    &program.expression_table,
                    arm.pattern,
                    render,
                ),
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    call: &TableCallExpression,
    render: &dyn Fn(ExpressionHandle) -> String,
) -> String {
    typed_trees::expression::display_call_label(&program.expression_table, call, render)
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

/// The full expression-label dispatcher the call-contract and domain renderers
/// share: every arm renders identically except `Name` (whose binder rules
/// differ by caller) and `StructLiteral`, which the caller closures supply.
pub(super) fn expression_label(
    program: &dyn ::validation::CheckingInput,
    node: &typed_trees::expression::ExpressionNode,
    render: &dyn Fn(ExpressionHandle) -> String,
    name_label: &dyn Fn(&typed_trees::expression::TableNamePath) -> String,
    struct_label: &dyn Fn(&typed_trees::expression::TableStructLiteral) -> String,
) -> String {
    match node {
        typed_trees::expression::ExpressionNode::SavedSubject(saved) => format!(
            "$saved-subject#{}:{}",
            saved.capture.arena_index(),
            saved.capture.generation()
        ),
        typed_trees::expression::ExpressionNode::Match(dispatch) => {
            match_label(program, dispatch, render)
        }
        typed_trees::expression::ExpressionNode::Atomic(atomic) => atomic_label(atomic, render),
        typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            array_literal_label(program, *values, render)
        }
        typed_trees::expression::ExpressionNode::Binary(binary) => binary_label(binary, render),
        typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::Cast(cast) => cast_label(program, cast, render),
        typed_trees::expression::ExpressionNode::Call(call) => call_label(program, call, render),
        typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::UndeterminedNaN(format) => {
            format!("<{} NaN without payload>", format.name())
        }
        typed_trees::expression::ExpressionNode::Indexed(indexed) => indexed_label(indexed, render),
        typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
        typed_trees::expression::ExpressionNode::Member(member) => member_label(member, render),
        typed_trees::expression::ExpressionNode::Borrow(inner) => borrow_label(inner, render),
        typed_trees::expression::ExpressionNode::Name(path) => name_label(path),
        typed_trees::expression::ExpressionNode::Range(range) => range_label(range, render),
        typed_trees::expression::ExpressionNode::StructLiteral(literal) => struct_label(literal),
        typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
        typed_trees::expression::ExpressionNode::Unary(unary) => unary_label(unary, render),
        typed_trees::expression::ExpressionNode::ZeroValue(type_reference) => format!(
            "zero_value<{}>()",
            program.display_type_reference(*type_reference)
        ),
    }
}
