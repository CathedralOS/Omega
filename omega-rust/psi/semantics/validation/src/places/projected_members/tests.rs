use super::*;
use source::SourceMap;
use source_files_to_tokens::Lexer;
use typed_trees::name::Identifier;

fn fixture() -> TypedTrees {
    let source = "data Small { value: u8; } data Large { value: u64; }
        data Choice { case Narrow(value: Small); case Wide(value: Large); }
        machine Choice::inspect(self) -> u64 {
            transition self {
                Choice::Narrow { value } -> 0
                Choice::Wide { value } -> value.value
            }
        }";
    let tokens = Lexer::new(source).tokenize().expect("tokenize projection");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("payload_projection.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse projection");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        std::sync::Arc::new(sources),
    )
    .expect("resolve projection");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type projection")
}

fn payload(program: &TypedTrees) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Member(member)
                if member.case_variant.as_ref().map(|name| name.as_str()) == Some("Wide") =>
            {
                Some(handle)
            }
            _ => None,
        })
        .expect("selected Wide payload")
}

fn declared_type(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Choice::inspect")
        .expect("receiver machine");
    declared_place_type_raw(
        program,
        machine,
        Some(&program.machine_states(machine)[0]),
        expression,
    )
}

#[test]
fn selected_case_controls_payload_and_nested_field_types() {
    let program = fixture();
    let payload = payload(&program);
    let payload_type = declared_type(&program, payload).expect("payload type");
    let large = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Large")
        .expect("Large declaration");
    assert_eq!(program.type_reference_symbol(payload_type), large.symbol);
    let nested = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| match expression {
            ExpressionNode::Member(member) if member.receiver == payload => Some(handle),
            _ => None,
        })
        .expect("nested payload member");
    let nested_type = declared_type(&program, nested).expect("nested field type");
    assert!(matches!(
        program.type_reference_table.type_reference(nested_type),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "u64"
    ));
}

#[test]
fn conflicting_payload_symbol_cannot_use_same_spelled_field() {
    let mut program = fixture();
    let payload = payload(&program);
    assert!(declared_type(&program, payload).is_some());
    let foreign = program.machines()[0].symbol;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(payload) else {
        unreachable!()
    };
    member.member_symbol = foreign;
    assert!(declared_type(&program, payload).is_none());
}

#[test]
fn missing_case_cannot_borrow_another_cases_payload() {
    let mut program = fixture();
    let payload = payload(&program);
    assert!(declared_type(&program, payload).is_some());
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(payload) else {
        unreachable!()
    };
    member.case_variant = Some(Identifier::generated("Missing"));
    assert!(declared_type(&program, payload).is_none());
}
