//! Expand an authored dispatch on the resolver's private syntax forest.
//! Its ordered arm statements share one evaluation of each runtime subject.
//! Earlier source consumers keep the original subjects and final pattern.
use arena::HandleSpan;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableDestructurePattern,
    UnaryOperator,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{
    DispatchArm, DispatchPattern, StatementHandle, StatementNode, TableDispatch, TableLocalData,
    TableTransition, TransitionGuardNode, TransitionPattern,
};

pub(super) fn prepare(syntax: &mut SyntaxTrees) {
    if !syntax.statements.has_dispatches() {
        return;
    }
    let bodies = syntax
        .items
        .statement_bodies()
        .filter(|(_, body)| {
            syntax.items.statements(*body).iter().any(|handle| {
                matches!(
                    syntax.statements.statement(*handle),
                    StatementNode::Dispatch(_)
                )
            })
        })
        .collect::<Vec<_>>();
    for (owner, body) in bodies {
        let statements = syntax.items.statements(body).to_vec();
        let mut prepared = HandleSpan::empty();
        for statement in statements {
            if let StatementNode::Dispatch(handle) = syntax.statements.statement(statement) {
                let dispatch = syntax.statements.dispatch(*handle).clone();
                expand(syntax, dispatch, &mut prepared);
            } else {
                prepared.push_contiguous(syntax.items.append_statement_handle(statement));
            }
        }
        syntax.items.set_statement_body(owner, prepared);
    }
}

fn expand(
    syntax: &mut SyntaxTrees,
    dispatch: TableDispatch,
    body: &mut HandleSpan<StatementHandle>,
) {
    let subjects = capture_subjects(syntax, &dispatch, body);
    let exhausts_booleans = covers_boolean_subjects(syntax, subjects.len(), &dispatch.arms);
    let arm_count = dispatch.arms.len();
    for (index, arm) in dispatch.arms.into_vec().into_iter().enumerate() {
        let last = index + 1 == arm_count;
        let (patterns, guard) = if last && exhausts_booleans {
            (HandleSpan::empty(), TransitionGuardNode::Always)
        } else {
            let patterns = arm
                .patterns
                .iter()
                .zip(&subjects)
                .filter_map(|(slot, &subject)| prepare_pattern(syntax, slot, subject))
                .collect::<Vec<_>>();
            (
                syntax.statements.insert_transition_patterns(patterns),
                arm.guard,
            )
        };
        let transition = syntax
            .statements
            .insert(StatementNode::Transition(TableTransition {
                patterns,
                guard,
                target: arm.target,
                proof_selectors: arm.proof_selectors(),
                source_span: arm.source_span,
                ends_dispatch: last,
                ..Default::default()
            }));
        body.push_contiguous(syntax.items.append_statement_handle(transition));
    }
}

fn capture_subjects(
    syntax: &mut SyntaxTrees,
    dispatch: &TableDispatch,
    body: &mut HandleSpan<StatementHandle>,
) -> Vec<ExpressionHandle> {
    let mut subjects = syntax
        .statements
        .expression_handles(dispatch.subjects)
        .to_vec();
    for (index, subject) in subjects.iter_mut().enumerate() {
        if closed_boolean(syntax, *subject) {
            continue;
        }
        // Source coordinates survive page partitioning and adoption.
        let name = Identifier::generated(format!(
            "__transition_subject#{}.{index}",
            dispatch
                .subjects_span
                .unwrap_or(dispatch.source_span)
                .span
                .start
        ));
        let capture = syntax
            .statements
            .insert(StatementNode::LocalData(TableLocalData {
                role: language_core::LocalBindingRole::DispatchSubject,
                name: name.clone(),
                initial_value: *subject,
                ..Default::default()
            }));
        body.push_contiguous(syntax.items.append_statement_handle(capture));
        let member = syntax.expressions.append_identifier_path_member(name);
        *subject = syntax.expressions.insert(ExpressionNode::Name {
            members: HandleSpan::from_parts(member, 1),
            application: None,
        });
    }
    subjects
}

fn prepare_pattern(
    syntax: &mut SyntaxTrees,
    slot: &DispatchPattern,
    subject: ExpressionHandle,
) -> Option<TransitionPattern> {
    let (is_structural, pattern) = match slot.pattern {
        MatchPattern::Wildcard => return None,
        MatchPattern::Destructure(pattern) => (true, pattern),
        MatchPattern::Value(value) => {
            let (head, guard) = if arm_pattern_is_condition(syntax, value) {
                (ExpressionHandle::invalid(), value)
            } else {
                (value, ExpressionHandle::invalid())
            };
            (
                false,
                TableDestructurePattern {
                    head,
                    guard,
                    has_rest: true,
                    ..Default::default()
                },
            )
        }
    };
    let capture = if slot.capture.is_valid() {
        syntax
            .expressions
            .set_transition_subject(slot.capture, subject);
        slot.capture
    } else {
        syntax.expressions.insert_transition_subject(subject)
    };
    Some(TransitionPattern {
        is_structural,
        pattern,
        subject,
        capture,
    })
}

fn arm_pattern_is_condition(syntax_trees: &SyntaxTrees, expression: ExpressionHandle) -> bool {
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => matches!(
            binary.operator,
            BinaryOperator::And
                | BinaryOperator::Or
                | BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
        ),
        ExpressionNode::Membership(_) => true,
        ExpressionNode::Unary(unary) => unary.operator == UnaryOperator::LogicalNot,
        _ => false,
    }
}

fn closed_boolean(syntax: &SyntaxTrees, subject: ExpressionHandle) -> bool {
    let expression = |handle| syntax.expressions.expression(handle);
    match expression(subject) {
        ExpressionNode::Boolean(_) => true,
        ExpressionNode::Binary(binary) => {
            binary.operator == BinaryOperator::Equal
                && matches!(expression(binary.left), ExpressionNode::Boolean(_))
                && matches!(expression(binary.right), ExpressionNode::Boolean(_))
        }
        _ => false,
    }
}

fn covers_boolean_subjects(syntax: &SyntaxTrees, count: usize, arms: &[DispatchArm]) -> bool {
    if !(1..=8).contains(&count) || (count == 1 && arms.len() != 2) {
        return false;
    }
    let mut rows = Vec::with_capacity(arms.len());
    for arm in arms {
        if arm.patterns.len() != count || !matches!(arm.guard, TransitionGuardNode::Always) {
            return false;
        }
        let mut mask = 0u32;
        let mut bits = 0u32;
        for (index, slot) in arm.patterns.iter().enumerate() {
            let value = match slot.pattern {
                MatchPattern::Wildcard => continue,
                MatchPattern::Value(value) => value,
                MatchPattern::Destructure(_) => return false,
            };
            let ExpressionNode::Boolean(value) = syntax.expressions.expression(value) else {
                return false;
            };
            mask |= 1 << index;
            bits |= u32::from(*value) << index;
        }
        rows.push((mask, bits));
    }
    (0..1 << count).all(|value| rows.iter().any(|&(mask, bits)| value & mask == bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_fixture::{Lexer, parse_syntax_trees};

    #[test]
    fn subject_is_evaluated_once_and_coverage_stops_at_each_dispatch() {
        let source = "machine caller() -> u32 {
            transition probe() { true -> 1u32, false -> 2u32 }
            transition probe() { false -> 3u32 }
        }";
        let mut syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
        prepare(&mut syntax);
        let machine = syntax
            .root_items()
            .find_map(|item| item.as_machine())
            .unwrap();
        let state = syntax
            .items
            .state(syntax.items.state_handles(machine.states)[0]);
        let handles = syntax.items.statements(state.statements);
        assert_eq!(handles.len(), 5);
        for &index in &[0, 3] {
            let StatementNode::LocalData(capture) = syntax.statements.statement(handles[index])
            else {
                panic!("one local per dispatch")
            };
            assert_eq!(
                capture.role,
                language_core::LocalBindingRole::DispatchSubject
            );
            assert!(matches!(
                syntax.expressions.expression(capture.initial_value),
                ExpressionNode::Call(_)
            ));
        }
        for (index, tested, ends) in [(1, true, false), (2, false, true), (4, true, true)] {
            let StatementNode::Transition(arm) = syntax.statements.statement(handles[index]) else {
                panic!("arm")
            };
            assert_eq!(!arm.patterns.is_empty(), tested);
            assert_eq!(arm.ends_dispatch, ends);
        }
    }

    #[test]
    fn trait_default_prepares_tuple_wildcards_and_preserves_an_uncovered_row() {
        let source = "trait Choose {
            machine choose(a: bool, b: bool) {
                transition (a, b) { (true, _) -> {}, (false, true) -> {}, (false, false) -> {} }
                transition (a, b) { (true, _) -> {}, (false, true) -> {} }
            }
        }";
        let mut syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
        prepare(&mut syntax);
        let (owner, body) = syntax.items.statement_bodies().next().unwrap();
        assert!(matches!(
            owner,
            syntax_trees::item::StatementBody::TraitDefault(_)
        ));
        let statements = syntax.items.statements(body);
        let branches: Vec<_> = statements
            .iter()
            .filter_map(|handle| match syntax.statements.statement(*handle) {
                StatementNode::Transition(arm) => Some((arm.patterns.len(), arm.ends_dispatch)),
                _ => None,
            })
            .collect();
        assert_eq!(
            branches,
            [(1, false), (2, false), (0, true), (1, false), (2, true)]
        );
        prepare(&mut syntax);
        assert_eq!(syntax.items.statement_bodies().next().unwrap().1, body);
    }
}
