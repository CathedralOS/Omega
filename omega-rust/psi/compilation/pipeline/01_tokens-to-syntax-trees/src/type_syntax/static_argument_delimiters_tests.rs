//! Nested type/domain closers must not consume an actual right-shift operand.

use source_files_to_tokens::Lexer;
use syntax_trees::expression::{BinaryOperator, ExpressionNode};
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

#[test]
fn nested_static_values_keep_closers_shifts_and_named_argument_transactions() {
    for operand in [
        "1u64",
        "N + 1u64",
        "8u64 >> 1u64",
        "(N + 8u64) >> 1u64",
        "N",
    ] {
        for domain in [false, true] {
            let field_type = if domain {
                format!("u64 in Quantity<Inner<{operand}>>")
            } else {
                format!("Outer<Inner<{operand}>>")
            };
            let text = format!("data Packet {{ value: {field_type}; }}");
            let tokens = Lexer::new(&text).tokenize().unwrap();
            let syntax = crate::parse_syntax_trees(&tokens).expect("nested static argument");
            let data = syntax
                .root_items()
                .find_map(|item| match item {
                    Item::Data(data) => Some(data),
                    _ => None,
                })
                .unwrap();
            let DataMember::Field(field) = &syntax.items.data_members(data.members)[0] else {
                panic!("field");
            };
            let outer_arguments = match syntax.type_references.type_reference(field.type_reference)
            {
                TypeReferenceNode::Generic { arguments, .. } => *arguments,
                TypeReferenceNode::Constrained { constraints, .. } => {
                    let [TypeConstraintNode::Domain(domain)] =
                        syntax.type_references.constraints(*constraints)
                    else {
                        panic!("domain");
                    };
                    domain.arguments
                }
                _ => panic!("outer application"),
            };
            let [inner] = syntax
                .type_references
                .type_reference_handles(outer_arguments)
            else {
                panic!("one inner application");
            };
            let TypeReferenceNode::Generic { arguments, .. } =
                syntax.type_references.type_reference(*inner)
            else {
                panic!("inner application");
            };
            let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
                panic!("one static value");
            };
            if operand == "N" {
                let TypeReferenceNode::Named(name) =
                    syntax.type_references.type_reference(*argument)
                else {
                    panic!("named argument");
                };
                assert_eq!(name.as_str(&syntax), "N");
                assert_eq!(
                    syntax.expressions.iter_expressions().count(),
                    0,
                    "rejected expression alternatives must not publish detached rows"
                );
                continue;
            }
            let TypeReferenceNode::ConstExpression(expression) =
                syntax.type_references.type_reference(*argument)
            else {
                panic!("retained static expression");
            };
            match syntax.expressions.expression(*expression) {
                ExpressionNode::Integer(_) => assert_eq!(operand, "1u64"),
                ExpressionNode::Binary(binary) => {
                    assert_eq!(
                        binary.operator,
                        if operand.contains(">>") {
                            BinaryOperator::ShiftRight
                        } else {
                            BinaryOperator::Add
                        }
                    );
                    assert!(matches!(
                        syntax.expressions.expression(binary.right),
                        ExpressionNode::Integer(_)
                    ));
                }
                _ => panic!("authored literal or computation"),
            }
        }
    }
}
