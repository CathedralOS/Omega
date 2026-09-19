use numerics::literals::{FloatFormat, FloatLiteral};
use std::{path::PathBuf, sync::Arc};
use typed_trees::{TypedTrees, expression::ExpressionNode};

fn fixture(carrier: &str, initializer: &str) -> TypedTrees {
    let text = format!("pub const VALUE: {carrier} = {initializer}; const INTEGER: u64 = 0;");
    source_fixture(&text)
}

fn source_fixture(text: &str) -> TypedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), text.to_owned())
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
    let sources = Arc::new(sources);
    let evaluated = super::super::evaluate(syntax, Some(sources.clone()), &[], None)
        .expect("anonymous float declaration evaluates");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &evaluated,
            sources: Some(sources),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("float normalization receipt resolves");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("float normalization receipt types")
}

#[test]
fn anonymous_float_replay_retains_computed_match_selector_dependencies() {
    for (selector, initializer) in [
        (
            "const SELECTOR: bool = 2 > 1;",
            "match SELECTOR { true -> (16777216 + 1) - 16777216, false -> 1 / 10 }",
        ),
        (
            "const SELECTOR: u64 = 1 + 1;",
            "match SELECTOR { 2 -> (16777216 + 1) - 16777216, _ -> 1 / 10 }",
        ),
    ] {
        let typed = source_fixture(&format!("pub const VALUE: f32 = {initializer}; {selector}"));
        super::validate(&typed, None)
            .expect("anonymous match result retains ordinary selector custody");
        let selected = typed
            .const_declarations()
            .iter()
            .find(|constant| typed.symbols.name(constant.symbol) == "VALUE")
            .expect("float result declaration");
        assert_eq!(
            selected.canonical_value_encoding.as_deref(),
            Some("float:f32:3f800000")
        );

        let mut changed = typed.clone();
        let dependency = changed
            .const_declarations()
            .iter()
            .find(|constant| changed.symbols.name(constant.symbol) == "SELECTOR")
            .expect("selector declaration")
            .materialized_initializer;
        let replacement = match changed.expression_table.expression(dependency) {
            ExpressionNode::Boolean(_) => ExpressionNode::Boolean(false),
            ExpressionNode::Integer(literal) => {
                let replacement = numerics::literals::IntegerLiteral::from_value(3);
                ExpressionNode::Integer(match literal.landing() {
                    Some(landing) => replacement.with_landing(landing),
                    None => replacement,
                })
            }
            _ => panic!("scalar selector"),
        };
        *changed.expression_table.expression_mut(dependency) = replacement;
        let diagnostics = super::validate(&changed, None)
            .expect_err("changed selector value must not preserve a stale copied selection");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("dependency or its substituted use drifted")),
            "{diagnostics:?}"
        );
        for erase in [false, true] {
            let mut changed = typed.clone();
            let declaration = changed
                .const_declarations()
                .iter()
                .find(|constant| changed.symbols.name(constant.symbol) == "SELECTOR")
                .expect("selector declaration")
                .clone();
            if erase {
                let span = changed.roots.const_declarations;
                changed
                    .tables
                    .const_declarations
                    .span_mut_or_empty(span)
                    .iter_mut()
                    .find(|constant| constant.symbol == declaration.symbol)
                    .unwrap()
                    .authored_initializer = Default::default();
            } else {
                let ExpressionNode::Binary(binary) = changed
                    .expression_table
                    .expression(declaration.authored_initializer)
                else {
                    panic!("authored selector computation");
                };
                let operand = binary.right;
                *changed.expression_table.expression_mut(operand) =
                    ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(3));
            }
            let diagnostics = super::validate(&changed, None)
                .expect_err("selector authored evidence must independently match its value");
            let expected = if erase {
                "lost its exact authored initializer"
            } else {
                "authored computation drifted"
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn anonymous_float_replay_recomputes_both_formats_without_index_atoms() {
    for (carrier, initializer, encoding) in [
        ("f32", "(16777216 + 1) - 16777216", "float:f32:3f800000"),
        (
            "f64",
            "(9007199254740992 + 1) - 9007199254740992",
            "float:f64:3ff0000000000000",
        ),
        ("f32", "1 / 10", "float:f32:3dcccccd"),
        ("f64", "1 / 10", "float:f64:3fb999999999999a"),
    ] {
        let typed = fixture(carrier, initializer);
        super::validate(&typed, None).expect("independent anonymous float replay");
        assert_eq!(
            typed.const_declarations()[0]
                .canonical_value_encoding
                .as_deref(),
            Some(encoding)
        );
        assert!(
            language_semantics::const_value::CanonicalConstIdentity {
                type_name: carrier.to_owned(),
                encoding: encoding.to_owned(),
            }
            .decode_encoding()
            .is_none(),
            "float materialization must not become a generic-index atom"
        );
    }
}

#[test]
fn float_replay_admits_ordinary_calls_selecting_anonymous_match_results() {
    for (declarations, initializer) in [
        (
            "machine truth() -> bool { true } const SELECTOR: bool = truth();",
            "match SELECTOR { true -> 1 / 3, false -> 2 / 3 }",
        ),
        (
            "machine truth() -> bool { true }",
            "match truth() { true -> 1 / 3, false -> 2 / 3 }",
        ),
        (
            "machine choose(value: u64) -> u64 { value }",
            "match choose(1 + 1) { 2 -> 1 / 3, _ -> 2 / 3 }",
        ),
    ] {
        let typed = source_fixture(&format!(
            "{declarations} pub const VALUE: f32 = {initializer};"
        ));
        super::validate(&typed, None)
            .expect("ordinary admitted selector calls retain exact anonymous result landing");
        let value = typed
            .const_declarations()
            .iter()
            .find(|constant| typed.symbols.name(constant.symbol) == "VALUE")
            .unwrap();
        assert_eq!(
            value.canonical_value_encoding.as_deref(),
            Some("float:f32:3eaaaaab")
        );
    }
}

#[test]
fn float_replay_rechecks_concrete_selector_call_admission() {
    let mut typed = source_fixture(
        "machine truth(value: u64) -> bool crashes Trap value == 0
         { transition { value != 0 -> 10 / value > 0 } crash Trap; }
         pub const VALUE: f32 = match truth(2) { true -> 1 / 3, false -> 2 / 3 };",
    );
    super::validate(&typed, None).expect("nonzero selector invocation is admitted");
    let original = typed.const_declarations()[0].authored_initializer;
    let ExpressionNode::Match(dispatch) = typed.expression_table.expression(original) else {
        panic!("authored match");
    };
    let ExpressionNode::Call(call) = typed.expression_table.expression(dispatch.subject) else {
        panic!("authored selector call");
    };
    let argument = typed.expression_table.expression_handles(call.arguments)[0];
    *typed.expression_table.expression_mut(argument) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(0));
    super::validate(&typed, None)
        .expect_err("zero-divisor call cannot supply an anonymous float selection");
}

#[test]
fn float_replay_requires_operator_free_match_authored_root() {
    let mut typed = fixture("f32", "match true { true -> 1, false -> 2 }");
    super::validate(&typed, None).expect("operator-free anonymous match replay");
    let span = typed.roots.const_declarations;
    typed.tables.const_declarations.span_mut_or_empty(span)[0].authored_initializer =
        Default::default();
    let diagnostics = super::validate(&typed, None)
        .expect_err("absence of operators cannot excuse an erased authored match");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("source-owned initializer roots")),
        "{diagnostics:?}"
    );
}

#[test]
fn float_literal_replay_rechecks_each_format_and_materialized_bits() {
    for (carrier, literal, encoding) in [
        ("f32", "1.5", "float:f32:3fc00000"),
        ("f64", "1.5f64", "float:f64:3ff8000000000000"),
        ("f32", "16777217", "float:f32:4b800000"),
    ] {
        let mut typed = fixture(carrier, literal);
        super::validate(&typed, None)
            .expect("uncomputed literal retains independent authored and materialized roots");
        let declaration = typed.const_declarations()[0].clone();
        assert_eq!(
            declaration.canonical_value_encoding.as_deref(),
            Some(encoding)
        );
        *typed
            .expression_table
            .expression_mut(declaration.materialized_initializer) =
            ExpressionNode::Float(FloatLiteral::parse("2.0").unwrap());
        let diagnostics = super::validate(&typed, None)
            .expect_err("literal materialization must match its authored value and receipt");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("literal bits or result encoding drifted")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn float_literal_replay_rejects_coordinated_payloadless_nan_receipts() {
    for (carrier, format) in [("f32", FloatFormat::F32), ("f64", FloatFormat::F64)] {
        let mut typed = fixture(carrier, "1.5");
        super::validate(&typed, None).expect("finite literal replay");
        let declaration = typed.const_declarations()[0].clone();
        let literal = FloatLiteral::parse("NaN")
            .expect("payloadless NaN meaning")
            .with_landing(format);
        let encoding = match format {
            FloatFormat::F32 => format!("float:f32:{:08x}", literal.value_f32().to_bits()),
            FloatFormat::F64 => format!("float:f64:{:016x}", literal.value_f64().to_bits()),
        };
        *typed
            .expression_table
            .expression_mut(declaration.authored_initializer) =
            ExpressionNode::Float(literal.clone());
        *typed
            .expression_table
            .expression_mut(declaration.materialized_initializer) = ExpressionNode::Float(literal);
        let span = typed.roots.const_declarations;
        typed.tables.const_declarations.span_mut_or_empty(span)[0].canonical_value_encoding =
            Some(encoding);
        let diagnostics = super::validate(&typed, None)
            .expect_err("matching NaN roots and receipt do not determine materializable bits");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("requires explicit NaN representation bits")),
            "{carrier}: {diagnostics:?}"
        );
    }
}

#[test]
fn anonymous_float_replay_rejects_forged_results_and_erased_custody() {
    for (carrier, format, changed_encoding) in [
        ("f32", FloatFormat::F32, "float:f32:40000000"),
        ("f64", FloatFormat::F64, "float:f64:4000000000000000"),
    ] {
        let typed = fixture(carrier, "1.0 + 0.5");
        super::validate(&typed, None).expect("unchanged float replay");
        let declaration = typed.const_declarations()[0].clone();
        for mutation in [
            "result",
            "result-and-encoding",
            "authored-value",
            "authored-root",
            "operator-custody",
            "format",
            "declared-carrier",
            "forged-literal-computation",
        ] {
            let mut changed = typed.clone();
            match mutation {
                "result" | "result-and-encoding" => {
                    *changed
                        .expression_table
                        .expression_mut(declaration.materialized_initializer) =
                        ExpressionNode::Float(
                            FloatLiteral::parse("2.0").unwrap().with_landing(format),
                        );
                    if mutation == "result-and-encoding" {
                        let span = changed.roots.const_declarations;
                        changed.tables.const_declarations.span_mut_or_empty(span)[0]
                            .canonical_value_encoding = Some(changed_encoding.to_owned());
                    }
                }
                "authored-value" => {
                    let ExpressionNode::Binary(binary) = changed
                        .expression_table
                        .expression(declaration.authored_initializer)
                    else {
                        panic!("authored addition");
                    };
                    let operand = binary.right;
                    *changed.expression_table.expression_mut(operand) =
                        ExpressionNode::Float(FloatLiteral::parse("1.0").unwrap());
                }
                "authored-root" => {
                    let span = changed.roots.const_declarations;
                    changed.tables.const_declarations.span_mut_or_empty(span)[0]
                        .authored_initializer = Default::default();
                }
                "operator-custody" => {
                    let literal = changed
                        .expression_table
                        .expression(declaration.materialized_initializer)
                        .clone();
                    let value = changed.expression_table.insert(literal);
                    changed
                        .expression_table
                        .set_source_span(value, declaration.initializer_source_span);
                    let span = changed.roots.const_declarations;
                    changed.tables.const_declarations.span_mut_or_empty(span)[0]
                        .materialized_initializer = value;
                }
                "format" => {
                    let wrong_format = match format {
                        FloatFormat::F32 => FloatFormat::F64,
                        FloatFormat::F64 => FloatFormat::F32,
                    };
                    *changed
                        .expression_table
                        .expression_mut(declaration.materialized_initializer) =
                        ExpressionNode::Float(
                            FloatLiteral::parse("1.5")
                                .unwrap()
                                .with_landing(wrong_format),
                        );
                }
                "declared-carrier" => {
                    let integer_type = changed
                        .const_declarations()
                        .iter()
                        .find(|constant| changed.symbols.name(constant.symbol) == "INTEGER")
                        .expect("integer carrier control")
                        .declared_type;
                    let span = changed.roots.const_declarations;
                    changed.tables.const_declarations.span_mut_or_empty(span)[0].declared_type =
                        integer_type;
                }
                "forged-literal-computation" => {
                    *changed
                        .expression_table
                        .expression_mut(declaration.authored_initializer) =
                        ExpressionNode::Float(FloatLiteral::parse("2.0").unwrap());
                    *changed
                        .expression_table
                        .expression_mut(declaration.materialized_initializer) =
                        ExpressionNode::Float(FloatLiteral::parse("2.0").unwrap());
                    let span = changed.roots.const_declarations;
                    changed.tables.const_declarations.span_mut_or_empty(span)[0]
                        .canonical_value_encoding = Some(changed_encoding.to_owned());
                }
                _ => unreachable!(),
            }
            let diagnostics = super::validate(&changed, None)
                .expect_err("mutated float normalization must reject");
            let expected = match mutation {
                "authored-root" => "distinct source-owned initializer roots",
                "operator-custody" => "operator custody drifted",
                "format" => "materialized format drifted",
                "declared-carrier" => "exact declared format",
                "forged-literal-computation" => {
                    "literal root retains computation or declaration custody"
                }
                _ => "materialized bits, or result encoding drifted",
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{carrier} mutation {mutation} must reject at its replay check: {diagnostics:?}"
            );
        }
    }
}

#[test]
fn anonymous_float_replay_does_not_repair_a_zero_divisor_with_forged_bits() {
    let mut typed = fixture("f32", "1 / 2");
    super::validate(&typed, None).expect("defined division replay");
    let declaration = typed.const_declarations()[0].clone();
    let ExpressionNode::Binary(binary) = typed
        .expression_table
        .expression(declaration.authored_initializer)
    else {
        panic!("authored division");
    };
    let divisor = binary.right;
    *typed.expression_table.expression_mut(divisor) =
        ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(0));
    assert!(super::validate(&typed, None).is_err());
}
