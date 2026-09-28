use super::ExplicitStateCallees;
use crate::TypedTrees;
use crate::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableCallExpression, TableMatchArm,
    TableMatchExpression,
};
use crate::state::State;
use crate::statement::{
    StatementNode, TableCall, TableLocalData, TableNamePath, TableTransition, TransitionGuardNode,
    TransitionTargetNode,
};
use symbols::SymbolHandle;

fn symbol(ordinal: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(ordinal)
}

fn call(
    typed: &mut TypedTrees,
    target: SymbolHandle,
    receiver: ExpressionHandle,
    arguments: &[ExpressionHandle],
) -> ExpressionHandle {
    let arguments = typed
        .expression_table
        .insert_expression_handles(arguments.iter().copied());
    typed
        .expression_table
        .insert(ExpressionNode::Call(TableCallExpression {
            receiver,
            target_symbol: target,
            static_machine_parameter: SymbolHandle::invalid(),
            target: Default::default(),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            quotient_operation: None,
            private_layout_operation: None,
            layout_query: None,
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
        }))
}

fn state(typed: &mut TypedTrees, statements: impl IntoIterator<Item = StatementNode>) -> State {
    let mut state = State::default();
    for statement in statements {
        typed
            .statement_table
            .push_statement(&mut state.statement_nodes, statement);
    }
    state
}

#[test]
fn nested_receiver_and_argument_calls_keep_first_encounter_order() {
    let mut typed = TypedTrees::default();
    let argument = call(&mut typed, symbol(1), ExpressionHandle::invalid(), &[]);
    let receiver = call(
        &mut typed,
        symbol(2),
        ExpressionHandle::invalid(),
        &[argument],
    );
    let outer = call(&mut typed, symbol(3), receiver, &[argument, argument]);
    let arguments = typed.expression_table.insert_expression_handles([outer]);
    let state = state(
        &mut typed,
        [
            StatementNode::LocalData(TableLocalData {
                initial_value: outer,
                ..Default::default()
            }),
            StatementNode::Call(TableCall {
                target_symbol: symbol(3),
                arguments,
                ..Default::default()
            }),
        ],
    );
    let mut inspection = ExplicitStateCallees::default();
    assert_eq!(
        inspection.collect(&typed, &state),
        &[symbol(3), symbol(2), symbol(1)]
    );
    assert_eq!(
        inspection.visited.len(),
        3,
        "shared expression nodes are visited once"
    );
}

#[test]
fn transitions_include_guards_named_targets_arguments_and_continuations() {
    let mut typed = TypedTrees::default();
    let guard = call(&mut typed, symbol(1), ExpressionHandle::invalid(), &[]);
    let argument = call(&mut typed, symbol(3), ExpressionHandle::invalid(), &[]);
    let continuation = call(&mut typed, symbol(4), ExpressionHandle::invalid(), &[]);
    let arguments = typed.expression_table.insert_expression_handles([argument]);
    let target = typed
        .statement_table
        .insert_transition_target(TransitionTargetNode::Named {
            path: TableNamePath {
                symbol: symbol(2),
                ..Default::default()
            },
            static_machine_parameter: SymbolHandle::invalid(),
            arguments,
            evidence_arguments: Box::default(),
            source_span: Default::default(),
            authored_call_selection: None,
        });
    let continuation = typed
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(continuation));
    let state = state(
        &mut typed,
        [StatementNode::Transition(TableTransition {
            target,
            continuation,
            guard: TransitionGuardNode::When(guard),
            ..Default::default()
        })],
    );
    assert_eq!(
        ExplicitStateCallees::default().collect(&typed, &state),
        &[symbol(1), symbol(2), symbol(3), symbol(4)]
    );
}

#[test]
fn match_inspection_includes_patterns_and_every_arm_without_selecting_execution() {
    let mut typed = TypedTrees::default();
    let subject = typed.expression_table.insert(ExpressionNode::Boolean(true));
    let pattern = call(&mut typed, symbol(1), ExpressionHandle::invalid(), &[]);
    let first = call(&mut typed, symbol(2), ExpressionHandle::invalid(), &[]);
    let second = call(&mut typed, symbol(3), ExpressionHandle::invalid(), &[]);
    let arms = typed.expression_table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Value(pattern),
            value: first,
            ..Default::default()
        },
        TableMatchArm {
            value: second,
            ..Default::default()
        },
    ]);
    let dispatch = typed
        .expression_table
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject,
            arms,
        }));
    let state = state(&mut typed, [StatementNode::Expression(dispatch)]);
    assert_eq!(
        ExplicitStateCallees::default().collect(&typed, &state),
        &[symbol(1), symbol(2), symbol(3)]
    );
}

#[test]
fn reused_inspection_clears_previous_program_handles_and_ignores_zero_callees() {
    let mut inspection = ExplicitStateCallees::default();
    for ordinal in [7, 9] {
        let mut typed = TypedTrees::default();
        let called = call(
            &mut typed,
            symbol(ordinal),
            ExpressionHandle::invalid(),
            &[],
        );
        let zero = call(&mut typed, SymbolHandle::invalid(), called, &[]);
        let state = state(
            &mut typed,
            [
                StatementNode::Expression(zero),
                StatementNode::Expression(ExpressionHandle::invalid()),
            ],
        );
        assert_eq!(inspection.collect(&typed, &state), &[symbol(ordinal)]);
        assert!(inspection.collect(&typed, &State::default()).is_empty());
    }
}
