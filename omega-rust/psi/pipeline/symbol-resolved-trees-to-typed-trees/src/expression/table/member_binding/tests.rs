use super::{declared_field, declared_symbol_type};
use crate::lower_symbol_resolved_trees;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees as resolved;
use symbols::SymbolHandle;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

const PROJECTED_MEASURES: &str = "
    data First { remaining: u64; }
    data Second { remaining: u64; }
    measure First::Remaining(value: First) -> u64 { value.remaining }
    measure Second::Remaining(value: Second) -> u64 { value.remaining }
";

fn resolve(source: &str) -> resolved::SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("measure tokens");
    let syntax = parse_syntax_trees(&tokens).expect("measure syntax");
    lower_syntax_trees(&syntax).expect("measure resolution")
}

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
fn measure_projection_binds_missing_selector_to_its_exact_parameter_type() {
    let program = resolve(PROJECTED_MEASURES);
    let typed = lower_symbol_resolved_trees(&program).expect("measure typing");
    let mut selected_fields = Vec::new();
    for (measure_position, measure) in program.measures.iter().enumerate() {
        let parameter = measure.parameter.as_ref().expect("measure parameter");
        let member = projected_member(&program, measure_position);
        assert_eq!(member.member_symbol, SymbolHandle::invalid());
        let resolved::expression::ExpressionNode::Name(receiver) = program
            .tables
            .bodies
            .expressions
            .expression(member.receiver)
        else {
            panic!("direct parameter receiver");
        };
        assert_eq!(receiver.symbol, parameter.symbol);
        assert_eq!(receiver.head_symbol, parameter.symbol);
        let expected = declared_field(&program, &parameter.type_reference, member)
            .expect("field in the parameter's declared owner")
            .symbol;
        assert!(expected.is_valid());
        assert_eq!(
            typed_member(&typed, measure_position).member_symbol,
            expected
        );
        selected_fields.push(expected);
    }
    assert_ne!(selected_fields[0], selected_fields[1]);
}

#[test]
fn measure_projection_preserves_conflicting_selector_for_validation() {
    let mut program = resolve(PROJECTED_MEASURES);
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
fn measure_projection_does_not_bind_an_unresolved_same_spelled_receiver() {
    let mut program = resolve(PROJECTED_MEASURES);
    let receiver = projected_member(&program, 0).receiver;
    let resolved::expression::ExpressionNode::Name(path) =
        program.tables.bodies.expressions.expression_mut(receiver)
    else {
        panic!("measure parameter receiver");
    };
    path.symbol = SymbolHandle::invalid();
    path.head_symbol = SymbolHandle::invalid();
    let typed = lower_symbol_resolved_trees(&program).expect("retain unresolved receiver");
    assert_eq!(
        typed_member(&typed, 0).member_symbol,
        SymbolHandle::invalid()
    );
    assert!(declared_symbol_type(&program, SymbolHandle::invalid()).is_none());
    assert!(declared_symbol_type(&program, program.measures[0].symbol).is_none());
}
