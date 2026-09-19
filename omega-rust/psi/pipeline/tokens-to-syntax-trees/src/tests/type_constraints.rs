use crate::parser::parse_syntax_trees;
use source_files_to_tokens::Lexer;
use syntax_trees::expression::ExpressionNode;
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
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize unary argument");
        let parsed = parse_syntax_trees(&tokens).expect("retain unary argument syntax");
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
            "data Buffer<Backing, Element, const Count: u64> where {equation} {{ storage: Backing; }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize equation");
        let parsed = parse_syntax_trees(&tokens).expect("parse fixed-array type equation");
        let item = parsed.root_items().next().expect("data item");
        let mut copied = syntax_trees::SyntaxTrees::default();
        let copied_item = copied.copy_item_from(&parsed, item);
        for (trees, item) in [(&parsed, item), (&copied, &copied_item)] {
            let syntax_trees::item::Item::Data(data) = item else {
                panic!("expected data");
            };
            let [syntax_trees::item::ProofFact::Expression(fact)] =
                trees.items.proof_facts(data.where_facts)
            else {
                panic!("expected one equation");
            };
            let ExpressionNode::Binary(equation) = trees.expressions.expression(*fact) else {
                panic!("expected equality");
            };
            assert_eq!(
                equation.operator,
                syntax_trees::expression::BinaryOperator::Equal
            );
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
            assert_eq!(count.as_str(), "Count");
            let TypeReferenceNode::Named(element) =
                trees.type_references.type_reference(*element_type)
            else {
                panic!("expected element binder");
            };
            assert_eq!(element.as_str(), "Element");
        }
    }
}

#[test]
fn proof_fact_array_values_remain_value_expressions() {
    for equation in [
        "[1, 2] == [1, 2]",
        "([[1, 2], [3, 4]] == [[1, 2], [3, 4]])",
        "[match 1 { 1 -> 2, _ -> 3 }] == [2]",
    ] {
        let source = format!("data Buffer where {equation} {{ storage: u64; }}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize array values");
        let parsed = parse_syntax_trees(&tokens).expect("parse array value equality");
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
fn fixed_array_equation_elements_use_existing_generic_type_grammar() {
    let source = "data Buffer<Backing, Element, const Count: u64> where Backing == [Pair<Element, Element>; Count] { storage: Backing; }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize generic element");
    let parsed = parse_syntax_trees(&tokens).expect("parse generic array element");
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
    assert_eq!(base_name.as_str(), "Pair");
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
    for operand in [
        "[Element;]",
        "[Element; -1]",
        "[Element; Count + 1]",
        "[; 4]",
    ] {
        let source = format!(
            "data Buffer<Backing, Element, const Count: u64> where Backing == {operand} {{ storage: Backing; }}"
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize malformed type");
        assert!(parse_syntax_trees(&tokens).is_err(), "{operand}");
    }
}

#[test]
fn fixed_array_type_operand_is_not_runtime_value_syntax() {
    let tokens = Lexer::new("machine main() -> u64 { [u8; 4] }")
        .tokenize()
        .expect("tokenize type in value position");
    assert!(parse_syntax_trees(&tokens).is_err());
}

#[test]
fn range_end_kind_and_authored_endpoints_survive_tree_copy() {
    for endpoint in [
        "10",
        "18446744073709551616",
        "-9223372036854775808",
        "255u8",
    ] {
        for (separator, inclusive) in [("..", false), ("..=", true)] {
            let source = format!("data Limits {{ value: i64 [0{separator}{endpoint}]; }}");
            let tokens = Lexer::new(&source).tokenize().expect("tokenize range");
            let parsed = parse_syntax_trees(&tokens).expect("parse authored endpoint");
            let item = parsed.root_items().next().expect("data item");
            let mut copied = syntax_trees::SyntaxTrees::default();
            let copied_item = copied.copy_item_from(&parsed, item);
            for (trees, item) in [(&parsed, item), (&copied, &copied_item)] {
                let syntax_trees::item::Item::Data(data) = item else {
                    panic!("expected data");
                };
                let [syntax_trees::item::DataMember::Field(field)] =
                    trees.items.data_members(data.members)
                else {
                    panic!("expected one field");
                };
                let TypeReferenceNode::Constrained { constraints, .. } =
                    trees.type_references.type_reference(field.type_reference)
                else {
                    panic!("expected constrained field");
                };
                let [
                    syntax_trees::types::TypeConstraintNode::Range {
                        minimum,
                        maximum,
                        end_inclusive,
                    },
                ] = trees.type_references.constraints(*constraints)
                else {
                    panic!("expected range");
                };
                assert_eq!(*end_inclusive, inclusive);
                assert_eq!(trees.expressions.display_name(*minimum), "0");
                let ExpressionNode::Integer(literal) = trees.expressions.expression(*maximum)
                else {
                    panic!("authored literal must not become generated arithmetic");
                };
                assert_eq!(
                    literal.value_bignum().unwrap().to_string(),
                    endpoint.trim_end_matches("u8")
                );
                assert_eq!(
                    literal.landing().map(|landing| landing.landed_type),
                    (endpoint == "255u8").then_some(numerics::literals::LandedIntegerType::U8)
                );
            }
        }
    }
}

#[test]
fn named_proof_constraints_reject_in_scalar_and_slice_type_positions() {
    for type_reference in [
        "i32 [magic]",
        "f32 [finite]",
        "i32 [positive]",
        "i32 [non_negative]",
        "u32 [exact]",
        "&[u8, [non_empty]]",
        "i32 [0..=10, magic]",
        "i32 [Example::Fact]",
    ] {
        let source = format!("data Main {{ value: {type_reference}; }}");
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let error = parse_syntax_trees(&tokens).expect_err("named proof constraint must reject");
        assert!(
            error
                .message
                .contains("named proof constraints in type brackets are retired"),
            "{type_reference}: {}",
            error.message,
        );
        assert!(error.message.contains("in Domain"));
        assert!(error.message.contains("contracts"));
    }
}

#[test]
fn range_constraints_keep_literal_and_named_lower_bounds() {
    for bounds in [
        "0..=10",
        "0..10",
        "minimum..=maximum",
        "minimum..maximum",
        "self.minimum..=self.maximum",
        "minimum + 1..=maximum",
        "range..=maximum",
    ] {
        let source = format!("data Main {{ value: i32 [{bounds}]; }}");
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        parse_syntax_trees(&tokens)
            .unwrap_or_else(|error| panic!("range [{bounds}] must parse: {}", error.message));
    }

    let tokens = Lexer::new("data Main { value: i32 [range<0, 10>]; }")
        .tokenize()
        .expect("tokenize");
    let error = parse_syntax_trees(&tokens).expect_err("old range spelling remains retired");
    assert!(error.message.contains("range<a, b>"));
}

#[test]
fn named_constraint_retirement_preserves_declared_properties_and_value_domains() {
    let source = r#"
        data Point [copy] { value: i32; }
        data Envelope<T [copy]> [copy] { value: T; }
        data Receipt [linear] { }
        data Main { value: f32 in Finite; }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    parse_syntax_trees(&tokens).expect("properties and value domains remain separate surfaces");
}
