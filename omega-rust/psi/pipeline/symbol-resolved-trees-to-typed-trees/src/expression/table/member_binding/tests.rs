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

#[test]
fn nested_constructor_projections_bind_fields_and_preserve_conflicting_selections() {
    let mut program = resolve(
        "data Inner [copy] { size: u64; }
        data Outer [copy] { inner: Inner; }
        data Other [copy] { size: u64; }
        const VALUE: Outer = Outer { inner: Inner { size: 7 } };
        machine read() -> u64 { VALUE.inner.size }",
    );
    let typed = lower_symbol_resolved_trees(&program).expect("literal receiver typing");
    let mut projections = 0;
    for (_, expression) in typed.expression_table.expression_entries() {
        let typed_trees::expression::ExpressionNode::Member(member) = expression else {
            continue;
        };
        assert!(
            member.member_symbol.is_valid(),
            "nested constructor field has an exact symbol"
        );
        assert_eq!(
            typed.symbols.name(member.member_symbol),
            member.member.as_str()
        );
        projections += 1;
    }
    assert_eq!(projections, 2);
    let other = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Other")
        .unwrap();
    let resolved::data::DataMember::Field(field) = &program.data_members(other.members)[0] else {
        panic!("foreign field");
    };
    let foreign = field.symbol;
    let expression = program.tables.bodies.expressions.iter_expressions().find_map(|(handle, expression)| {
        matches!(expression, resolved::expression::ExpressionNode::Member(member) if member.member.as_str() == "size").then_some(handle)
    }).unwrap();
    let resolved::expression::ExpressionNode::Member(member) =
        program.tables.bodies.expressions.expression_mut(expression)
    else {
        panic!("projection");
    };
    member.member_symbol = foreign;
    let typed =
        lower_symbol_resolved_trees(&program).expect("retain conflicting projection for checking");
    assert!(typed.expression_table.expression_entries().any(
        |(_, expression)| matches!(expression,
        typed_trees::expression::ExpressionNode::Member(member)
            if member.member.as_str() == "size" && member.member_symbol == foreign)
    ));
}

#[test]
fn module_constructor_projection_keeps_its_declaring_field_owner() {
    let root = "use settings; data Config [copy] { size: u64; }
        machine read() -> u64 { settings::VALUE.size }";
    let module = "module settings; pub data Config [copy] { size: u64; }
        pub const VALUE: Config = Config { size: 7 };";
    let tokens = Lexer::new(root).tokenize().unwrap();
    let mut syntax = parse_syntax_trees(&tokens).unwrap();
    let tokens = Lexer::new(module).tokenize().unwrap();
    tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
        &mut syntax,
        source::SourceId(1),
        &tokens,
    )
    .unwrap();
    let resolved = lower_syntax_trees(&syntax).expect("module projection resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("module projection typing");
    let member = typed
        .expression_table
        .expression_entries()
        .find_map(|(_, expression)| match expression {
            typed_trees::expression::ExpressionNode::Member(member) => Some(member),
            _ => None,
        })
        .expect("module constant field projection");
    let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
        typed.expression_table.expression(member.receiver)
    else {
        panic!("module constructor");
    };
    assert!(member.member_symbol.is_valid());
    assert_eq!(
        typed.symbols.get(member.member_symbol).parent,
        literal.type_symbol
    );
    let caller_owner = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Config")
        .expect("caller's same-spelled data");
    assert_ne!(literal.type_symbol, caller_owner.symbol);
}
