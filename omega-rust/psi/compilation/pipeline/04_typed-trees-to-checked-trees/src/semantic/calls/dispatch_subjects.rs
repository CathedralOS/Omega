//! The call a closed-case dispatch evaluates once.
//!
//! The parser spells `transition f(x) { Case::A { v } -> .. Case::B -> .. }`
//! as one guarded arm per case. Each guard compares a copy of `f(x)` with its
//! variant, and each payload read is a case-qualified member of another copy.
//! The copies are separate expression nodes that repeat the same call,
//! operands and authored selection occurrence. The language evaluates the
//! call once, at the first arm (wiki/spec/language/expressions.md), so each
//! copy names that one result rather than a call site of its own.
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode};

/// The authored call whose saved result a case guard tests. Ordinary call
/// operand preparation owns its nested invocations and evaluation order.
pub(crate) fn computed_case_subject(
    program: &TypedTrees,
    guard: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let (subject, _) = crate::proof::exact_outcome_case_test(program, guard)?;
    repeats_subject(program, subject, subject).then_some(subject)
}

/// Whether `copy` repeats the computed `subject`: the same call and operands
/// carrying exactly the subject's nonempty authored selection occurrence.
/// Source spans are diagnostic only, so a separately authored call of the
/// same shape keeps its own occurrence and stays a separate evaluation.
pub(crate) fn repeats_subject(
    program: &TypedTrees,
    subject: ExpressionHandle,
    copy: ExpressionHandle,
) -> bool {
    let table = &program.expression_table;
    if !table.expression_is_valid(subject)
        || !table.expression_is_valid(copy)
        || !matches!(table.expression(subject), ExpressionNode::Call(_))
        || !matches!(table.expression(copy), ExpressionNode::Call(_))
        || !table.expressions_structurally_equal(subject, copy)
    {
        return false;
    }
    let mut occurrences = table.authored_selection_occurrences(subject).peekable();
    occurrences.peek().is_some() && occurrences.eq(table.authored_selection_occurrences(copy))
}

/// The computed subject statement `index` evaluates: the subject of its
/// guard when no earlier transition of `statements` tests a copy of it.
pub(crate) fn evaluated_case_subject(
    program: &TypedTrees,
    statements: &[StatementNode],
    index: usize,
) -> Option<ExpressionHandle> {
    let tested = |statement: &StatementNode| {
        let StatementNode::Transition(transition) = statement else {
            return None;
        };
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        computed_case_subject(program, guard)
    };
    let subject = tested(statements.get(index)?)?;
    (!statements[..index].iter().any(|statement| {
        tested(statement).is_some_and(|earlier| repeats_subject(program, subject, earlier))
    }))
    .then_some(subject)
}
