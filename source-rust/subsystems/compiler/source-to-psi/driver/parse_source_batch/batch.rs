//! Run stages 00 and 01 for each fresh file and place retained files in the same input order.
use arena::Arena;
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source_files::LoadedSource;
use source_files::LoadedSources;
use syntax_trees::{ParsedSource, ParsedSources, ParsedUnit, SyntaxTrees};

#[derive(Clone, Copy)]
pub(super) enum Layout<'a> {
    Shared,
    Independent,
    Reuse(&'a [Option<&'a ParsedUnit>]),
}

#[derive(Default)]
struct ParsedPageSources {
    sources: Vec<ParsedSource>,
    lexical_error: Option<Diagnostic>,
    parse_error: Option<Diagnostic>,
    reuse_error: Option<Diagnostic>,
}

pub(super) fn parse(
    mut loaded: LoadedSources,
    first_page: u32,
    layout: Layout<'_>,
) -> Result<(Vec<SyntaxTrees>, ParsedSources), Vec<Diagnostic>> {
    let sources = loaded.sources.span_mut_or_empty(loaded.batch);
    let count = sources.len();
    let page_ends = page_ends(count, layout);
    let page_count = page_ends.len();
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
    let restore_or_parse = |page_index: usize,
                            first_file: usize,
                            sources: &mut [LoadedSource],
                            syntax: &mut SyntaxTrees| {
        if let Layout::Reuse(units) = layout
            && let Some(unit) = units[first_file]
        {
            let [file] = sources else {
                unreachable!("a retained file owns one page")
            };
            let mut page = ParsedPageSources::default();
            match unit.at_coordinates(file.source_id, first_page + page_index as u32) {
                Ok((relocated, root_items)) => {
                    *syntax = relocated;
                    page.sources.push(ParsedSource {
                        source_id: file.source_id,
                        path: std::mem::take(&mut file.path),
                        source: file.source.clone(),
                        origin: file.origin,
                        scope: file.scope,
                        root_items,
                        import_root_range: unit.import_root_range.clone(),
                    });
                }
                Err(reason) => page.reuse_error = Some(Diagnostic::error(reason)),
            }
            return page;
        }
        parse_page(sources, syntax)
    };
    let mut page_files = Vec::with_capacity(page_count);
    let mut remaining = sources;
    let mut page_start = 0;
    for &page_end in &page_ends {
        let (files, rest) = std::mem::take(&mut remaining).split_at_mut(page_end - page_start);
        page_files.push((page_start, files));
        remaining = rest;
        page_start = page_end;
    }
    let mut trees: Vec<_> = page_files
        .iter()
        .enumerate()
        .map(|(index, (_, files))| {
            SyntaxTrees::new_with_generation(files[0].source_id, first_page + index as u32)
        })
        .collect();
    let mut pages = Vec::with_capacity(if page_count == 1 { 0 } else { page_count });
    let mut single_page_sources = worker_budget::within_budget(|| {
        if let ([syntax], [(first_file, files)]) = (trees.as_mut_slice(), page_files.as_mut_slice())
        {
            Some(restore_or_parse(0, *first_file, files, syntax))
        } else {
            page_files
                .par_iter_mut()
                .zip_eq(trees.par_iter_mut())
                .enumerate()
                .map(|(index, ((first_file, files), syntax))| {
                    restore_or_parse(index, *first_file, files, syntax)
                })
                .collect_into_vec(&mut pages);
            None
        }
    });
    for page in single_page_sources.iter_mut().chain(&mut pages) {
        if let Some(error) = page.lexical_error.take() {
            return Err(diagnostics::DiagnosticStage::Lexing.attribute(vec![error]));
        }
    }
    for page in single_page_sources.iter_mut().chain(&mut pages) {
        if let Some(error) = page.parse_error.take() {
            return Err(diagnostics::DiagnosticStage::Parsing.attribute(vec![error]));
        }
    }
    for page in single_page_sources.iter_mut().chain(&mut pages) {
        if let Some(error) = page.reuse_error.take() {
            return Err(diagnostics::DiagnosticStage::Parsing.attribute(vec![error]));
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

/// Return the exclusive end of each page's file range, in input order.
///
/// A retained file needs a page of its own because its relocated parse
/// replaces the whole page. Fresh files share pages of the size the shared
/// layout would give the fresh files alone, so a few retained files split
/// the fresh pages only where they interrupt them instead of turning every
/// fresh file into a single-file page.
fn page_ends(count: usize, layout: Layout<'_>) -> Vec<usize> {
    let retained = |index: usize| matches!(layout, Layout::Reuse(units) if units[index].is_some());
    let fresh = (0..count).filter(|&index| !retained(index)).count();
    let files_per_page = if matches!(layout, Layout::Independent) {
        1
    } else {
        let pages = fresh
            .min(worker_budget::current_workers().saturating_mul(4))
            .max(1);
        fresh.max(1).div_ceil(pages)
    };
    // Each retained file adds its own page and may end one fresh page early.
    let mut ends = Vec::with_capacity(fresh.div_ceil(files_per_page) + 2 * (count - fresh));
    let mut page_start = 0;
    for index in 0..count {
        let retained = retained(index);
        if retained && index > page_start {
            ends.push(index);
        }
        if retained || index + 1 - page_start == files_per_page {
            ends.push(index + 1);
            page_start = index + 1;
        }
    }
    if page_start < count {
        ends.push(count);
    }
    ends
}

fn parse_page(sources: &mut [LoadedSource], syntax: &mut SyntaxTrees) -> ParsedPageSources {
    let mut page = ParsedPageSources {
        sources: Vec::with_capacity(sources.len()),
        ..Default::default()
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
