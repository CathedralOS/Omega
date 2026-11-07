use crate::parser::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::BinaryOperator;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::DataMember;
use syntax_trees::item::ProofFact;
use syntax_trees::types::TypeReferenceNode;

#[test]
fn unary_const_arguments_retain_their_typed_evaluation_obligation() {
    use syntax_trees::item::{DataMember, Item};

    // Even wrong operand carriers belong to semantic admission, not an
    // integer-only fold in the parser.
    for argument in [
        "!false", "(!false)", "!!true", "~1u8", "(~1u8)", "!0", "~false",
    ] {
        let source = format!("data Main {{ value: Flag<{argument}>; }}");
        let parsed = super::parse_source_verbose(
            &source,
            "tokenize unary argument",
            "retain unary argument syntax",
        );
        let Item::Data(data) = parsed.root_items().next().unwrap() else {
            panic!("data declaration");
        };
        let [DataMember::Field(field)] = parsed.items.data_members(data.members) else {
            panic!("one field");
        };
        let TypeReferenceNode::Generic { arguments, .. } =
            parsed.type_references.type_reference(field.type_reference)
        else {
            panic!("generic field");
        };
        let [argument] = parsed.type_references.type_reference_handles(*arguments) else {
            panic!("one argument");
        };
        let TypeReferenceNode::ConstExpression(expression) =
            parsed.type_references.type_reference(*argument)
        else {
            panic!("retain expression until its carrier is known");
        };
        assert!(matches!(
            parsed.expressions.expression(*expression),
            ExpressionNode::Unary(_)
        ));
    }
}

#[test]
fn fixed_array_equation_operands_retain_type_structure_through_grouping_and_copy() {
    for equation in [
        "Backing == [[Element; Count]; 2]",
        "(Backing == [[Element; Count]; 2])",
        "([[Element; Count]; 2]) == Backing",
    ] {
        let source = format!(
            "data Buffer<Backing, Element, const Count: u64> requires {equation} {{ storage: Backing; }}"
        );
        let parsed = super::parse_source_verbose(
            &source,
            "tokenize equation",
            "parse fixed-array type equation",
        );
        let item = parsed.root_items().next().expect("data item");
        let mut copied = syntax_trees::SyntaxTrees::default();
        let copied_item = copied.copy_item_from(&parsed, item);
        for (trees, item) in [(&parsed, item), (&copied, &copied_item)] {
            let syntax_trees::item::Item::Data(data) = item else {
                panic!("expected data");
            };
            let [ProofFact::Expression(fact)] = trees.items.proof_facts(data.where_facts) else {
                panic!("expected one equation");
            };
            let ExpressionNode::Binary(equation) = trees.expressions.expression(*fact) else {
                panic!("expected equality");
            };
            assert_eq!(equation.operator, BinaryOperator::Equal);
            let operand = [equation.left, equation.right]
                .into_iter()
                .find_map(|operand| match trees.expressions.expression(operand) {
                    ExpressionNode::TypeExpression(type_reference) => Some(*type_reference),
                    _ => None,
                })
                .expect("one operand retains a type reference");
            let TypeReferenceNode::FixedArray {
                element_type,
                length: syntax_trees::types::FixedArrayLength::Literal(2),
            } = trees.type_references.type_reference(operand)
            else {
                panic!("expected outer fixed array");
            };
            let TypeReferenceNode::FixedArray {
                element_type,
                length: syntax_trees::types::FixedArrayLength::ConstParameter(count),
            } = trees.type_references.type_reference(*element_type)
            else {
                panic!("expected nested fixed array");
            };
            assert_eq!(count.as_str(trees), "Count");
            let TypeReferenceNode::Named(element) =
                trees.type_references.type_reference(*element_type)
            else {
                panic!("expected element binder");
            };
            assert_eq!(element.as_str(trees), "Element");
        }
    }
}

#[test]
fn proof_fact_array_values_remain_value_expressions() {
    for equation in [
        "[1, 2] == [1, 2]",
        "([[1, 2], [3, 4]] == [[1, 2], [3, 4]])",
        "[match 1 { 1 -> 2, _ -> 3 }] == [2]",
        "&[1, 2] == &[1, 2]",
    ] {
        let source = format!("data Buffer requires {equation} {{ storage: u64; }}");
        let parsed = super::parse_source_verbose(
            &source,
            "tokenize array values",
            "parse array value equality",
        );
        assert!(
            parsed
                .expressions
                .iter_expressions()
                .any(|(_, expression)| { matches!(expression, ExpressionNode::ArrayLiteral(_)) })
        );
        assert!(
            !parsed
                .expressions
                .iter_expressions()
                .any(|(_, expression)| { matches!(expression, ExpressionNode::TypeExpression(_)) })
        );
    }
}

#[test]
fn reference_operands_retain_access_and_element_type_role_through_copy() {
    for (access, expected) in [
        ("", language_core::ReferenceAccess::Shared),
        ("mut ", language_core::ReferenceAccess::Mutable),
        ("write ", language_core::ReferenceAccess::WriteOnly),
    ] {
        let source = format!(
            "data Buffer<Backing, Element, const Count: u64> requires Backing == (&{access}[&Element; Count]) {{ storage: Backing; }}"
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let parsed = parse_syntax_trees(&tokens).expect("reference around a type-role operand");
        let item = parsed.root_items().next().unwrap();
        let mut copied = syntax_trees::SyntaxTrees::default();
        let copied_item = copied.copy_item_from(&parsed, item);
        for (trees, item) in [(&parsed, item), (&copied, &copied_item)] {
            let syntax_trees::item::Item::Data(data) = item else {
                panic!("data");
            };
            let [ProofFact::Expression(fact)] = trees.items.proof_facts(data.where_facts) else {
                panic!("equation");
            };
            let ExpressionNode::Binary(equation) = trees.expressions.expression(*fact) else {
                panic!("equality");
            };
            let ExpressionNode::Borrow(borrow) = trees.expressions.expression(equation.right)
            else {
                panic!("borrow syntax retains access until type-role resolution");
            };
            assert_eq!(borrow.access, expected);
            let ExpressionNode::TypeExpression(reference) =
                trees.expressions.expression(borrow.target)
            else {
                panic!("array operand retains its type role");
            };
            let TypeReferenceNode::FixedArray { element_type, .. } =
                trees.type_references.type_reference(*reference)
            else {
                panic!("array");
            };
            assert!(matches!(
                trees.type_references.type_reference(*element_type),
                TypeReferenceNode::Reference {
                    access: language_core::ReferenceAccess::Shared,
                    lifetime: None,
                    ..
                }
            ));
        }
    }
}

#[test]
fn reference_value_operands_remain_borrows_and_recasts_reject_policy_suffixes() {
    let tokens = Lexer::new("data Buffer requires &left == &right { storage: u64; }")
        .tokenize()
        .unwrap();
    let parsed = parse_syntax_trees(&tokens).unwrap();
    assert!(
        !parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| matches!(expression, ExpressionNode::TypeExpression(_)))
    );
    for target in ["&u8", "& &u8", "&mut &write u8"] {
        let source =
            format!("machine view(value: &u64) -> &u8 {{ value as {target} in Wrapping }}");
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let error = parse_syntax_trees(&tokens).expect_err("recast policy must reject");
        assert!(
            error.message.contains("takes no arithmetic domain"),
            "{error:?}"
        );
        let without_policy = source.replace(" in Wrapping", "");
        let tokens = Lexer::new(&without_policy).tokenize().unwrap();
        parse_syntax_trees(&tokens).expect("the reference target itself is valid syntax");
    }
}

#[test]
fn fixed_array_equation_elements_use_existing_generic_type_grammar() {
    let source = "data Buffer<Backing, Element, const Count: u64> requires Backing == [Pair<Element, Element>; Count] { storage: Backing; }";
    let parsed = super::parse_source_verbose(
        source,
        "tokenize generic element",
        "parse generic array element",
    );
    let array = parsed
        .expressions
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            ExpressionNode::TypeExpression(type_reference) => Some(*type_reference),
            _ => None,
        })
        .expect("retained array type operand");
    let TypeReferenceNode::FixedArray { element_type, .. } =
        parsed.type_references.type_reference(array)
    else {
        panic!("expected fixed array");
    };
    let TypeReferenceNode::Generic {
        base_name,
        arguments,
        ..
    } = parsed.type_references.type_reference(*element_type)
    else {
        panic!("expected generic element");
    };
    assert_eq!(base_name.as_str(&parsed), "Pair");
    assert_eq!(
        parsed
            .type_references
            .type_reference_handles(*arguments)
            .len(),
        2
    );
}

#[test]
fn fixed_array_equation_operands_use_the_type_grammars_rejections() {
    for operand in ["[Element;]", "[; 4]"] {
        let source = format!(
            "data Buffer<Backing, Element, const Count: u64> requires Backing == {operand} {{ storage: Backing; }}"
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize malformed type");
        assert!(parse_syntax_trees(&tokens).is_err(), "{operand}");
    }
    // The type grammar retains a computed extent for checked semantic
    // evaluation instead of judging its value; an equation operand accepts
    // exactly what a field type accepts.
    for operand in ["[Element; -1]", "[Element; Count + 1]"] {
        let equation = format!(
            "data Buffer<Backing, Element, const Count: u64> requires Backing == {operand} {{ storage: Backing; }}"
        );
        let parsed = super::parse_source_verbose(
            &equation,
            "tokenize computed extent",
            "computed extents are retained for semantic evaluation",
        );
        let operand_type = parsed
            .expressions
            .iter_expressions()
            .find_map(|(_, expression)| match expression {
                ExpressionNode::TypeExpression(type_reference) => Some(*type_reference),
                _ => None,
            })
            .expect("the equation operand keeps its type role");
        let field = format!("data Buffer<Element, const Count: u64> {{ storage: {operand}; }}");
        let field_parsed = super::parse_source_verbose(
            &field,
            "tokenize computed extent",
            "computed extents are retained for semantic evaluation",
        );
        let syntax_trees::item::Item::Data(data) =
            field_parsed.root_items().next().expect("data item")
        else {
            panic!("data");
        };
        let [DataMember::Field(storage)] = field_parsed.items.data_members(data.members) else {
            panic!("one field");
        };
        for (trees, array) in [
            (&parsed, operand_type),
            (&field_parsed, storage.type_reference),
        ] {
            let TypeReferenceNode::FixedArray {
                length: syntax_trees::types::FixedArrayLength::Expression(extent),
                ..
            } = trees.type_references.type_reference(array)
            else {
                panic!("computed extent retained as an expression: {operand}");
            };
            assert!(matches!(
                trees.type_references.type_reference(*extent),
                TypeReferenceNode::ConstExpression(_)
            ));
        }
    }
}

#[test]
fn fixed_array_type_operand_is_not_runtime_value_syntax() {
    // `[u8; 4]` keeps its type role in a value position rather than becoming
    // an array value. Role resolution and validation reject executable use.
    let parsed = super::parse_source(
        "machine main() -> u64 { [u8; 4] }",
        "type operands share the complete expression grammar",
    );
    assert!(
        parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| matches!(expression, ExpressionNode::TypeExpression(_)))
    );
    assert!(
        !parsed
            .expressions
            .iter_expressions()
            .any(|(_, expression)| matches!(expression, ExpressionNode::ArrayLiteral(_)))
    );
}

#[test]
fn comma_constraint_slots_inside_type_brackets_reject() {
    for source in [
        "data Main { value: &[u8, [non_empty]]; }",
        "data Main { value: [u8, x <= 10]; }",
        "data Main { value: [i32, 0..=10]; }",
        "machine m(x: [u8, nonzero]) { }",
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        assert!(
            parse_syntax_trees(&tokens).is_err(),
            "inside-bracket constraint slot must reject: {source}"
        );
    }
}
