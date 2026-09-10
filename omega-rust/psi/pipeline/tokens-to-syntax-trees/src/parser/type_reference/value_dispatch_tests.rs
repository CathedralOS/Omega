use super::*;
use source_files_to_tokens::Lexer;
use syntax_trees::item::{DataMember, Item};

#[test]
fn match_indices_retain_complete_syntax_for_semantic_admission() {
    for expression in [
        "(match true { true -> 1, false -> (1u8 / 0) })",
        "(1 + (match true { true -> 2, false -> 3 }))",
        "(match true { true -> false })",
    ] {
        for owner in [
            format!("Buffer<{expression}>"),
            format!("u64 in Quantity<{expression}>"),
        ] {
            let source = format!("data Main {{ value: {owner}; }}");
            let tokens = Lexer::new(&source).tokenize().expect("index tokens");
            let syntax = crate::parse_syntax_trees(&tokens)
                .expect("parser retains even semantically invalid dispatch");
            let Item::Data(data) = syntax.root_items().next().expect("data") else {
                panic!("data");
            };
            let [DataMember::Field(field)] = syntax.items.data_members(data.members) else {
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
                _ => panic!("index owner"),
            };
            let [argument] = syntax.type_references.type_reference_handles(arguments) else {
                panic!("one index");
            };
            assert!(matches!(
                syntax.type_references.type_reference(*argument),
                TypeReferenceNode::ConstExpression(_)
            ));
            let dispatch = syntax
                .expressions
                .iter_expressions()
                .find_map(|(_, expression)| match expression {
                    ExpressionNode::Match(dispatch) => Some(dispatch),
                    _ => None,
                })
                .expect("original Match retained");
            let arms = syntax.expressions.match_arms(dispatch.arms);
            assert_eq!(
                arms.len(),
                if expression.contains("true -> false") {
                    1
                } else {
                    2
                }
            );
        }
    }
}
