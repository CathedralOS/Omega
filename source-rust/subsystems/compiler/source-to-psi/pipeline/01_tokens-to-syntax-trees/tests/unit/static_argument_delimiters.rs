use syntax_trees::expression::{BinaryOperator, ExpressionNode};
use syntax_trees::item::{DataMember, Item};
use syntax_trees::types::{TypeConstraintNode, TypeReferenceNode};

/// The source that puts `operand` in a static argument of `owner`: a nested
/// type application, a nested domain application or a conformance head.
fn static_argument_source(owner: &str, operand: &str) -> String {
    match owner {
        "domain" => format!("data Packet {{ value: u64 in Quantity<Inner<{operand}>>; }}"),
        "implementation" => format!("Selected satisfies Outer<{operand}> {{}}"),
        _ => format!("data Packet {{ value: Outer<Inner<{operand}>>; }}"),
    }
}

/// Between `<` and `>` every `>` closes a list, so two that touch close two
/// lists and never shift. The same shift in parentheses is one argument; the
/// twin differs only in those parentheses and is refused.
#[test]
fn a_right_shift_in_a_static_argument_needs_parentheses() {
    // The nested owners have two lists open, so `>>` closes both and the
    // count, three bytes after the first `>`, is the stray token. The
    // conformance head has one list open, so its second `>` is.
    for (owner, stray, after_first_greater) in [
        ("type", "1u64", 3),
        ("domain", "1u64", 3),
        ("implementation", ">", 1),
    ] {
        let text = static_argument_source(owner, "(8u64 >> 1u64)");
        crate::tests::parse_source(&text);

        let text = static_argument_source(owner, "8u64 >> 1u64");
        let error = crate::tests::parse_error(&text);
        let span = error.source_span.span;
        assert_eq!(
            &text[span.start..span.end],
            stray,
            "{text}: {}",
            error.message
        );
        assert_eq!(
            span.start,
            text.find(">>").unwrap() + after_first_greater,
            "{text}"
        );
    }
}

/// `Outer<Inner<u8>>` closes two lists with two touching `>`: the outer
/// application holds exactly the inner one, which holds `u8`.
#[test]
fn touching_closers_end_a_nested_type_application() {
    let text = "data Packet { value: Outer<Inner<u8>>; }";
    let syntax = crate::tests::parse_source(text);
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
    let mut reference = field.type_reference;
    for name in ["Outer", "Inner"] {
        let TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } = syntax.type_references.type_reference(reference)
        else {
            panic!("`{name}` is an application");
        };
        assert_eq!(base_name.as_str(&syntax), name);
        let [argument] = syntax.type_references.type_reference_handles(*arguments) else {
            panic!("`{name}` holds one argument");
        };
        reference = *argument;
    }
    let TypeReferenceNode::Named(element) = syntax.type_references.type_reference(reference) else {
        panic!("the innermost argument is a name");
    };
    assert_eq!(element.as_str(&syntax), "u8");
    assert_eq!(syntax.expressions.iter_expressions().count(), 0);
}

#[test]
fn nested_static_values_keep_closers_shifts_and_named_argument_transactions() {
    for operand in [
        "1u64",
        "N + 1u64",
        "(8u64 >> 1u64)",
        "((N + 8u64) >> 1u64)",
        "N",
    ] {
        for owner in ["type", "domain", "implementation"] {
            let text = static_argument_source(owner, operand);
            let syntax = crate::tests::parse_source(&text);
            let argument = if owner == "implementation" {
                let implementation = syntax
                    .root_items()
                    .find_map(|item| match item {
                        Item::Conformance(implementation) => Some(implementation),
                        _ => None,
                    })
                    .unwrap();
                let [argument] = syntax
                    .type_references
                    .type_reference_handles(implementation.trait_arguments)
                else {
                    panic!("one implementation argument");
                };
                *argument
            } else {
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
                let outer_arguments =
                    match syntax.type_references.type_reference(field.type_reference) {
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
                *argument
            };
            if operand == "N" {
                let TypeReferenceNode::Named(name) =
                    syntax.type_references.type_reference(argument)
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
                syntax.type_references.type_reference(argument)
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
