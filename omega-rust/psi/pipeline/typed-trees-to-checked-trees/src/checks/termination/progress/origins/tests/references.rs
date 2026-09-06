use super::*;

#[test]
fn a_reference_observes_the_current_value_not_its_declaration_snapshot() {
    for (statements, expected) in [
        ("let borrowed: &Context = &context;", "context"),
        (
            "let borrowed: &Context = &context; context.scheduler = replacement.scheduler;",
            "replacement",
        ),
    ] {
        let fixture = Fixture::new(statements, "borrowed.scheduler");
        assert_eq!(
            fixture.query(fixture.subject("borrowed", &[("Context", "scheduler")])),
            Some(fixture.subject(expected, &[("Context", "scheduler")]))
        );
    }
}

#[test]
fn a_reference_to_a_field_retains_its_full_storage_projection() {
    let fixture = Fixture::new(
        "let borrowed: &SchedulerHandle = &context.scheduler;
         context.scheduler = replacement.scheduler;",
        "borrowed",
    );
    assert_eq!(
        fixture.query(fixture.subject("borrowed", &[])),
        Some(fixture.subject("replacement", &[("Context", "scheduler")]))
    );
}

#[test]
fn constrained_reference_aliases_use_the_same_live_origin_query() {
    let mut fixture = Fixture::new("let borrowed: &Context = &context;", "borrowed.scheduler");
    let subject = fixture.subject("borrowed", &[("Context", "scheduler")]);
    let expected = fixture.subject("context", &[("Context", "scheduler")]);
    assert_eq!(fixture.query(subject.clone()), Some(expected.clone()));
    let state =
        crate::semantic_calls::find_state(&fixture.program, fixture.state.state_symbol).unwrap();
    let statements = state.statement_nodes;
    let StatementNode::LocalData(local) =
        &fixture.program.statement_table.statements(statements)[0]
    else {
        panic!("alias declaration")
    };
    let base_type = local.type_reference;
    let constrained = fixture
        .program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type,
            constraints: HandleSpan::empty(),
        });
    let StatementNode::LocalData(local) =
        &mut fixture.program.statement_table.statements_mut(statements)[0]
    else {
        unreachable!()
    };
    local.type_reference = constrained;
    assert_eq!(fixture.query(subject), Some(expected));
}

#[test]
fn a_foreign_same_spelling_root_cannot_supply_a_reference_origin() {
    let mut fixture = Fixture::new("let borrowed: &Context = &context;", "borrowed.scheduler");
    let subject = fixture.subject("borrowed", &[("Context", "scheduler")]);
    assert_eq!(
        fixture.query(subject.clone()),
        Some(fixture.subject("context", &[("Context", "scheduler")]))
    );
    let foreign = fixture
        .program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe_scheduler")
        .unwrap();
    let foreign = fixture
        .program
        .state_parameters(&fixture.program.machine_states(foreign)[0])[0]
        .symbol;
    let state =
        crate::semantic_calls::find_state(&fixture.program, fixture.state.state_symbol).unwrap();
    let StatementNode::LocalData(local) = &fixture
        .program
        .statement_table
        .statements(state.statement_nodes)[0]
    else {
        panic!("alias declaration")
    };
    let typed_trees::expression::ExpressionNode::Borrow(borrow) = fixture
        .program
        .expression_table
        .expression(local.initial_value)
    else {
        panic!("explicit borrow")
    };
    let target = borrow.target;
    let typed_trees::expression::ExpressionNode::Name(name) =
        fixture.program.expression_table.expression_mut(target)
    else {
        panic!("retained source name")
    };
    // Keep the authored `context` spelling; only its nominal root is foreign.
    name.head_symbol = foreign;
    name.symbol = foreign;
    assert_eq!(fixture.query(subject), None);
}
