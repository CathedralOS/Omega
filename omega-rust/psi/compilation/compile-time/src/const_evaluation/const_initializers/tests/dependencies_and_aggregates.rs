use super::{constant, evaluate, evaluate_fully, integer_encoding, literal_encoding, parse};
use language_semantics::const_value::CanonicalConstValue;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::Item;

#[test]
fn unused_invalid_declarations_cannot_escape_evaluation() {
    for declaration in [
        "const UNUSED: u64 = 7 / 2;",
        "const UNUSED: u8 = 255 + 1;",
        "const UNUSED: u8 = 255u8 + 1u8 - 1u8;",
        "const UNUSED: u64 = 1 / 0;",
    ] {
        let text = format!("{declaration} machine run() -> u64 {{ 0 }}");
        let errors = evaluate(&text).expect_err("unused invalid initializer must reject");
        assert!(!errors.is_empty(), "{declaration}");
    }
}

#[test]
fn initializer_cycles_and_unresolved_operands_reject() {
    for text in [
        "const FIRST: u64 = SECOND + 1; const SECOND: u64 = FIRST + 1;",
        "const SELF: u64 = SELF + 1;",
        "const VALUE: u64 = MISSING + 1;",
    ] {
        let errors = evaluate(text).expect_err("initializer cannot invent a dependency value");
        assert!(!errors.is_empty(), "{text}");
    }
}

#[test]
fn anonymous_decimal_leaves_retain_exact_integer_and_boolean_meaning() {
    let integer = evaluate("const COUNT: u64 = 3.0;").expect("integral anonymous decimal");
    integer_encoding(&integer, "COUNT", 3);
    let evaluated = evaluate("const COUNT: u64 = 1.5 * 2; const FLAG: bool = 0.5 < 1;")
        .expect("anonymous decimals are exact rationals, not landed floating operands");
    integer_encoding(&evaluated, "COUNT", 3);
    let flag = constant(&evaluated, "FLAG");
    assert_eq!(
        evaluated.expressions.expression(flag.value),
        &ExpressionNode::Boolean(true)
    );
    assert_eq!(
        flag.normalization
            .as_ref()
            .expect("decimal comparison receipt")
            .canonical_result_encoding,
        CanonicalConstValue::boolean(true).encoding
    );
}

#[test]
fn landed_float_leaves_compose_at_their_own_format() {
    let evaluated = evaluate(
        "const A: f32 = 2.0;
         const ALIAS: f32 = A;
         const SUM: f32 = A + 1.0;
         const PRODUCT: f32 = 1.5f32 * 2;
         const FLAG: bool = 0.5f32 < 1;",
    )
    .expect("landed float operands evaluate at their landed format");
    for (name, encoding) in [
        ("ALIAS", "float:f32:40000000"),
        ("SUM", "float:f32:40400000"),
        ("PRODUCT", "float:f32:40400000"),
    ] {
        let receipt = constant(&evaluated, name)
            .normalization
            .as_ref()
            .expect("evaluated declaration");
        assert_eq!(receipt.canonical_result_encoding, encoding, "{name}");
    }
    let flag = constant(&evaluated, "FLAG");
    assert_eq!(
        evaluated.expressions.expression(flag.value),
        &ExpressionNode::Boolean(true)
    );
}

#[test]
fn landed_float_leaves_keep_their_carrier_and_finite_identity() {
    for text in [
        // A float result is not an integer and does not land at one.
        "const COUNT: u64 = 1.5f32 * 2;",
        // Landed formats do not implicitly change, not even f32 to f64.
        "const COUNT: f64 = 1.5f32 + 0.25f32;",
        // A landed integer cannot mix into a float operation unconverted.
        "const MIXED: f32 = 1u64 + 0.5f32;",
        // A computed NaN has no authored payload bits to carry as identity.
        "const NAN: f32 = 0.0f32 / 0.0f32;",
    ] {
        let errors =
            evaluate(text).expect_err("landed float values keep carrier and determined bits");
        assert!(!errors.is_empty(), "{text}");
    }
}

#[test]
fn aggregate_constant_names_materialize_the_selected_value() {
    let declarations = "data Pair [copy] { first: u64; second: u64; }";
    let evaluated = evaluate(&format!(
        "{declarations}
        const A: [u64; 2] = [4, 9];
        const P: Pair = Pair {{ first: 7, second: 8 }};
        const B: [u64; 2] = A;
        const Q: Pair = P;",
    ))
    .expect("aggregate constant names evaluate");
    for (name, carrier, literal) in [
        ("B", "[u64; 2]", "[4, 9]"),
        ("Q", "Pair", "Pair { first: 7, second: 8 }"),
    ] {
        let definition = constant(&evaluated, name);
        let receipt = definition
            .normalization
            .as_ref()
            .expect("evaluated receipt");
        assert_eq!(
            receipt.canonical_result_encoding,
            literal_encoding(
                &format!("{declarations} const {name}: {carrier} = {literal};"),
                name,
            ),
            "{name}"
        );
        assert!(
            matches!(
                evaluated
                    .expressions
                    .expression(receipt.authored_expression),
                ExpressionNode::Name(_)
            ),
            "{name} retains its authored constant use"
        );
        assert_ne!(receipt.authored_expression, definition.value);
    }
    for (name, dependency) in [("B", "A"), ("Q", "P")] {
        let receipt = constant(&evaluated, name).normalization.as_ref().unwrap();
        assert!(
            receipt.selections.iter().any(|origin| {
                origin.declaration == constant(&evaluated, dependency).name.source_span()
            }),
            "{name} retains the exact selected declaration"
        );
    }
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "B").value),
        ExpressionNode::ArrayLiteral(_)
    ));
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "Q").value),
        ExpressionNode::StructLiteral(_)
    ));
    crate::syntax_probes::resolve(&evaluated, None, &[])
        .expect("aggregate constant-name receipts rejoin");
    evaluate_fully(
        &[(
            "main.omg",
            &format!(
                "{declarations}
        const A: [u64; 2] = [4, 9];
        const P: Pair = Pair {{ first: 7, second: 8 }};
        const B: [u64; 2] = A;
        const Q: Pair = P;",
            ),
        )],
        &[],
    );
}

#[test]
fn aggregate_producers_compose_inside_literal_fields_and_match_arms() {
    let declarations = "data Holder [copy] { values: [u64; 2]; tag: u64; }
        data Pair [copy] { first: u64; second: u64; }
        machine elements() -> [u64; 2] { [5, 6] }";
    let text = format!(
        "{declarations}
        const H: Holder = Holder {{ values: elements(), tag: 3 }};
        const M: Pair = match true {{ true -> Pair {{ first: 1, second: 2 }}, false -> Pair {{ first: 9, second: 9 }} }};",
    );
    let evaluated = evaluate(&text).expect("nested aggregate producers evaluate");
    assert_eq!(
        constant(&evaluated, "H")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const H: Holder = Holder {{ values: [5, 6], tag: 3 }};"),
            "H",
        ),
    );
    assert_eq!(
        constant(&evaluated, "M")
            .normalization
            .as_ref()
            .unwrap()
            .canonical_result_encoding,
        literal_encoding(
            &format!("{declarations} const M: Pair = Pair {{ first: 1, second: 2 }};"),
            "M",
        ),
    );
    evaluate_fully(&[("main.omg", &text)], &[]);
}

#[test]
fn aggregate_receipts_reject_changed_results_and_dropped_call_custody() {
    let text = "data Pair [copy] { first: u64; second: u64; }
        machine make() -> Pair { Pair { first: 1, second: 2 } }
        const P: Pair = make();";
    // The receipt comparison re-derives machine declaration custody; only the
    // sourced lowering can supply those spans, so the mutations below — not
    // the missing source context — must be the cause of rejection.
    let (syntax, sources) = parse(text);
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("aggregate receipt baseline");
    for mutation in ["result", "calls"] {
        let mut invalid = evaluated.clone();
        let item = *invalid
            .root_item_handles()
            .iter()
            .find(|item| {
                matches!(invalid.root_item(**item), Item::Const(definition) if definition.name.as_str() == "P")
            })
            .unwrap();
        let Item::Const(mut definition) = invalid.root_item(item).clone() else {
            panic!("constant");
        };
        let receipt = definition.normalization.as_mut().unwrap();
        match mutation {
            "result" => receipt.canonical_result_encoding.push('0'),
            "calls" => receipt.call_selections.clear(),
            _ => unreachable!(),
        }
        invalid.items.replace_item(item, Item::Const(definition));
        assert!(
            crate::syntax_probes::resolve(&invalid, Some(sources.clone()), &[]).is_err(),
            "{mutation}"
        );
    }
}
