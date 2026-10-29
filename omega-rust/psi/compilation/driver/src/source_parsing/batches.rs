//! Each worker owns token capacity and publishes complete syntax pages. Source
//! order fixes page identities; completion order never determines handles. Lexical
//! failures retain batch priority over parse failures, even in an earlier source.
use arena::Arena;
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source::{LoadedSource, LoadedSources, SourcePosition};
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
            .min(rayon::current_num_threads().max(1).saturating_mul(4))
            .max(1)
    };
    let chunk_size = count.max(1).div_ceil(desired_pages);
    let page_count = count.div_ceil(chunk_size);
    // Check the complete identity interval before constructing any page. The
    // empty batch consumes no generation, including when first_page is zero.
    if page_count != 0
        && (first_page == 0
            || u32::try_from(page_count - 1)
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
    // The forest header stays in its final allocation. Workers return only
    // source/error metadata; source order still fixes page and diagnostic order.
    let mut pages = Vec::with_capacity(if page_count == 1 { 0 } else { page_count });
    // A single page needs no metadata vector or parallel collection. It still
    // parses on the dedicated pool's stack, just like larger source batches.
    let mut single_page_sources = tokens_to_syntax_trees::parser::with_parse_workers(|| {
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
    let mut scratch = tokens::TokenScratch::default();
    for source in sources {
        let tokens = match source_files_to_tokens::Lexer::new(&source.source)
            .tokenize_reusing(&mut scratch)
        {
            Ok(tokens) => tokens.retain_source(source.source.clone()),
            Err(error) => {
                page.lexical_error = Some(located_error(source, error.span.start, error.message));
                break;
            }
        };
        if page.parse_error.is_none() {
            match tokens_to_syntax_trees::parse(syntax, source.source_id, &tokens) {
                Ok(roots) => page.sources.push(ParsedSource {
                    source_id: source.source_id,
                    path: std::mem::take(&mut source.path),
                    source: source.source.clone(),
                    origin: source.origin,
                    root_items: roots.items,
                    import_root_range: roots.import_range,
                    scope: source.scope,
                }),
                Err(error) => {
                    page.parse_error = Some(located_error(
                        source,
                        error.source_span.span.start,
                        error.message,
                    ))
                }
            }
        }
        tokens.recycle(&mut scratch);
    }
    page
}

fn located_error(source: &LoadedSource, start: usize, message: String) -> Diagnostic {
    let position = SourcePosition::of(&source.source, start);
    Diagnostic::error(format!(
        "{}:{}:{}: {}",
        source.path.display(),
        position.line,
        position.column,
        message
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::SourceId;
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
            let expected = source_files_to_tokens::lex_sources(input.clone()).unwrap_err();
            let actual = parse(input, 1, false).unwrap_err();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }
        // Several sources share one page; a preceding parse failure must not
        // prevent the task from discovering a later lexical failure.
        let mut same_page = loaded(&["machine", "\"unterminated"]);
        let mut syntax = SyntaxTrees::new_with_generation(SourceId(0), 1);
        let page = parse_page(
            same_page.sources.span_mut_or_empty(same_page.batch),
            &mut syntax,
        );
        assert!(page.lexical_error.is_some());
        assert!(page.parse_error.is_some());
        let input = loaded(&["machine", "data"]);
        let lexed = source_files_to_tokens::lex_sources(input.clone()).unwrap();
        let expected = tokens_to_syntax_trees::parse_source_pages_from(lexed, 1)
            .err()
            .unwrap();
        let actual = parse(input, 1, false).unwrap_err();
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    }

    #[test]
    fn fused_batch_matches_page_identity_metadata_and_owned_literals() {
        let texts = (0..97)
            .map(|index| format!("const Text{index}: [u8] = \"value\\n{index}\";"))
            .collect::<Vec<_>>();
        let input = loaded(&texts.iter().map(String::as_str).collect::<Vec<_>>());
        let lexed = source_files_to_tokens::lex_sources(input.clone()).unwrap();
        let (expected_pages, expected_sources) =
            tokens_to_syntax_trees::parse_source_pages_from(lexed, 5)
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
