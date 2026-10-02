use super::borrowed_initializer_source;
use checked_trees::CheckedUnitStructuralPathSegment as Segment;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn bare_reference_return_uses_exact_parameter_and_only_reduces_access() {
    let program = crate::front_end::typed_program(
        "data Node { value: i32; }
         data Other { value: i32; }
         machine shared(value: &mut Node) -> &Node { value }
         machine wrong(value: &mut Node) -> &Other { value }
         machine strengthen(value: &Node) -> &mut Node { value }
         machine unreadable(value: &write Node) -> &Node { value }",
    );
    for machine in program.machines() {
        let state = &program.machine_states(machine)[0];
        assert_eq!(
            super::source_parameter(&program, state),
            (machine.name.as_str() == "shared").then_some(0),
            "{}",
            machine.name.as_str(),
        );
    }
}

fn source() -> typed_trees::TypedTrees {
    crate::front_end::typed_program(
        "data Context { values: [i32; 2]; stored: &mut i32; }
         data Other { values: [i32; 2]; }
         machine value(context: &mut Context) -> &mut i32 { &mut context.values[0] }
         machine stored(context: &mut Context) -> &mut i32 { &mut context.stored }
         machine wrong(context: &mut Context) -> &mut u64 { &mut context.values[0] }",
    )
}

#[test]
fn borrowed_value_and_stored_reference_have_distinct_exact_paths() {
    let program = source();
    for (name, expected) in [
        (
            "value",
            vec![Segment::Field("values".into()), Segment::FixedIndex(0)],
        ),
        (
            "stored",
            vec![Segment::Field("stored".into()), Segment::Referent],
        ),
    ] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let StatementNode::Expression(expression) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("tail")
        };
        let source = borrowed_initializer_source(&program, state, expression, state.return_type)
            .expect("exact projected borrow");
        assert_eq!(source.path, expected);
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "wrong")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("tail")
    };
    assert!(borrowed_initializer_source(&program, state, expression, state.return_type).is_none());
}

#[test]
fn projected_reference_rejects_a_foreign_field_with_matching_spelling() {
    let mut program = source();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = program.machine_states(machine)[0].clone();
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("tail")
    };
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
        panic!("borrow")
    };
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(borrow.target)
    else {
        panic!("index")
    };
    let member = indexed.collection;
    let other = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap();
    let typed_trees::data::DataMember::Field(field) = &program.data_members(other)[0] else {
        panic!("field")
    };
    let foreign = field.symbol;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(member) else {
        panic!("member")
    };
    member.member_symbol = foreign;
    assert!(borrowed_initializer_source(&program, &state, expression, state.return_type).is_none());
}
