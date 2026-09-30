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

/// The call a case guard tests, when the dispatch subject is a call whose
/// operands evaluate no call of their own. Every copy repeats those
/// operands, so a nested call would appear once per arm while running once;
/// no planner accounts for that yet.
pub(crate) fn computed_case_subject(
    program: &TypedTrees,
    guard: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let (subject, _) = crate::proof::exact_outcome_case_test(program, guard)?;
    let table = &program.expression_table;
    let ExpressionNode::Call(call) = table.expression(subject) else {
        return None;
    };
    if table.authored_selection_occurrences(subject).len() == 0
        || (call.receiver.is_valid() && !call_free(program, call.receiver))
        || !table
            .expression_handles(call.arguments)
            .iter()
            .all(|argument| call_free(program, *argument))
    {
        return None;
    }
    Some(subject)
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

/// Whether `expression` reads names, literals and places through operators
/// alone, invoking no call.
fn call_free(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let table = &program.expression_table;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if !table.expression_is_valid(expression) {
            return false;
        }
        match table.expression(expression) {
            ExpressionNode::Name(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Indexed(indexed) => {
                pending.extend([indexed.collection, indexed.index]);
            }
            _ => return false,
        }
    }
    true
}
