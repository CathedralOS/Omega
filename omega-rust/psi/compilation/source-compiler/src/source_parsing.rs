//! Sequence complete lexical and parsing stages, then admit or retain their results.

use crate::{PsiCompilationObserver, PsiCompilationStage};
use arena::Arena;
use diagnostics::Diagnostic;
use source::{DependencyScope, LoadedSource, LoadedSources, SourceContents, SourceFileSystem};
use std::path::PathBuf;
use syntax_trees::{ParsedSource, ParsedSources, ParsedUnit, SyntaxTrees};

pub fn load_source_files(
    frontier: Vec<(PathBuf, DependencyScope)>,
    filesystem: &SourceFileSystem,
    observer: &mut impl PsiCompilationObserver,
) -> Result<Vec<SourceContents>, Vec<Diagnostic>> {
    observer.record_result(PsiCompilationStage::SourceParsing, || {
        let mut loaded = Vec::with_capacity(frontier.len());

        for (path, scope) in frontier {
            let source = filesystem.read_to_string(&path).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "failed to read {}: {error}",
                    path.display()
                ))]
            })?;

            loaded.push(SourceContents {
                path,
                source,
                scope,
            });
        }

        Ok(loaded)
    })
}

pub struct SourceInput(pub LoadedSources);

impl SourceInput {
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

    pub fn into_units(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Vec<SyntaxTrees>, Vec<Diagnostic>> {
        self.parse_batch(1, true, observer).map(|(pages, _)| pages)
    }

    /// Parses each source into its own standalone page and retains it
    /// with its root items, so a later admission of the same bytes can
    /// adopt the parse through `append_reusing`.
    pub fn into_parsed_units(
        self,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<Vec<ParsedUnit>, Vec<Diagnostic>> {
        let (pages, mut sources) = self.parse_batch(1, true, observer)?;
        let parsed = sources.sources.span_mut_or_empty(sources.batch);
        if parsed.len() != pages.len() {
            return Err(vec![Diagnostic::error(
                "independent parse lost the page of a source",
            )]);
        }
        Ok(pages
            .into_iter()
            .zip(parsed)
            .map(|(syntax, source)| ParsedUnit {
                syntax,
                source: source.source.clone(),
                root_items: std::mem::take(&mut source.root_items),
                import_root_range: source.import_root_range.clone(),
            })
            .collect())
    }

    /// Appends these admitted sources to `syntax` as `append` does, except
    /// that a source for which `retained` supplies the parse of its exact
    /// bytes adopts that parse instead of being lexed and parsed again.
    /// The result lists every source in input order either way.
    pub fn append_reusing<'unit>(
        self,
        syntax: &mut SyntaxTrees,
        observer: &mut impl PsiCompilationObserver,
        mut retained: impl FnMut(&LoadedSource) -> Option<&'unit ParsedUnit>,
    ) -> Result<ParsedSources, Vec<Diagnostic>> {
        let loaded = self.0.sources.span_or_empty(self.0.batch);
        let units = loaded
            .iter()
            .map(|source| retained(source).filter(|unit| unit.source == source.source))
            .collect::<Vec<_>>();
        let fresh = Self(
            loaded
                .iter()
                .zip(&units)
                .filter(|(_, unit)| unit.is_none())
                .map(|(source, _)| source.clone())
                .collect(),
        )
        .append(syntax, observer)?;
        let mut fresh = fresh.sources.span_or_empty(fresh.batch).iter().cloned();
        let mut sources = Arena::with_capacity(loaded.len());
        let mut ordered = Vec::with_capacity(loaded.len());
        for (source, unit) in loaded.iter().zip(units) {
            ordered.push(match unit {
                Some(unit) => ParsedSource {
                    source_id: source.source_id,
                    path: source.path.clone(),
                    source: source.source.clone(),
                    origin: source.origin,
                    root_items: syntax
                        .adopt_unit(unit, source.source_id)
                        .map_err(|message| vec![Diagnostic::error(message)])?,
                    import_root_range: unit.import_root_range.clone(),
                    scope: source.scope,
                },
                None => fresh
                    .next()
                    .ok_or_else(|| vec![Diagnostic::error("parser lost an admitted source")])?,
            });
        }
        let batch = sources.insert_many(ordered);
        Ok(ParsedSources { sources, batch })
    }

    fn parse_batch(
        self,
        first_page: u32,
        independent_units: bool,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
        observer.record_result(PsiCompilationStage::SourceParsing, || {
            let tokens = source_files_to_tokens::lex(self.0)
                .map_err(|error| vec![error.into_diagnostic()])?;
            let options = tokens_to_syntax_trees::ParseOptions {
                first_page_identity: first_page,
                layout: if independent_units {
                    tokens_to_syntax_trees::ParseLayout::IndependentUnits
                } else {
                    tokens_to_syntax_trees::ParseLayout::SharedPages
                },
            };
            tokens_to_syntax_trees::parse(&tokens, options)
                .map(|parsed| parsed.into_parts())
                .map_err(|error| vec![error.into_diagnostic()])
        })
    }
}

#[cfg(test)]
#[path = "../tests/unit/source_parsing.rs"]
mod batch_tests;
#[cfg(test)]
#[path = "../tests/unit/reused_sources.rs"]
mod reuse_tests;

pub struct ParsedText<'text> {
    pub tokens: tokens::TokenStream<'text>,
    pub syntax: SyntaxTrees,
}

#[derive(Debug)]
pub enum SourceParseError {
    Lex { message: String },
    Parse { message: String },
}

pub fn parse_text(text: &str) -> Result<ParsedText<'_>, SourceParseError> {
    let loaded = [LoadedSource {
        source: std::sync::Arc::from(text),
        ..Default::default()
    }]
    .into_iter()
    .collect();
    let mut lexed = source_files_to_tokens::lex(loaded).map_err(|error| SourceParseError::Lex {
        message: error.message,
    })?;
    let parsed = tokens_to_syntax_trees::parse(&lexed, Default::default()).map_err(|error| {
        SourceParseError::Parse {
            message: error.message,
        }
    })?;
    let (mut pages, _) = parsed.into_parts();
    let syntax = pages.pop().expect("one source produces one syntax page");
    let tokens = std::mem::take(&mut lexed.sources.span_mut_or_empty(lexed.batch)[0].tokens);
    Ok(ParsedText { tokens, syntax })
}
