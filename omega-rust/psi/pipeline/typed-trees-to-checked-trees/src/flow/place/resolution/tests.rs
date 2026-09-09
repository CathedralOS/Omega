//! Case qualification selects declaration identity, not the first matching spelling.
use super::*;
use typed_trees::expression::TableMemberExpression;

fn fixture() -> (typed_trees::TypedTrees, TableMemberExpression) {
    let source = r#"
        data Outcome { case First(count: u64); case Second(count: u64); }
        data Other { case Second(count: u64); }
        machine observe(value: Outcome) {
            transition value {
                Outcome::Second { count } -> done(count)
                Outcome::First { count } -> done(count)
            }
            state done(count: u64) {}
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let member = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, node)| match node {
            ExpressionNode::Member(member)
                if member
                    .case_variant
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "Second") =>
            {
                Some(member.clone())
            }
            _ => None,
        })
        .expect("case pattern retains its qualified payload projection");
    (program, member)
}

fn field(program: &typed_trees::TypedTrees, type_name: &str, case_name: &str) -> SymbolHandle {
    let declaration = program
        .data_definitions()
        .iter()
        .find(|row| row.name.as_str() == type_name)
        .unwrap();
    let variant = program
        .data_members(declaration)
        .iter()
        .find_map(|row| match row {
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == case_name =>
            {
                Some(variant)
            }
            _ => None,
        })
        .unwrap();
    program.data_payload_fields(variant)[0].symbol
}

#[test]
fn case_qualified_payload_preserves_the_selected_case_identity() {
    let (program, mut member) = fixture();
    let expected = field(&program, "Outcome", "Second");
    assert_ne!(expected, field(&program, "Outcome", "First"));
    member.member_symbol = SymbolHandle::invalid();
    assert_eq!(
        effective_member_symbol(&program, member.receiver, &member),
        expected
    );
    member.member_symbol = expected;
    assert_eq!(
        effective_member_symbol(&program, member.receiver, &member),
        expected
    );
}

#[test]
fn case_qualified_payload_rejects_conflicting_retained_identity() {
    let (program, mut member) = fixture();
    for conflicting in [
        field(&program, "Outcome", "First"),
        field(&program, "Other", "Second"),
    ] {
        member.member_symbol = conflicting;
        assert!(!effective_member_symbol(&program, member.receiver, &member).is_valid());
    }
}

#[test]
fn case_qualified_payload_does_not_fall_back_when_qualification_is_missing() {
    let (program, original) = fixture();
    let mut member = original.clone();
    member.member_symbol = field(&program, "Outcome", "Second");
    member.case_variant = Some(Identifier::generated("Absent"));
    assert!(!effective_member_symbol(&program, member.receiver, &member).is_valid());
    member = original.clone();
    member.member = Identifier::generated("absent");
    assert!(!effective_member_symbol(&program, member.receiver, &member).is_valid());
    assert!(!effective_member_symbol(&program, ExpressionHandle::invalid(), &original).is_valid());
}
