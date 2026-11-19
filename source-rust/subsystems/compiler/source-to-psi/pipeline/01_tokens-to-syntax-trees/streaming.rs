//! Stream admitted source units into ordered syntax pages.

mod batches {
    use arena::Arena;
    use diagnostics::Diagnostic;
    use rayon::prelude::*;
    use source_files::LoadedSource;
    use source_files::LoadedSources;
    use syntax_trees::{ParsedSource, ParsedSources, SyntaxTrees};

    struct ParsedPageSources {
        sources: Vec<ParsedSource>,
        lexical_error: Option<Diagnostic>,
        parse_error: Option<Diagnostic>,
    }

    pub(super) fn parse(
        mut loaded: LoadedSources,
        first_page: u32,
        independent_units: bool,
    ) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
        let sources = loaded.sources.span_mut_or_empty(loaded.batch);
        let count = sources.len();
        let desired_pages = if independent_units {
            count.max(1)
        } else {
            count
                .min(worker_budget::current_workers().saturating_mul(4))
                .max(1)
        };
        let chunk_size = count.max(1).div_ceil(desired_pages);
        let page_count = count.div_ceil(chunk_size);
        if first_page == 0
            || (page_count != 0
                && u32::try_from(page_count - 1)
                    .ok()
                    .and_then(|last| first_page.checked_add(last))
                    .is_none())
        {
            return Err(vec![Diagnostic::error(
                "parser exhausted its source-page directory",
            )]);
        }
        let mut trees: Vec<_> = sources
            .chunks(chunk_size)
            .enumerate()
            .map(|(index, chunk)| {
                SyntaxTrees::new_with_generation(chunk[0].source_id, first_page + index as u32)
            })
            .collect();
        let mut pages = Vec::with_capacity(if page_count == 1 { 0 } else { page_count });
        let mut single_page_sources = worker_budget::within_budget(|| {
            if let [syntax] = trees.as_mut_slice() {
                Some(parse_page(sources, syntax))
            } else {
                sources
                    .par_chunks_mut(chunk_size)
                    .zip_eq(trees.par_iter_mut())
                    .map(|(sources, syntax)| parse_page(sources, syntax))
                    .collect_into_vec(&mut pages);
                None
            }
        });
        for page in single_page_sources.iter_mut().chain(&mut pages) {
            if let Some(error) = page.lexical_error.take() {
                return Err(vec![error]);
            }
        }
        for page in single_page_sources.iter_mut().chain(&mut pages) {
            if let Some(error) = page.parse_error.take() {
                return Err(vec![error]);
            }
        }
        let mut parsed = Arena::with_capacity(count);
        let batch = parsed.insert_many(
            single_page_sources
                .into_iter()
                .chain(pages)
                .flat_map(|page| page.sources),
        );
        Ok((
            trees,
            ParsedSources {
                sources: parsed,
                batch,
            },
        ))
    }

    fn parse_page(sources: &mut [LoadedSource], syntax: &mut SyntaxTrees) -> ParsedPageSources {
        let mut page = ParsedPageSources {
            sources: Vec::with_capacity(sources.len()),
            lexical_error: None,
            parse_error: None,
        };
        // Keep this frontier streamed: lex and parse one admitted unit,
        // then recycle its token buffer for the next. After a parse error
        // we still scan later units to preserve lexical-error precedence.
        let mut scratch = tokens::TokenScratch::default();
        for source in sources {
            let mut file = match source_lexer::lex_file(source, &mut scratch) {
                Ok(file) => file,
                Err(error) => {
                    page.lexical_error = Some(error.into_diagnostic());
                    break;
                }
            };
            if page.parse_error.is_none() {
                match source_parser::parse_file(syntax, &mut file) {
                    Ok(parsed) => page.sources.push(parsed),
                    Err(error) => page.parse_error = Some(error.into_diagnostic()),
                }
            }
            file.tokens.recycle(&mut scratch);
        }
        page
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use source_text::SourceId;
        use std::sync::Arc;

        fn loaded(texts: &[&str]) -> LoadedSources {
            texts
                .iter()
                .enumerate()
                .map(|(index, text)| LoadedSource {
                    source_id: SourceId(index),
                    path: format!("source-{index}.omg").into(),
                    source: Arc::from(*text),
                    ..Default::default()
                })
                .collect()
        }

        #[test]
        fn fused_batch_preserves_lexical_error_priority_and_source_order() {
            for texts in [
                vec!["machine", "\"unterminated"],
                vec!["machine", "data Good {}", "\"unterminated"],
            ] {
                let input = loaded(&texts);
                let expected = vec![
                    crate::lexing::lex(input.clone())
                        .unwrap_err()
                        .into_diagnostic(),
                ];
                let actual = parse(input, 1, false).unwrap_err();
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
            }
            let mut same_page = loaded(&["machine", "\"unterminated"]);
            let mut syntax = SyntaxTrees::new_with_generation(SourceId(0), 1);
            let page = parse_page(
                same_page.sources.span_mut_or_empty(same_page.batch),
                &mut syntax,
            );
            assert!(page.lexical_error.is_some());
            assert!(page.parse_error.is_some());
            let input = loaded(&["machine", "data"]);
            let lexed = crate::lexing::lex(input.clone()).unwrap();
            let expected = vec![
                crate::parsing::parse(&lexed, Default::default())
                    .err()
                    .unwrap()
                    .into_diagnostic(),
            ];
            let actual = parse(input, 1, false).unwrap_err();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }

        #[test]
        fn fused_batch_matches_page_identity_metadata_and_owned_literals() {
            let texts = (0..97)
                .map(|index| format!("const Text{index}: [u8] = \"value\\n{index}\";"))
                .collect::<Vec<_>>();
            let input = loaded(&texts.iter().map(String::as_str).collect::<Vec<_>>());
            let lexed = crate::lexing::lex(input.clone()).unwrap();
            let (expected_pages, expected_sources) = crate::parsing::parse(
                &lexed,
                crate::parsing::ParseOptions {
                    first_page_identity: 5,
                    ..Default::default()
                },
            )
            .unwrap()
            .into_parts();
            let (actual_pages, actual_sources) = parse(input, 5, false).unwrap();
            assert_eq!(
                format!("{actual_sources:?}"),
                format!("{expected_sources:?}")
            );
            assert_eq!(actual_pages.len(), expected_pages.len());
            for (actual, expected) in actual_pages.iter().zip(&expected_pages) {
                assert_eq!(actual.snapshot(), expected.snapshot());
            }
            let units = parse(loaded(&["data First {}", "data Second {}"]), 1, true)
                .unwrap()
                .0;
            assert_eq!(units.len(), 2);
            assert_eq!(units[0].root_item_handles()[0].generation(), 1);
            assert_eq!(units[1].root_item_handles()[0].generation(), 2);
        }
    }
}

use crate::{PsiCompilationObserver, PsiCompilationStage};
use arena::Arena;
use diagnostics::Diagnostic;
use source_files::LoadedSource;
use source_files::LoadedSources;
use syntax_trees::{ParsedSource, ParsedSources, ParsedUnit, SyntaxTrees};

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
            batches::parse(self.0, first_page, independent_units)
        })
    }
}

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod batch_tests;
#[cfg(test)]
#[path = "../../tests/unit/reused_sources.rs"]
mod reuse_tests;
