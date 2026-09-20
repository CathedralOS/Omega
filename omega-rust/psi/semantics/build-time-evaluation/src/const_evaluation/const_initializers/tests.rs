//! Fixtures shared by the const initializer tests: parsing, evaluation and
//! encodings.

mod computed_leaves_and_replays;
mod concrete_invocations_and_initializers;
mod dependencies_and_aggregates;
mod generic_application_carriers;
mod generic_invocations;

use language_semantics::const_value::CanonicalConstIdentity;
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::ExpressionNode;
use syntax_trees::item::{ConstDefinition, Item};
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

fn parse(text: &str) -> (SyntaxTrees, Arc<SourceMap>) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("main.omg"), text.to_owned())
        .source_id;
    let tokens = Lexer::new(text).tokenize().expect("initializer tokens");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("initializer syntax");
    (syntax, Arc::new(sources))
}

fn parse_files(files: &[(&str, &str)]) -> (SyntaxTrees, Arc<SourceMap>, Vec<source::SourceId>) {
    let mut sources = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    let mut ids = Vec::new();
    for (path, text) in files {
        let source_id = sources
            .add(PathBuf::from(path), (*text).to_owned())
            .source_id;
        let tokens = Lexer::new(text).tokenize().expect("initializer tokens");
        parse_syntax_trees_into_with_id(&mut syntax, source_id, &tokens)
            .expect("initializer syntax");
        ids.push(source_id);
    }
    (syntax, Arc::new(sources), ids)
}

fn evaluate(text: &str) -> Result<SyntaxTrees, Vec<diagnostics::Diagnostic>> {
    let (syntax, sources) = parse(text);
    super::evaluate(syntax, Some(sources), &[], None)
}

/// Run the complete pre-resolution evaluation and typed receipt replay so
/// retained structured leaves re-derive their canonical results through the
/// checked interpreter rather than trusting the materialized literal.
fn evaluate_fully(
    files: &[(&str, &str)],
    bindings: &[symbols::SourceScopedTopLevelBinding],
) -> typed_trees::TypedTrees {
    let (syntax, sources, _) = parse_files(files);
    let evaluated = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
        syntax_trees: syntax,
        source_context: Some(crate::BuildTimeSourceContext {
            sources: sources.clone(),
            source_scoped_top_level_bindings: bindings,
            selection_authority: None,
            retained_base: None,
        }),
    })
    .expect("pre-resolution evaluation");
    let (syntax, pre_check) = evaluated.into_syntax_and_pre_check();
    let resolved =
        crate::machine_execution::syntax_probes::resolve(&syntax, Some(sources), bindings)
            .expect("resolve");
    let mut typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    pre_check.evaluate(&mut typed).expect("receipt replay");
    typed
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

fn array_leaves(
    syntax: &SyntaxTrees,
    expression: syntax_trees::expression::ExpressionHandle,
) -> Vec<ExpressionNode> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(elements) => syntax
            .expressions
            .expression_handles(*elements)
            .iter()
            .flat_map(|element| array_leaves(syntax, *element))
            .collect(),
        leaf => vec![leaf.clone()],
    }
}

fn literal_encoding(text: &str, name: &str) -> String {
    let (syntax, _) = parse(text);
    syntax_trees_to_symbol_resolved_trees::pre_resolution::canonicalize_declared_const_definition(
        &syntax,
        constant(&syntax, name),
    )
    .expect("literal array canonicalization")
    .encoding
}
