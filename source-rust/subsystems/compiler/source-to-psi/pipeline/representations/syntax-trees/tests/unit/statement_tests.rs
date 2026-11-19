use crate::expression::{ExpressionNode, ExpressionTable};
use crate::identifier::Identifier;
use crate::statement::{
    StatementNode, StatementTable, TableTransition, TransitionGuardNode, TransitionTargetNode,
};
use arena::HandleSpan;

#[test]
fn statement_table_stores_transition_payloads_as_handles() {
    let mut statements = StatementTable::new();
    let mut expressions = ExpressionTable::new();
    let path_start = statements.append_identifier_path_member(Identifier::generated("next"));
    let path = HandleSpan::from_parts(path_start, 1);
    let argument_one = expressions.insert(ExpressionNode::Integer(
        numeric_literals::IntegerLiteral::from_value(1),
    ));
    let argument_one = statements.append_expression_handle(argument_one);
    let argument_two = expressions.insert(ExpressionNode::Integer(
        numeric_literals::IntegerLiteral::from_value(2),
    ));
    let _argument_two = statements.append_expression_handle(argument_two);
    let arguments = HandleSpan::from_parts(argument_one, 2);
    let target = statements.insert_transition_target(TransitionTargetNode::Named {
        operational_acknowledgement: Default::default(),
        path,
        arguments,
        evidence_arguments: Box::default(),
        source_span: Default::default(),
    });
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
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = statements.transition_target(transition.target)
    else {
        panic!("transition target should be named");
    };

    assert_eq!(path.count(), 1);
    assert_eq!(arguments.count(), 2);
    assert_eq!(expressions.expression_count(), 3);

    let mut acknowledged = statements.transition_target(target).clone();
    let TransitionTargetNode::Named {
        operational_acknowledgement,
        ..
    } = &mut acknowledged
    else {
        panic!("named target");
    };
    operational_acknowledgement.acknowledges_block = true;
    let bytes = record_storage::encode(&acknowledged, Default::default()).unwrap();
    let restored: TransitionTargetNode =
        record_storage::decode(&bytes, Default::default()).unwrap();
    let TransitionTargetNode::Named {
        operational_acknowledgement,
        path: restored_path,
        arguments: restored_arguments,
        ..
    } = restored
    else {
        panic!("record changed the named call form");
    };
    assert!(operational_acknowledgement.acknowledges_block);
    assert!(!operational_acknowledgement.acknowledges_suspend);
    assert_eq!(restored_path, *path);
    assert_eq!(restored_arguments, *arguments);
}
