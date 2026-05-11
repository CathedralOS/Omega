use std::path::PathBuf;
use std::sync::Arc;

use crate::ast::AstFile;
use crate::lexer::{Token, TokenStream, TokenText};
use crate::source::FileId;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSource {
    pub file_id: FileId,
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
    pub file_id: FileId,
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
    pub file_id: FileId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub ast: AstFile,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            file_id: FileId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            ast: AstFile {
                file_id: FileId::default(),
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
