//! Parse token streams into unresolved syntax trees, preserving source order and spans.
//! `parse` publishes one source's root items through `declarations::items`.
//! Batch entry points below retain source metadata and schedule independent syntax pages.

#![forbid(unsafe_code)]

pub use errors::parse_error::ParseError;

use crate::declarations::items::{ParsedDeclaration, parse_item};
use crate::token_input::cursor::Input;
use arena::{Arena, ConcurrentPagedArena, Handle, SealedPagedArena};
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source::{SourceId, SourcePosition};
use syntax_trees::{ParsedRootItems, ParsedSource, ParsedSources, SyntaxTrees, item::Item};
use tokens::{LexedSources, TokenStream};

pub fn parse(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &TokenStream<'_>,
) -> Result<ParsedRootItems, ParseError> {
    let retained_source = Input::retain_source(tokens);
    let mut input = Input::new(source_id, tokens, &retained_source, syntax_trees);
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
            ParsedDeclaration::Mathematical(definition) => {
                syntax_trees.push_root_mathematical_definition(definition);
            }
        }
        input = rest;
    }

    Ok(roots)
}

pub fn parse_syntax_trees(tokens: &TokenStream<'_>) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &TokenStream<'_>,
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse(&mut syntax_trees, source_id, tokens)?;
    Ok(syntax_trees)
}

pub fn parse_sources(
    mut lexed: LexedSources,
    syntax_trees: &mut SyntaxTrees,
) -> Result<ParsedSources, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_mut_or_empty(lexed.batch);
    let source_count = lexed_sources.len();
    let mut parsed_sources = Arena::with_capacity(source_count);
    let mut parsed = Vec::with_capacity(source_count);

    for lexed_source in lexed_sources {
        parsed.push(parse_recorded_source(syntax_trees, lexed_source)?);
    }

    let batch = parsed_sources.insert_many(parsed);

    Ok(ParsedSources {
        sources: parsed_sources,
        batch,
    })
}

pub struct ParsedSyntaxSource {
    source: ParsedSource,
    syntax: Handle<SyntaxTrees>,
}

impl ParsedSyntaxSource {
    pub const fn source(&self) -> &ParsedSource {
        &self.source
    }

    pub const fn syntax_handle(&self) -> Handle<SyntaxTrees> {
        self.syntax
    }
}

pub struct ParsedSyntaxPages {
    pages: SealedPagedArena<SyntaxTrees>,
    sources: Vec<ParsedSyntaxSource>,
}

impl ParsedSyntaxPages {
    pub fn sources(&self) -> &[ParsedSyntaxSource] {
        &self.sources
    }

    pub fn syntax(&self, handle: Handle<SyntaxTrees>) -> Option<&SyntaxTrees> {
        self.pages.try_get(handle)
    }

    pub fn syntax_for<T>(&self, handle: Handle<T>) -> Option<&SyntaxTrees> {
        self.syntax(Handle::from_parts(1, handle.generation()))
    }

    pub fn page_count(&self) -> usize {
        self.pages.page_count()
    }

    pub fn into_parts(self) -> (Vec<SyntaxTrees>, ParsedSources) {
        let trees = self.pages.into_pages().into_iter().flatten().collect();
        let mut sources = Arena::with_capacity(self.sources.len());
        let batch = sources.insert_many(self.sources.into_iter().map(|source| source.source));
        (trees, ParsedSources { sources, batch })
    }
}

pub fn parse_source_pages_from(
    mut lexed: LexedSources,
    first_page_identity: u32,
) -> Result<ParsedSyntaxPages, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_mut_or_empty(lexed.batch);
    let source_count = lexed_sources.len();
    let desired_pages = source_count
        .min(execution_admission::current_workers().saturating_mul(4))
        .max(1);
    parse_source_pages_with_count(lexed_sources, desired_pages, first_page_identity)
}

pub fn parse_source_units(mut lexed: LexedSources) -> Result<ParsedSyntaxPages, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_mut_or_empty(lexed.batch);
    let desired_pages = lexed_sources.len().max(1);
    parse_source_pages_with_count(lexed_sources, desired_pages, 1)
}

fn parse_source_pages_with_count(
    lexed_sources: &mut [tokens::LexedSource],
    desired_pages: usize,
    first_page_identity: u32,
) -> Result<ParsedSyntaxPages, Vec<Diagnostic>> {
    let source_count = lexed_sources.len();
    let chunk_size = source_count.max(1).div_ceil(desired_pages);
    let page_count = source_count.div_ceil(chunk_size);
    let arena = ConcurrentPagedArena::<SyntaxTrees>::new_with_first_page(
        desired_pages,
        1,
        first_page_identity,
    );
    let page_reservations = (0..page_count)
        .map(|_| {
            arena.reserve_page().ok_or_else(|| {
                vec![Diagnostic::error(
                    "parser exhausted its source-page directory",
                )]
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let parsed_pages = execution_admission::within_budget(|| {
        page_reservations
            .into_par_iter()
            .zip(lexed_sources.par_chunks_mut(chunk_size))
            .map(|(page, sources)| parse_source_page(page, sources))
            .collect::<Vec<_>>()
    })
    .into_iter()
    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;

    let mut parsed_sources = Vec::with_capacity(source_count);
    for (syntax, sources) in parsed_pages {
        parsed_sources.extend(
            sources
                .into_iter()
                .map(|source| ParsedSyntaxSource { source, syntax }),
        );
    }

    let pages = arena.seal().map_err(|error| {
        vec![Diagnostic::error(format!(
            "parser page publication incomplete: {} of {} pages published",
            error.published_pages, error.reserved_pages
        ))]
    })?;
    Ok(ParsedSyntaxPages {
        pages,
        sources: parsed_sources,
    })
}

fn parse_source_page(
    mut page: arena::ConcurrentArenaPage<'_, SyntaxTrees>,
    sources: &mut [tokens::LexedSource],
) -> Result<(Handle<SyntaxTrees>, Vec<ParsedSource>), Vec<Diagnostic>> {
    let page_identity = page.page_identity();
    let first_source = sources
        .first()
        .map(|source| source.source_id)
        .unwrap_or_default();
    let mut syntax = SyntaxTrees::new_with_generation(first_source, page_identity);
    let mut parsed = Vec::with_capacity(sources.len());

    for lexed_source in sources {
        parsed.push(parse_recorded_source(&mut syntax, lexed_source)?);
    }

    let syntax = page.push(syntax);
    page.publish();
    Ok((syntax, parsed))
}

fn parse_recorded_source(
    syntax_trees: &mut SyntaxTrees,
    lexed_source: &mut tokens::LexedSource,
) -> Result<ParsedSource, Vec<Diagnostic>> {
    let root_items =
        parse(syntax_trees, lexed_source.source_id, &lexed_source.tokens).map_err(|error| {
            let position =
                SourcePosition::of(lexed_source.source.as_ref(), error.source_span.span.start);
            vec![Diagnostic::error(format!(
                "{}:{}:{}: {}",
                lexed_source.path.display(),
                position.line,
                position.column,
                error.message
            ))]
        })?;
    Ok(ParsedSource {
        source_id: lexed_source.source_id,
        path: std::mem::take(&mut lexed_source.path),
        source: lexed_source.source.clone(),
        origin: lexed_source.origin,
        root_items: root_items.items,
        import_root_range: root_items.import_range,
        scope: lexed_source.scope,
    })
}

mod syntax_storage {
    pub(crate) mod handle_spans;
    pub(crate) mod statement_paths;
    pub(crate) mod tables;
}
mod machine_bodies {
    pub(crate) mod defaults;
    pub(crate) mod states;
}
mod statements {
    pub(crate) mod bindings;
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
    #[path = "../tests/unit/generic_parameters.rs"]
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
    #[path = "../tests/unit/static_targets.rs"]
    mod static_targets_tests;
}
mod assembly {
    pub(crate) mod generated_statements;
    pub(crate) mod parse;
}
mod errors {
    pub(crate) mod expected;
    pub(crate) mod parse_error;
}
mod token_input {
    pub(crate) mod cursor;
    pub(crate) mod delimiters;
    pub(crate) mod paths;
}
mod literals {
    pub(crate) mod numbers;
}
mod patterns {
    pub(crate) mod bindings;
    pub(crate) mod fields;
}
mod types {
    pub(crate) mod properties;
    #[cfg(test)]
    #[path = "../tests/unit/qualified_types.rs"]
    mod qualified_names;
    pub(crate) mod references;
    #[cfg(test)]
    #[path = "../tests/unit/type_remainders.rs"]
    mod remainder_tests;
    #[cfg(test)]
    #[path = "../tests/unit/static_argument_delimiters.rs"]
    mod static_argument_delimiters_tests;
}

#[cfg(test)]
#[path = "tests/unit/mod.rs"]
mod tests;
