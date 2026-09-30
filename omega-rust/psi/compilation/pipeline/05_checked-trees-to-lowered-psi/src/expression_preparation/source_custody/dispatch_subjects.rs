//! Closed-case dispatch subjects as the parser spells them, re-read from
//! source independently of the checker's plan.
//!
//! A transition dispatch over a case sum is parsed into one guarded arm per
//! case. Each guard is an equality between the dispatch subject and a variant
//! name. When the subject is a place, every arm names that place. When it is
//! a call, the parser copies the call into every guard and every payload read
//! (`copy.field` qualified by the arm's case). The copies are separate
//! expression nodes that repeat the same call, operands and authored
//! selection occurrence. The language evaluates that call once, at the first
//! arm (wiki/spec/language/expressions.md), so every other copy reads that
//! one result.
use checked_trees::CheckedTrees;
use checked_trees::data::DataMember;
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::statement::{StatementNode, TransitionGuardNode};

/// The subject and variant a case guard tests, looking through the parser's
/// `true == ..` wrapping.
pub(crate) fn case_test(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Option<(ExpressionHandle, symbols::SymbolHandle)> {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != BinaryOperator::Equal {
        return None;
    }
    if matches!(
        checked.expression_table.expression(binary.left),
        ExpressionNode::Boolean(true)
    ) {
        return case_test(checked, binary.right);
    }
    if matches!(
        checked.expression_table.expression(binary.right),
        ExpressionNode::Boolean(true)
    ) {
        return case_test(checked, binary.left);
    }
    for (subject, candidate) in [(binary.left, binary.right), (binary.right, binary.left)] {
        if let ExpressionNode::Name(path) = checked.expression_table.expression(candidate)
            && checked.data_definitions().iter().any(|definition| {
                checked.data_members(definition).iter().any(|member|
                matches!(member, DataMember::Variant(variant) if variant.symbol == path.symbol))
            })
        {
            return Some((subject, path.symbol));
        }
    }
    None
}

/// The call a case guard tests, when the dispatch subject is a call whose
/// operands evaluate no call of their own. Every copy repeats those
/// operands, so a nested call would be numbered once per arm while running
/// once; the checker plans no such dispatch and this replay admits none.
pub(crate) fn computed_case_subject(
    checked: &CheckedTrees,
    guard: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let (subject, _) = case_test(checked, guard)?;
    let table = &checked.expression_table;
    let ExpressionNode::Call(call) = table.expression(subject) else {
        return None;
    };
    let operands = table.expression_handles(call.arguments);
    if !repeats_subject(checked, subject, subject)
        || operands.len() != call.arguments.count() as usize
        || (call.receiver.is_valid() && !call_free(checked, call.receiver))
        || !operands.iter().all(|operand| call_free(checked, *operand))
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
    checked: &CheckedTrees,
    subject: ExpressionHandle,
    copy: ExpressionHandle,
) -> bool {
    let table = &checked.expression_table;
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
    checked: &CheckedTrees,
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
        computed_case_subject(checked, guard)
    };
    let subject = tested(statements.get(index)?)?;
    (!statements[..index].iter().any(|statement| {
        tested(statement).is_some_and(|earlier| repeats_subject(checked, subject, earlier))
    }))
    .then_some(subject)
}

/// Whether `expression` reads names, literals and places through operators
/// alone, invoking no call.
fn call_free(checked: &CheckedTrees, expression: ExpressionHandle) -> bool {
    let table = &checked.expression_table;
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
