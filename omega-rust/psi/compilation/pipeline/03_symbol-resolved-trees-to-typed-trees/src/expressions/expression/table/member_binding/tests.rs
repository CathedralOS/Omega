use super::declared_field;
use crate::lower_symbol_resolved_trees;
use symbol_resolved_trees as resolved;

const PROJECTED_MEASURES: &str = "
    data First { remaining: u64; }
    data Second { remaining: u64; }
    measure First::Remaining(value: First) -> u64 { value.remaining }
    measure Second::Remaining(value: Second) -> u64 { value.remaining }
";

fn projected_member(
    program: &resolved::SymbolResolvedTrees,
    measure_position: usize,
) -> &resolved::expression::TableMemberExpression {
    let expressions = &program.tables.bodies.expressions;
    let body = expressions.expression_handles(program.measures[measure_position].body)[0];
    let resolved::expression::ExpressionNode::Member(member) = expressions.expression(body) else {
        panic!("measure field projection");
    };
    member
}

fn typed_member(
    program: &typed_trees::TypedTrees,
    measure_position: usize,
) -> &typed_trees::expression::TableMemberExpression {
    let expressions = &program.expression_table;
    let body = expressions.expression_handles(program.measures()[measure_position].body)[0];
    let typed_trees::expression::ExpressionNode::Member(member) = expressions.expression(body)
    else {
        panic!("typed measure field projection");
    };
    member
}

#[test]
fn measure_projection_preserves_conflicting_selector_for_validation() {
    let mut program = crate::front_end::resolved_program(PROJECTED_MEASURES);
    let foreign_field = declared_field(
        &program,
        &program.measures[1]
            .parameter
            .as_ref()
            .unwrap()
            .type_reference,
        projected_member(&program, 1),
    )
    .expect("second measure's field")
    .symbol;
    let body = program
        .tables
        .bodies
        .expressions
        .expression_handles(program.measures[0].body)[0];
    let resolved::expression::ExpressionNode::Member(member) =
        program.tables.bodies.expressions.expression_mut(body)
    else {
        panic!("first measure projection");
    };
    member.member_symbol = foreign_field;
    assert!(
        declared_field(
            &program,
            &program.measures[0]
                .parameter
                .as_ref()
                .unwrap()
                .type_reference,
            projected_member(&program, 0),
        )
        .is_none()
    );
    let typed = lower_symbol_resolved_trees(&program).expect("retain conflicting selection");
    assert_eq!(typed_member(&typed, 0).member_symbol, foreign_field);
}

#[test]
fn array_constructor_projection_needs_the_retained_declaration_type() {
    let mut program = crate::front_end::resolved_program(
        "data Cell [copy] { value: u64; }
         data Other [copy] { value: u64; }
         const VALUES: [Cell; 2] = [Cell { value: 7 }, Cell { value: 9 }];
         machine read() -> u64 { VALUES[0].value }",
    );
    let member = program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(_, node)| {
            if let resolved::expression::ExpressionNode::Member(member) = node {
                Some(member.clone())
            } else {
                None
            }
        })
        .expect("array element field");
    let field =
        super::declared_receiver_field(&program, &program.tables.bodies.expressions, &member, 0)
            .expect("retained array element type selects its field")
            .symbol;
    let owner = program.symbols.get(field).parent;
    assert_eq!(program.symbols.name(owner), "Cell");
    let typed = lower_symbol_resolved_trees(&program).expect("type array element field");
    assert!(typed.expression_table.expression_entries().any(|(_, node)| {
        matches!(node, typed_trees::expression::ExpressionNode::Member(member) if member.member_symbol == field)
    }));
    program.roots.const_declarations = Default::default();
    assert!(
        super::declared_receiver_field(&program, &program.tables.bodies.expressions, &member, 0)
            .is_none(),
        "a missing declaration cannot be reconstructed from the first element's spelling"
    );
}
