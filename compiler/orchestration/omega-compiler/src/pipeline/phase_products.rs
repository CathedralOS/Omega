use std::path::PathBuf;
use std::sync::Arc;

use crate::parser::SourceTrees;
use crate::source::SourceId;
use crate::tokens::{Token, TokenStream, TokenText};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSources {
    pub sources: Arena<LoadedSource>,
    pub batch: HandleSpan<LoadedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub tokens: TokenStream<'static>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexedSources {
    pub sources: Arena<LexedSource>,
    pub batch: HandleSpan<LexedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub source_trees: SourceTrees,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            source_trees: SourceTrees {
                source_id: SourceId::default(),
                items: Vec::new(),
                tables: Default::default(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedSources {
    pub sources: Arena<ParsedSource>,
    pub batch: HandleSpan<ParsedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssembledSyntax;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedProgram;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypedProgram;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidatedProgram;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendPlan;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmittedProgram;

pub type DiscoveredImports = Vec<PathBuf>;

pub fn own_token_stream(tokens: &TokenStream<'_>) -> TokenStream<'static> {
    let owned_tokens = tokens
        .as_slice()
        .iter()
        .map(|token| Token {
            kind: token.kind,
            lexeme: TokenText::owned(token.lexeme.as_str().to_owned()),
            span: token.span,
        })
        .collect();

    TokenStream::new(owned_tokens)
}
