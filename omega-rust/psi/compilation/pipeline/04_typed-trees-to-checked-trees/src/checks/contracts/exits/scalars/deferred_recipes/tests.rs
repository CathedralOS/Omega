use super::closed_terms_identical;
use numerics::literals::IntegerLiteral;
use typed_trees::{
    TypedTrees,
    expression::{ExpressionNode, StaticMachineArgument, TableCallExpression},
};

fn call() -> TableCallExpression {
    TableCallExpression {
        authored_call_binding: None,
        build_selection: Default::default(),
        authored_machine_application: Default::default(),
        exact_coercion_target: Default::default(),
        requirement_application: Default::default(),
        receiver: Default::default(),
        target_symbol: symbols::SymbolHandle::from_arena_index(1),
        static_machine_parameter: Default::default(),
        target: typed_trees::name::Identifier::generated("closed"),
        static_requirement_dispatch: None,
        machine_arguments: Box::new([]),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        native_code_layout: None,
        arguments: Default::default(),
        evidence_arguments: Box::new([]),
        operational_acknowledgement: Default::default(),
    }
}

/// Runtime call shape omits proof-static arguments. It therefore cannot prove
/// equal denotation, either before specialization or after operands are erased
/// into their retained static application identities.
#[test]
fn identical_runtime_calls_do_not_identify_distinct_static_denotations() {
    let mut program = TypedTrees::default();
    let mut first = call();
    first.machine_arguments = vec![StaticMachineArgument {
        const_literal: Some(IntegerLiteral::from_value(8)),
        ..Default::default()
    }]
    .into_boxed_slice();
    let mut second = first.clone();
    second.machine_arguments[0].const_literal = Some(IntegerLiteral::from_value(6));
    let first = program.expression_table.insert(ExpressionNode::Call(first));
    let second = program
        .expression_table
        .insert(ExpressionNode::Call(second));
    assert!(
        program
            .expression_table
            .expressions_structurally_equal(first, second)
    );
    assert!(!closed_terms_identical(&program, first, second, 0));

    let mut first = call();
    first.authored_machine_application = arena::Handle::from_arena_index(1);
    let mut second = first.clone();
    second.authored_machine_application = arena::Handle::from_arena_index(2);
    let same = program
        .expression_table
        .insert(ExpressionNode::Call(first.clone()));
    let first = program.expression_table.insert(ExpressionNode::Call(first));
    let second = program
        .expression_table
        .insert(ExpressionNode::Call(second));
    assert!(
        program
            .expression_table
            .expressions_structurally_equal(first, second)
    );
    assert!(!closed_terms_identical(&program, first, second, 0));
    assert!(closed_terms_identical(&program, first, same, 0));
}
