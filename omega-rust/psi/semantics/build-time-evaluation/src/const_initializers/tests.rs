use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{ConstDefinition, Item};
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

fn parse(text: &str) -> (SyntaxTrees, Arc<SourceMap>) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), text.to_owned())
        .source_id;
    let tokens = Lexer::new(text).tokenize().expect("initializer tokens");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("initializer syntax");
    (syntax, Arc::new(sources))
}

fn evaluate(text: &str) -> Result<SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    let (syntax, sources) = parse(text);
    super::evaluate(syntax, Some(sources), &[], None)
}

fn constant<'syntax>(syntax: &'syntax SyntaxTrees, name: &str) -> &'syntax ConstDefinition {
    syntax
        .root_items()
        .find_map(|item| match item {
            Item::Const(definition) if definition.name.as_str() == name => Some(definition),
            _ => None,
        })
        .expect("named constant")
}

fn integer_encoding(syntax: &SyntaxTrees, name: &str, value: i128) {
    let definition = constant(syntax, name);
    let receipt = definition
        .normalization
        .as_ref()
        .expect("evaluated declaration");
    assert_eq!(
        receipt.canonical_result_encoding,
        CanonicalConstIdentity::integer("u64", value).encoding
    );
    assert!(matches!(
        syntax.expressions.expression(definition.value),
        ExpressionNode::Integer(_)
    ));
}

#[test]
fn anonymous_fractional_intermediate_lands_once_and_retains_authored_expression() {
    let (syntax, sources) = parse("const SIZE: u64 = 7 / 2 * 2;");
    let original = constant(&syntax, "SIZE").value;
    let original_expression = syntax.expressions.expression(original).clone();
    let original_span = syntax.expressions.source_span(original);
    let evaluated =
        super::evaluate(syntax, Some(sources), &[], None).expect("integral final landing");
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
    let evaluated = super::evaluate(syntax, Some(sources), &[], None).expect("forward dependency");
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
fn landed_float_leaves_cannot_be_reinterpreted_as_anonymous_scalar_operands() {
    for text in [
        "const COUNT: u64 = 1.5f32 * 2;",
        "const FLAG: bool = 0.5f32 < 1;",
    ] {
        let errors =
            evaluate(text).expect_err("landed float operands remain outside the scalar evaluator");
        assert!(!errors.is_empty(), "{text}");
    }
}
