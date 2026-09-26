use super::{
    constant, evaluate, evaluate_fully, integer_encoding, literal_encoding, parse, parse_files,
};
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::Item;

#[test]
fn anonymous_fractional_intermediate_lands_once_and_retains_authored_expression() {
    let (syntax, sources) = parse("const SIZE: u64 = 7 / 2 * 2;");
    let original = constant(&syntax, "SIZE").value;
    let original_expression = syntax.expressions.expression(original).clone();
    let original_span = syntax.expressions.source_span(original);
    let evaluated =
        super::super::evaluate(syntax, Some(sources), &[], None).expect("integral final landing");
    integer_encoding(&evaluated, "SIZE", 7);
    let definition = constant(&evaluated, "SIZE");
    let receipt = definition
        .normalization
        .as_ref()
        .expect("initializer receipt");
    assert_eq!(receipt.authored_expression, original);
    assert_ne!(definition.value, original);
    assert_eq!(
        evaluated.expressions.expression(original),
        &original_expression
    );
    assert_eq!(
        evaluated.expressions.source_span(definition.value),
        original_span
    );
    assert!(receipt.selections.is_empty());
    assert_eq!(receipt.builtin_operators.len(), 2);
}

#[test]
fn forward_boolean_dependency_retains_exact_declaration_and_initializer_custody() {
    let (syntax, sources) = parse("const ENABLED: bool = SIZE == 7; const SIZE: u64 = 7 / 2 * 2;");
    let size = constant(&syntax, "SIZE");
    let declaration = size.name.source_span();
    let initializer = syntax.expressions.source_span(size.value);
    let ExpressionNode::Binary(comparison) = syntax
        .expressions
        .expression(constant(&syntax, "ENABLED").value)
    else {
        panic!("authored comparison");
    };
    let ExpressionNode::Name(path) = syntax.expressions.expression(comparison.left) else {
        panic!("named dependency");
    };
    let reference = syntax.expressions.identifier_path_members(*path)[0].source_span();
    let evaluated =
        super::super::evaluate(syntax, Some(sources), &[], None).expect("forward dependency");
    integer_encoding(&evaluated, "SIZE", 7);
    let enabled = constant(&evaluated, "ENABLED");
    assert_eq!(
        evaluated.expressions.expression(enabled.value),
        &ExpressionNode::Boolean(true)
    );
    let receipt = enabled.normalization.as_ref().expect("comparison receipt");
    assert_eq!(
        receipt.canonical_result_encoding,
        CanonicalConstValue::boolean(true).encoding
    );
    assert_eq!(
        receipt.selections,
        vec![syntax_trees::types::ConstArgumentOrigin {
            reference,
            declaration,
            initializer,
            canonical_value_encoding: CanonicalConstIdentity::integer("u64", 7).encoding,
        }]
    );
}

#[test]
fn dependency_layers_preserve_declared_integer_division_and_transitive_origins() {
    let evaluated = evaluate(
        "const RESTORED: u64 = HALF * 2;
         const HALF: u64 = SIZE / 2;
         const SIZE: u64 = 7 / 2 * 2;",
    )
    .expect("typed dependency layers");
    integer_encoding(&evaluated, "SIZE", 7);
    integer_encoding(&evaluated, "HALF", 3);
    integer_encoding(&evaluated, "RESTORED", 6);
    let receipt = constant(&evaluated, "RESTORED")
        .normalization
        .as_ref()
        .expect("layered receipt");
    assert_eq!(receipt.selections.len(), 2);
    for (name, value) in [("HALF", 3), ("SIZE", 7)] {
        assert!(
            receipt.selections.iter().any(|origin| origin.declaration
                == constant(&evaluated, name).name.source_span()
                && origin.canonical_value_encoding
                    == CanonicalConstIdentity::integer("u64", value).encoding),
            "missing {name} provenance: {receipt:?}"
        );
    }
}

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
fn selective_expressions_retain_unselected_dependency_origins() {
    for (expression, expected) in [
        ("false && FLAG", false),
        ("true || FLAG", true),
        ("match true { true -> true, false -> FLAG }", true),
    ] {
        let text = format!("const RESULT: bool = {expression}; const FLAG: bool = 1 == 1;");
        let evaluated = evaluate(&text).expect("valid skipped dependency retains custody");
        let result = constant(&evaluated, "RESULT");
        assert_eq!(
            evaluated.expressions.expression(result.value),
            &ExpressionNode::Boolean(expected)
        );
        let receipt = result.normalization.as_ref().expect("selective receipt");
        assert_eq!(
            receipt.canonical_result_encoding,
            CanonicalConstValue::boolean(expected).encoding
        );
        assert_eq!(receipt.selections.len(), 1);
        assert_eq!(
            receipt.selections[0].declaration,
            constant(&evaluated, "FLAG").name.source_span()
        );
        assert_eq!(
            receipt.selections[0].canonical_value_encoding,
            CanonicalConstValue::boolean(true).encoding
        );
    }
}

#[test]
fn selective_expressions_cannot_hide_invalid_or_unresolved_dependencies() {
    for expression in [
        "false && (BAD == 0)",
        "true || (BAD == 0)",
        "match true { true -> true, false -> BAD == 0 }",
    ] {
        for declaration in ["const BAD: u64 = 7 / 2;", ""] {
            let text = format!("const RESULT: bool = {expression}; {declaration}");
            let errors =
                evaluate(&text).expect_err("unselected dependencies still require admission");
            assert!(!errors.is_empty(), "{text}");
        }
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
fn bare_aliases_preserve_selected_declared_values() {
    let evaluated = evaluate("const COPY: u64 = VALUE; const VALUE: u64 = 3 + 4;")
        .expect("bare constant initializer alias");
    integer_encoding(&evaluated, "COPY", 7);
    let receipt = constant(&evaluated, "COPY").normalization.as_ref().unwrap();
    assert_eq!(receipt.selections.len(), 1);
    assert_eq!(
        receipt.selections[0].declaration,
        constant(&evaluated, "VALUE").name.source_span()
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
    crate::machine_execution::syntax_probes::resolve(&evaluated, None, &[])
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
fn aggregate_producing_calls_replay_both_roots_against_the_canonical_result() {
    let declarations = "data Pair [copy] { first: u64; second: u64; }
        machine make() -> Pair { Pair { first: 1, second: 2 } }
        machine elements() -> [u64; 2] { [3, 4] }";
    let text = format!(
        "{declarations}
        const P: Pair = make();
        const A: [u64; 2] = elements();",
    );
    // Call-custody receipts re-derive machine declaration spans; rejoining
    // them therefore requires the sourced lowering entrypoint.
    let (syntax, sources) = parse(&text);
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("aggregate-producing calls evaluate");
    for (name, carrier, literal) in [
        ("P", "Pair", "Pair { first: 1, second: 2 }"),
        ("A", "[u64; 2]", "[3, 4]"),
    ] {
        assert_eq!(
            constant(&evaluated, name)
                .normalization
                .as_ref()
                .unwrap()
                .canonical_result_encoding,
            literal_encoding(
                &format!("{declarations} const {name}: {carrier} = {literal};"),
                name,
            ),
            "{name}"
        );
    }
    let receipt = constant(&evaluated, "P").normalization.as_ref().unwrap();
    assert!(
        !receipt.call_selections.is_empty(),
        "aggregate call leaf retains invocation custody"
    );
    assert!(matches!(
        evaluated
            .expressions
            .expression(receipt.authored_expression),
        ExpressionNode::Call(_)
    ));
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "P").value),
        ExpressionNode::StructLiteral(_)
    ));
    crate::machine_execution::syntax_probes::resolve(&evaluated, Some(sources), &[])
        .expect("aggregate call receipts rejoin");
    evaluate_fully(&[("main.omg", &text)], &[]);
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
fn module_owned_aggregate_producers_reselect_their_exact_carrier() {
    let main_text =
        "use selected::settings::Pair; use selected::settings::make; const B: Pair = make();";
    let declaration_text = "module settings;
        pub data Pair [copy] { first: u64; second: u64; }
        pub machine make() -> Pair { Pair { first: 7, second: 8 } }";
    let (syntax, sources, ids) = parse_files(&[
        ("root/main.omg", main_text),
        ("dependency/settings.omg", declaration_text),
    ]);
    let [main, declaration] = ids.as_slice() else {
        panic!("module probe sources");
    };
    let bindings = [
        symbols::SourceScopedTopLevelBinding::module_import(
            *main,
            *declaration,
            "selected::settings::Pair",
            1,
        ),
        symbols::SourceScopedTopLevelBinding::module_import(
            *main,
            *declaration,
            "selected::settings::make",
            1,
        ),
    ];
    let evaluated = super::super::evaluate(syntax, Some(sources), &bindings, None)
        .expect("module-owned aggregate call evaluates");
    let definition = constant(&evaluated, "B");
    let ExpressionNode::StructLiteral(literal) = evaluated.expressions.expression(definition.value)
    else {
        panic!("module-owned aggregate materializes its constructor literal");
    };
    assert_eq!(literal.constructor_name.as_str(), "settings::Pair");
    evaluate_fully(
        &[
            ("root/main.omg", main_text),
            ("dependency/settings.omg", declaration_text),
        ],
        &bindings,
    );
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
            crate::machine_execution::syntax_probes::resolve(&invalid, Some(sources.clone()), &[])
                .is_err(),
            "{mutation}"
        );
    }
}
