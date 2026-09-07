use super::*;
use source_files_to_tokens::Lexer;
use syntax_trees::item::{DataMember, Item};

#[test]
fn an_empty_parenthesis_pair_remains_a_unit_type_argument() {
    let tokens = Lexer::new("data Main { value: Buffer<( /* unit */ )>; }")
        .tokenize()
        .expect("tokenize Unit argument");
    let syntax = crate::parse_syntax_trees(&tokens).expect("parse Unit argument");
    let Item::Data(definition) = syntax.root_items().next().expect("Main declaration") else {
        panic!("data declaration");
    };
    let [DataMember::Field(field)] = syntax.items.data_members(definition.members) else {
        panic!("one field");
    };
    let TypeReferenceNode::Generic { arguments, .. } =
        syntax.type_references.type_reference(field.type_reference)
    else {
        panic!("generic field type");
    };
    let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
        panic!("one type argument");
    };
    assert!(matches!(
        syntax.type_references.type_reference(*argument),
        TypeReferenceNode::Unit
    ));
}

#[test]
fn division_and_remainder_const_arguments_retain_operations_before_semantic_admission() {
    for operand in [
        "7 % 2",
        "8 % 2",
        "7u64 % 2",
        "7 + 8 % 2",
        "7 / 2",
        "7 / 2 * 2",
        "7u64 / 2 * 2",
        "7 + 8 / 2",
        "(7 / 2) * 2",
        "(7u64 / 2) * 2",
    ] {
        for domain in [false, true] {
            let field_type = if domain {
                format!("u64 in Quantity<{operand}>")
            } else {
                format!("Buffer<{operand}>")
            };
            let source = format!("data Main {{ value: {field_type}; }}");
            let tokens = Lexer::new(&source).tokenize().expect("tokenize");
            let syntax = crate::parse_syntax_trees(&tokens).expect("parse const arithmetic syntax");
            let data = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Data(data) => Some(data),
                    _ => None,
                })
                .expect("data");
            let DataMember::Field(field) = &syntax.items.data_members(data.members)[0] else {
                panic!("field");
            };
            let arguments = match syntax.type_references.type_reference(field.type_reference) {
                TypeReferenceNode::Generic { arguments, .. } => *arguments,
                TypeReferenceNode::Constrained { constraints, .. } => {
                    let [TypeConstraintNode::Domain(domain)] =
                        syntax.type_references.constraints(*constraints)
                    else {
                        panic!("domain");
                    };
                    domain.arguments
                }
                _ => panic!("argument owner"),
            };
            let [argument] = syntax.type_references.type_reference_handles(arguments) else {
                panic!("one argument");
            };
            assert!(
                matches!(
                    syntax.type_references.type_reference(*argument),
                    TypeReferenceNode::ConstExpression(_)
                ),
                "{source}"
            );
        }
    }
}
