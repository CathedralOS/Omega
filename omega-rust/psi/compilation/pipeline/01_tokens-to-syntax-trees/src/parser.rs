pub use crate::diagnostics::parse_error;

use crate::ParseError;
use crate::declarations::parse_declaration::{ParsedDeclaration, parse_item};
use crate::input::token_cursor::Input;
use arena::{Arena, ConcurrentPagedArena, Handle, SealedPagedArena};
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source::{SourceId, SourcePosition};
use syntax_trees::{ParsedRootItems, ParsedSource, ParsedSources, SyntaxTrees, item::Item};
use tokens::{LexedSources, TokenStream};

/// Parse an ordered token batch into the caller's syntax arena, retaining
/// per-source roots, backing text and metadata. Consumed path buffers move into
/// the result while text stays shared with syntax payloads. Failure leaves the already
/// parsed prefix in that arena and reports the current source's exact location.
pub fn parse_sources(
    mut lexed: LexedSources,
    syntax_trees: &mut SyntaxTrees,
) -> Result<ParsedSources, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_mut_or_empty(lexed.batch);
    let source_count = lexed_sources.len();
    let mut parsed_sources = Arena::with_capacity(source_count);
    let mut parsed = Vec::with_capacity(source_count);

    for lexed_source in lexed_sources {
        let root_items = parse(syntax_trees, lexed_source.source_id, &lexed_source.tokens)
            .map_err(|error| {
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

        parsed.push(ParsedSource {
            source_id: lexed_source.source_id,
            path: std::mem::take(&mut lexed_source.path),
            source: lexed_source.source.clone(),
            origin: lexed_source.origin,
            root_items: root_items.items,
            import_root_range: root_items.import_range,
            scope: lexed_source.scope,
        });
    }

    let batch = parsed_sources.insert_many(parsed);

    Ok(ParsedSources {
        sources: parsed_sources,
        batch,
    })
}

/// One parsed source and the worker-owned syntax page containing its roots.
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

/// Ordered source metadata over independently published parser pages.
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

    /// Resolve any syntax handle through the page identity carried in its
    /// generation coordinate. Every parser page stores its `SyntaxTrees` at
    /// slot one.
    pub fn syntax_for<T>(&self, handle: Handle<T>) -> Option<&SyntaxTrees> {
        self.syntax(Handle::from_parts(1, handle.generation()))
    }

    pub fn page_count(&self) -> usize {
        self.pages.page_count()
    }

    /// Consume worker pages into their source-chunk order without copying any
    /// syntax tree or its arena allocations.
    pub fn into_parts(self) -> (Vec<SyntaxTrees>, ParsedSources) {
        let trees = self.pages.into_pages().into_iter().flatten().collect();
        let mut sources = Arena::with_capacity(self.sources.len());
        let batch = sources.insert_many(self.sources.into_iter().map(|source| source.source));
        (trees, ParsedSources { sources, batch })
    }
}

/// Parse one ordered lexical frontier into bounded worker-owned pages. Tasks
/// receive contiguous source chunks, so Rayon collection restores authored
/// source order without sorting by completion or allocation identity.
pub fn parse_source_pages_from(
    mut lexed: LexedSources,
    first_page_identity: u32,
) -> Result<ParsedSyntaxPages, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_mut_or_empty(lexed.batch);
    let source_count = lexed_sources.len();
    let desired_pages = source_count
        .min(rayon::current_num_threads().max(1).saturating_mul(4))
        .max(1);
    parse_source_pages_with_count(lexed_sources, desired_pages, first_page_identity)
}

/// Parse each source into its own published syntax unit. Generated-source
/// extension custody uses this route because normalization must not synthesize
/// across generated-unit boundaries.
pub fn parse_source_units(mut lexed: LexedSources) -> Result<ParsedSyntaxPages, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_mut_or_empty(lexed.batch);
    let desired_pages = lexed_sources.len().max(1);
    parse_source_pages_with_count(lexed_sources, desired_pages, 1)
}

/// Stack budget for the parallel parse workers. Expression and type parsing
/// recurse on authored nesting depth, so page workers need the same budget
/// the compile thread provisions (`COMPILE_STACK_SIZE` in the driver), not
/// the host's default thread stack. The stage owns this pool so every
/// caller — CLI, tests, and embedding drivers — gets the contract without
/// configuring the process-wide Rayon pool.
const PARSE_STACK_SIZE: usize = 256 * 1024 * 1024;

/// Run source parsing under the grammar's required stack budget. The facade
/// may combine lexical scratch and parsing in one independently owned task.
pub fn with_parse_workers<Output: Send>(operation: impl FnOnce() -> Output + Send) -> Output {
    parse_thread_pool().install(operation)
}

fn parse_thread_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .thread_name(|index| format!("omega-parse-{index}"))
            .stack_size(PARSE_STACK_SIZE)
            .build()
            .expect("parser worker pool must spawn")
    })
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

    let parsed_pages = parse_thread_pool()
        .install(|| {
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
        let root_items =
            parse(&mut syntax, lexed_source.source_id, &lexed_source.tokens).map_err(|error| {
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
        parsed.push(ParsedSource {
            source_id: lexed_source.source_id,
            path: std::mem::take(&mut lexed_source.path),
            source: lexed_source.source.clone(),
            origin: lexed_source.origin,
            root_items: root_items.items,
            import_root_range: root_items.import_range,
            scope: lexed_source.scope,
        });
    }

    let syntax = page.push(syntax);
    page.publish();
    Ok((syntax, parsed))
}

/// Parses one source into the caller's arena and returns that source's root
/// item handles and import bounds. Top-level `let`/`boundary let` declarations land on
/// `SyntaxTreeRoots::mathematical_definitions` instead — they are not items —
/// so they do not appear in the returned list. Existing roots retain their
/// identities. On error, the arena retains the parsed prefix.
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

/// Creates a syntax arena for an anonymous source and parses its tokens.
pub fn parse_syntax_trees(tokens: &TokenStream<'_>) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

/// Creates a syntax arena for one identified source and parses its tokens.
pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &TokenStream<'_>,
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse(&mut syntax_trees, source_id, tokens)?;
    Ok(syntax_trees)
}
