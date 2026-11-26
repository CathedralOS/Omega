//! Parse token streams into unresolved syntax trees, preserving source order and spans.
//! `parse` owns tree creation, source metadata, page scheduling and publication.
//! Grammar helpers and partial tree construction are private to this stage.

#![forbid(unsafe_code)]

pub use errors::parse_error::ParseError;

use crate::declarations::items::{ParsedDeclaration, parse_item};
use crate::token_input::cursor::Input;
use arena::{ConcurrentPagedArena, Handle};
use rayon::prelude::*;
use source_text::{SourceId, SourcePosition};
use syntax_trees::{
    ParsedRootItems, ParsedSource, ParsedSyntaxPages, ParsedSyntaxSource, SyntaxTrees, item::Item,
};
use tokens::{LexedSources, TokenStream};

/// How source trees may share a page. Independent units can be retained and adopted separately.
#[derive(Debug, Clone, Copy, Default)]
pub enum ParseLayout {
    #[default]
    SharedPages,
    IndependentUnits,
}

/// Storage coordinates for this parse. The compiler assigns the first unused page identity.
#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    pub first_page_identity: u32,
    pub layout: ParseLayout,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            first_page_identity: 1,
            layout: ParseLayout::SharedPages,
        }
    }
}

/// Borrows a complete token batch and returns owned syntax pages or its first source error.
/// Callers neither prepare mutable trees nor assemble roots and source records.
pub fn parse(lexed: &LexedSources, options: ParseOptions) -> Result<ParsedSyntaxPages, ParseError> {
    let sources = lexed.sources.span_or_empty(lexed.batch);
    let desired_pages = match options.layout {
        ParseLayout::SharedPages => sources
            .len()
            .min(worker_budget::current_workers().saturating_mul(4))
            .max(1),
        ParseLayout::IndependentUnits => sources.len().max(1),
    };
    parse_source_pages_with_count(sources, desired_pages, options.first_page_identity)
}

pub fn parse_into(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &TokenStream<'_>,
) -> Result<ParsedRootItems, ParseError> {
    let qualifiers = token_input::file_qualifiers::FileQualifiers::default();
    let mut input = Input::new(source_id, tokens, syntax_trees);
    input.file_qualifiers = Some(&qualifiers);
    let mut roots = ParsedRootItems::default();
    while !input.tokens.is_empty() {
        let (declaration, rest) = parse_item(syntax_trees, input)?;
        match declaration {
            ParsedDeclaration::Item(item) => {
                if matches!(item, Item::Use(_)) {
                    if roots.import_range.is_empty() {
                        roots.import_range.start = roots.items.len();
                    }
                    roots.import_range.end = roots.items.len() + 1;
                }
                roots.items.push(syntax_trees.push_root_item(item));
            }
            ParsedDeclaration::DataFamily(items) => {
                for item in items {
                    roots.items.push(syntax_trees.push_root_item(item));
                }
            }
            ParsedDeclaration::Mathematical(definition) => {
                syntax_trees.push_root_mathematical_definition(definition);
            }
        }
        input = rest;
    }

    crate::declarations::imports::reject_packed_contract_imports(syntax_trees, &roots)?;
    if let Some(site) = qualifiers.packed_instruction() {
        crate::declarations::imports::import_packed_contracts(syntax_trees, &mut roots, site);
    }
    crate::declarations::imports::check_file_aliases(syntax_trees, &roots, tokens.source_text())?;
    crate::declarations::imports::mark_product_operand_imports(syntax_trees, &roots, &qualifiers);
    Ok(roots)
}

fn parse_source_pages_with_count(
    lexed_sources: &[tokens::LexedSource],
    desired_pages: usize,
    first_page_identity: u32,
) -> Result<ParsedSyntaxPages, ParseError> {
    let source_count = lexed_sources.len();
    let chunk_size = source_count.max(1).div_ceil(desired_pages);
    let page_count = source_count.div_ceil(chunk_size);
    // Chunk rounding may use fewer pages than requested. Validate the same
    // capacity we reserve, including the empty arena's one directory slot.
    let arena_pages = page_count.max(1);
    if first_page_identity == 0
        || u32::try_from(arena_pages)
            .ok()
            .and_then(|count| first_page_identity.checked_add(count - 1))
            .is_none()
    {
        // Page identities run from `first_page_identity` to `u32::MAX`; zero
        // names no page. The first source past the last identity has no page.
        let placed_pages = match first_page_identity {
            0 => 0,
            first => (u32::MAX - first) as usize + 1,
        };
        return Err(unplaced_source_error(
            "parser exhausted its source-page directory",
            lexed_sources.get(placed_pages.saturating_mul(chunk_size)),
        ));
    }
    let arena = ConcurrentPagedArena::<SyntaxTrees>::new_with_first_page(
        arena_pages,
        1,
        first_page_identity,
    );
    let page_reservations = (0..page_count)
        .map(|page| {
            arena.reserve_page().ok_or_else(|| {
                unplaced_source_error(
                    "parser exhausted its source-page directory",
                    lexed_sources.get(page * chunk_size),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let parsed_pages = worker_budget::within_budget(|| {
        // Keep the large parsing stack, but one page needs no parallel dispatch.
        if page_count <= 1 {
            page_reservations
                .into_iter()
                .zip(lexed_sources.chunks(chunk_size))
                .map(|(page, sources)| parse_source_page(page, sources))
                .collect::<Result<Vec<_>, ParseError>>()
        } else {
            page_reservations
                .into_par_iter()
                .zip(lexed_sources.par_chunks(chunk_size))
                .map(|(page, sources)| parse_source_page(page, sources))
                .collect::<Vec<_>>()
                .into_iter()
                .collect::<Result<Vec<_>, ParseError>>()
        }
    })?;

    let mut parsed_sources = Vec::with_capacity(source_count);
    for (syntax, sources) in parsed_pages {
        parsed_sources.extend(
            sources
                .into_iter()
                .map(|source| ParsedSyntaxSource { source, syntax }),
        );
    }

    // The arena does not say which page is missing, so this reports the
    // batch: the first token of its first source.
    let pages = arena.seal().map_err(|error| {
        unplaced_source_error(
            format!(
                "parser page publication incomplete: {} of {} pages published",
                error.published_pages, error.reserved_pages
            ),
            lexed_sources.first(),
        )
    })?;
    Ok(ParsedSyntaxPages {
        pages,
        sources: parsed_sources,
    })
}

/// A page-directory fault is a limit of this parse, not a mistake in a
/// file, so no token is wrong. The error points at where parsing stopped:
/// the first token of the first source that got no page, or the end of that
/// source when it holds no token. An empty batch has no source to point at,
/// and its error carries only the message.
fn unplaced_source_error(
    message: impl Into<String>,
    unplaced: Option<&tokens::LexedSource>,
) -> ParseError {
    let Some(source) = unplaced else {
        return ParseError::at_source_span(message, source_text::SourceSpan::default());
    };
    let text = source.tokens.source_text();
    let span = source
        .tokens
        .as_slice()
        .iter()
        .find(|token| !token.is_trivia())
        .map_or(source_text::Span::new(text.len(), text.len()), |token| {
            token.span
        });
    let mut error = ParseError::at_source_span(
        message,
        source_text::SourceSpan::new(source.source_id, span),
    );
    error.location = Some((source.path.clone(), SourcePosition::of(text, span.start)));
    error
}

fn parse_source_page(
    mut page: arena::ConcurrentArenaPage<'_, SyntaxTrees>,
    sources: &[tokens::LexedSource],
) -> Result<(Handle<SyntaxTrees>, Vec<ParsedSource>), ParseError> {
    let page_identity = page.page_identity();
    let first_source = sources
        .first()
        .map(|source| source.source_id)
        .unwrap_or_default();
    let mut syntax = SyntaxTrees::new_with_generation(first_source, page_identity);
    let mut parsed = Vec::with_capacity(sources.len());

    for lexed_source in sources {
        parsed.push(parse_recorded_source(
            &mut syntax,
            lexed_source,
            lexed_source.path.clone(),
        )?);
    }

    let syntax = page.push(syntax);
    page.publish();
    Ok((syntax, parsed))
}

/// Transfer the streamed file's path and roots to its syntax record. The
/// caller can recycle its tokens as soon as this call returns.
pub fn parse_file(
    syntax_trees: &mut SyntaxTrees,
    lexed_source: &mut tokens::LexedSource,
) -> Result<ParsedSource, ParseError> {
    let path = std::mem::take(&mut lexed_source.path);
    parse_recorded_source(syntax_trees, lexed_source, path)
}

fn parse_recorded_source(
    syntax_trees: &mut SyntaxTrees,
    lexed_source: &tokens::LexedSource,
    path: std::path::PathBuf,
) -> Result<ParsedSource, ParseError> {
    let root_items = parse_into(syntax_trees, lexed_source.source_id, &lexed_source.tokens)
        .map_err(|mut error| {
            let position = SourcePosition::of(
                lexed_source.tokens.source_text(),
                error.source_span.span.start,
            );
            error.location = Some((path.clone(), position));
            error
        })?;
    Ok(ParsedSource {
        source_id: lexed_source.source_id,
        path,
        source: lexed_source.tokens.shared_source(),
        origin: lexed_source.origin,
        root_items: root_items.items,
        import_root_range: root_items.import_range,
        scope: lexed_source.scope,
    })
}

mod statements {
    pub(crate) mod destructuring;
    pub(crate) mod expressions;
    pub(crate) mod local_data;
    pub(crate) mod parse;
    pub(crate) mod sequence;
}
mod transitions {
    pub(crate) mod blocks;
    pub(crate) mod guards;
    pub(crate) mod targets;
}
mod contracts {
    pub(crate) mod bindings;
    pub(crate) mod clauses;
    pub(crate) mod conformance;
    pub(crate) mod effects;
    pub(crate) mod facts;
    pub(crate) mod signatures;
}
mod parameters {
    pub(crate) mod callable;
    #[cfg(test)]
    #[path = "../../tests/unit/generic_parameters.rs"]
    mod generic_parameters_tests;
    pub(crate) mod generics;
    pub(crate) mod relevance;
    pub(crate) mod requirements;
}
mod declarations {
    pub(crate) mod capabilities;
    pub(crate) mod conformances;
    pub(crate) mod constants;
    pub(crate) mod data;
    pub(crate) mod domains;
    pub(crate) mod imports;
    pub(crate) mod items;
    pub(crate) mod machine_bodies;
    pub(crate) mod machines;
    pub(crate) mod mathematical_bindings;
    pub(crate) mod measures;
    pub(crate) mod namespaces;
    pub(crate) mod operators;
    pub(crate) mod propositions;
    pub(crate) mod traits;
}
mod expressions {
    pub(crate) mod context;
    pub(crate) mod membership;
    pub(crate) mod operators;
    pub(crate) mod parse;
    pub(crate) mod postfix;
    pub(crate) mod primary;
    pub(crate) mod proof_facts;
    pub(crate) mod static_arguments;
    #[cfg(test)]
    #[path = "../../tests/unit/static_targets.rs"]
    mod static_targets_tests;
}
mod assembly {
    pub(crate) mod parse;
}
mod errors {
    pub(crate) mod expected;
    pub(crate) mod parse_error;
}
mod token_input {
    pub(crate) mod file_qualifiers;
    pub(crate) mod cursor;
    pub(crate) mod delimiters;
}
mod literals {
    pub(crate) mod numbers;
}
mod patterns {
    pub(crate) mod bindings;
    pub(crate) mod fields;
    pub(crate) mod positional;
}
mod types {
    pub(crate) mod properties;
    #[cfg(test)]
    #[path = "../../tests/unit/qualified_types.rs"]
    mod qualified_names;
    pub(crate) mod references;
    #[cfg(test)]
    #[path = "../../tests/unit/type_remainders.rs"]
    mod remainder_tests;
    #[cfg(test)]
    #[path = "../../tests/unit/static_argument_delimiters.rs"]
    mod static_argument_delimiters_tests;
}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/support/internal.rs"]
mod test_support;
#[cfg(test)]
use test_support::{
    parse_source_units, parse_sources, parse_syntax_trees, parse_syntax_trees_with_id,
};

#[cfg(test)]
use crate as parsing;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_files::LoadedSource;
#[cfg(test)]
#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
