use super::*;
use source_files_to_tokens::Lexer;
use syntax_trees::item::{DataMember, Item};

#[test]
fn remainder_const_arguments_retain_the_operation_before_semantic_admission() {
    for operand in ["7 % 2", "8 % 2", "7u64 % 2", "7 + 8 % 2"] {
        for domain in [false, true] {
            let field_type = if domain {
                format!("u64 in Quantity<{operand}>")
            } else {
                format!("Buffer<{operand}>")
            };
            let source = format!("data Main {{ value: {field_type}; }}");
            let tokens = Lexer::new(&source).tokenize().expect("tokenize");
            let syntax = crate::parse_syntax_trees(&tokens).expect("parse remainder syntax");
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
