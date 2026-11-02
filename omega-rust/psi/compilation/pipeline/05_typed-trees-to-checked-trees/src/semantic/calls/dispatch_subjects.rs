//! The call a closed-case dispatch evaluates once.
//!
//! The parser spells `transition f(x) { Case::A { v } -> .. Case::B -> .. }`
//! as one guarded arm per case. Each guard compares a copy of `f(x)` with its
//! variant, and each payload read is a case-qualified member of another copy.
//! The copies are separate expression nodes that repeat the same call,
//! operands and authored selection occurrence. The language evaluates the
//! call once, at the first arm (wiki/spec/language/expressions.md), so each
//! copy names that one result rather than a call site of its own.
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode};

/// The authored call whose saved result a case guard tests. Ordinary call
/// operand preparation owns its nested invocations and evaluation order.
pub(crate) fn computed_case_subject(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    statements: &[StatementNode],
    index: usize,
) -> Option<ExpressionHandle> {
    let tested = |statement: &StatementNode| {
        let transition = statement.as_transition()?;
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

/// The dispatch roles of the calls one transition statement spells: the
/// computed subject it evaluates when it is the dispatch's first arm, and
/// every call that reads the saved result instead of running, which is each
/// copy of the subject in its guard or arm and each call inside such a copy.
#[derive(Default)]
pub(crate) struct StatementDispatchCalls {
    pub(crate) evaluation: ExpressionHandle,
    pub(crate) saved_reads: Vec<ExpressionHandle>,
}

impl StatementDispatchCalls {
    /// The role of the call expression `expression` in this statement.
    pub(crate) fn role(&self, expression: ExpressionHandle) -> checked_trees::FlowCallDispatchRole {
        if !expression.is_valid() {
            checked_trees::FlowCallDispatchRole::Ordinary
        } else if expression == self.evaluation {
            checked_trees::FlowCallDispatchRole::SubjectEvaluation
        } else if self.saved_reads.contains(&expression) {
            checked_trees::FlowCallDispatchRole::SavedSubjectRead
        } else {
            checked_trees::FlowCallDispatchRole::Ordinary
        }
    }
}

/// Classify the calls statement `index` spells against the computed case
/// subject its guard tests. A statement whose guard tests no computed subject
/// spells only ordinary calls.
pub(crate) fn statement_dispatch_calls(
    program: &dyn ::validation::CheckingInput,
    statements: &[StatementNode],
    index: usize,
) -> StatementDispatchCalls {
    let mut calls = StatementDispatchCalls::default();
    let Some(statement @ StatementNode::Transition(transition)) = statements.get(index) else {
        return calls;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return calls;
    };
    let Some(subject) = computed_case_subject(program, guard) else {
        return calls;
    };
    if evaluated_case_subject(program, statements, index) == Some(subject) {
        calls.evaluation = subject;
    }
    let table = &program.expression_table;
    let mut pending = program
        .tables
        .expression_roots(statement)
        .into_iter()
        .map(|(_, root)| (root, false))
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    while let Some((expression, inside_copy)) = pending.pop() {
        if !table.expression_is_valid(expression) || visited.contains(&(expression, inside_copy)) {
            continue;
        }
        visited.push((expression, inside_copy));
        let node = table.expression(expression);
        let copy = inside_copy
            || (expression != calls.evaluation && repeats_subject(program, subject, expression));
        if copy
            && matches!(node, ExpressionNode::Call(_))
            && !calls.saved_reads.contains(&expression)
        {
            calls.saved_reads.push(expression);
        }
        validation::expression_traversal::children(program, node, |child| {
            pending.push((child, copy));
        });
    }
    calls
}
