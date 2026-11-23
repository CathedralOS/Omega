use crate::expression::{ExpressionNode, ExpressionTable, TableCallExpression};
use crate::identifier::Identifier;
use crate::statement::{
    StatementNode, StatementTable, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use arena::HandleSpan;

#[test]
fn statement_table_stores_transition_payloads_as_handles() {
    let mut statements = StatementTable::new();
    let mut expressions = ExpressionTable::new();
    let values = [1, 2].map(|value| expressions.insert(ExpressionNode::Integer(
        numeric_literals::IntegerLiteral::from_value(value),
    )));
    let arguments = expressions.insert_expression_handles(values);
    let call = expressions.insert(ExpressionNode::Call(TableCallExpression {
        callee: Default::default(),
        receiver: Default::default(),
        target: Identifier::generated("next"),
        target_is_static: true,
        arguments,
        machine_arguments: Box::default(),
        evidence_arguments: Box::default(),
        operational_acknowledgement: language_core::CallOperationalAcknowledgement {
            acknowledges_block: true,
            ..Default::default()
        },
    }));
    let target = statements.insert_transition_target(TransitionTargetNode::Named(call));
    let guard = expressions.insert(ExpressionNode::Boolean(true));
    let statement = statements.insert(StatementNode::Transition(TableTransition {
        patterns: Default::default(),
        target,
        continuation: crate::statement::TransitionTargetHandle::invalid(),
        guard: TransitionGuardNode::When(guard),
        proof_selectors: HandleSpan::empty(),
        exit: Default::default(),
        ends_dispatch: true,
        source_span: Default::default(),
    }));

    let StatementNode::Transition(transition) = statements.statement(statement) else {
        panic!("statement should lower to a table transition");
    };
    let stored = statements.transition_target(transition.target);
    let bytes = record_storage::encode(stored, Default::default()).unwrap();
    let restored: TransitionTargetNode =
        record_storage::decode(&bytes, Default::default()).unwrap();
    let TransitionTargetNode::Named(retained) = restored else {
        panic!("record changed the named call form");
    };
    assert_eq!(retained, call);
    let call = expressions.call(retained);
    assert_eq!(expressions.expression_handles(call.arguments), values);
    assert!(call.operational_acknowledgement.acknowledges_block);
    assert!(!call.operational_acknowledgement.acknowledges_suspend);
}
