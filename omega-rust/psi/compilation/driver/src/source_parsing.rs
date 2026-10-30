//! Source frontiers become syntax through one Psi-owned lexical/grammar sequence.
//!
//! Packages choose and admit sources; this owner loads the supplied view, lexes,
//! parses and publishes pages. Appending keeps all existing handles stable.
//! Generated strata can instead request independent units, which prevents their
//! later normalization from synthesizing across sibling custody boundaries.

mod batches;

use crate::{PsiCompilationObserver, PsiCompilationStage};
use diagnostics::Diagnostic;
use source::{DependencyScope, LoadedSources, SourceContents, SourceFileSystem};
use std::path::PathBuf;
use syntax_trees::{ParsedSources, SyntaxTrees};

/// Load the complete frontier before parsing, preserving read-error precedence.
/// The package owner assigns custody and coordinates to these exact bytes before
/// returning them as `SourceInput`; parser order never allocates source IDs.
pub fn load_source_files(
    frontier: Vec<(PathBuf, DependencyScope)>,
    filesystem: &SourceFileSystem,
    observer: &mut impl PsiCompilationObserver,
) -> Result<Vec<SourceContents>, Vec<Diagnostic>> {
    observer.record_result(PsiCompilationStage::SourceParsing, || {
        source_files_to_tokens::load_sources(frontier, filesystem)
    })
}

/// Admitted source instances with coordinates assigned by their source owner.
/// Parsing neither discovers imports nor acquires additional package authority.
pub struct SourceInput(pub LoadedSources);

impl SourceInput {
    /// Append a frontier as owned parser pages, transferring rather than copying
    /// their arenas. Returned roots refer to the caller's extended syntax forest.
    pub fn append(
        self,
        syntax: &mut SyntaxTrees,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ParsedSources, Vec<Diagnostic>> {
        let first_page = u32::try_from(syntax.page_count())
            .ok()
            .and_then(|identity| identity.checked_add(1))
            .ok_or_else(|| vec![Diagnostic::error("syntax page identity overflow")])?;
        let (pages, sources) = self.parse_batch(first_page, false, observer)?;
        for page in pages {
            syntax.adopt_page(page);
        }
        Ok(sources)
    }

    /// Preserve one forest per generated unit for independent normalization.
    pub fn into_units(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Vec<SyntaxTrees>, Vec<Diagnostic>> {
        self.parse_batch(1, true, observer).map(|(pages, _)| pages)
    }

    fn parse_batch(
        self,
        first_page: u32,
        independent_units: bool,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
        observer.record_result(PsiCompilationStage::SourceParsing, || {
            batches::parse(self.0, first_page, independent_units)
        })
    }
}

/// A standalone editor/projector result. Tokens retain spelling and comments;
/// syntax supplies the grammar. Neither result establishes source semantics.
pub struct ParsedText<'text> {
    pub tokens: tokens::TokenStream<'text>,
    pub syntax: SyntaxTrees,
}

#[derive(Debug)]
pub enum SourceParseError {
    Lex { message: String },
    Parse { message: String },
}

/// Parse a borrowed snippet for source editing or declaration projection. The
/// same ordinary lexer and parser run here; no heuristic grammar is substituted.
pub fn parse_text(text: &str) -> Result<ParsedText<'_>, SourceParseError> {
    let tokens = source_files_to_tokens::Lexer::new(text)
        .tokenize()
        .map_err(|error| SourceParseError::Lex {
            message: error.message,
        })?;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).map_err(|error| {
        SourceParseError::Parse {
            message: error.message,
        }
    })?;
    Ok(ParsedText { tokens, syntax })
}
