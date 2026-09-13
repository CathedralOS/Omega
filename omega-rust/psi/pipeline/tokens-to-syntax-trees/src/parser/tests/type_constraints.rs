use super::*;

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
