//! Lexing, parsing and snapshotting shared by this stage's integration tests.
//! A source that must parse panics with the parser's message and the text its
//! span covers, so a failing case names what the parser refused.
#![allow(dead_code)]

use crate::frontend_fixture::{Lexer, parse_syntax_trees};
use serde_json::Value;
use syntax_trees::SyntaxTrees;

pub(crate) fn parse(source: &str) -> Result<SyntaxTrees, source_parser::ParseError> {
    let stream = Lexer::new(source).tokenize().expect("the source lexes");
    parse_syntax_trees(&stream)
}

pub(crate) fn trees(source: &str) -> SyntaxTrees {
    parse(source).unwrap_or_else(|error| {
        let span = error.source_span.span;
        panic!(
            "{} at `{}` in {source:?}",
            error.message,
            &source[span.start..span.end]
        )
    })
}

pub(crate) fn snapshot(source: &str) -> Value {
    serde_json::to_value(trees(source).snapshot()).expect("a syntax snapshot serializes")
}
