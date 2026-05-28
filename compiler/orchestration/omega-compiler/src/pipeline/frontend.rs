use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::pipeline::source::SourceStorage;
use crate::{lexer, parser};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::source::SourceId;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{Item, ItemHandle};
use omega_tokens::{Token, TokenStream, TokenText};

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
    pub root_items: Vec<ItemHandle>,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            root_items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedSources {
    pub sources: Arena<ParsedSource>,
    pub batch: HandleSpan<ParsedSource>,
}

pub fn load_sources(
    frontier: Vec<PathBuf>,
    first_source_id: usize,
) -> Result<LoadedSources, Vec<Diagnostic>> {
    let source_count = frontier.len();
    let mut sources = Arena::with_capacity(source_count);
    let mut loaded = Vec::with_capacity(source_count);

    for (index, path) in frontier.into_iter().enumerate() {
        let source = std::fs::read_to_string(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                path.display()
            ))]
        })?;

        loaded.push(LoadedSource {
            source_id: SourceId(first_source_id + index),
            path,
            source: Arc::from(source),
        });
    }

    let batch = sources.insert_many(loaded);

    Ok(LoadedSources { sources, batch })
}

pub fn lex_sources(sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let mut lexed = Vec::with_capacity(source_count);

    for loaded_source in loaded_sources {
        let tokens = lexer::Lexer::new(loaded_source.source.as_ref())
            .tokenize()
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "{}: {}",
                    loaded_source.path.display(),
                    error.message
                ))]
            })?;

        lexed.push(LexedSource {
            source_id: loaded_source.source_id,
            path: loaded_source.path.clone(),
            source: loaded_source.source.clone(),
            tokens: own_token_stream(&tokens, &loaded_source.source),
        });
    }

    let batch = lexed_sources.insert_many(lexed);

    Ok(LexedSources {
        sources: lexed_sources,
        batch,
    })
}

pub fn parse_sources(
    lexed: LexedSources,
    syntax_trees: &mut SyntaxTrees,
) -> Result<ParsedSources, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_or_empty(lexed.batch);
    let source_count = lexed_sources.len();
    let mut parsed_sources = Arena::with_capacity(source_count);
    let mut parsed = Vec::with_capacity(source_count);

    for lexed_source in lexed_sources {
        let root_items = parser::parse_syntax_trees_into_with_id(
            syntax_trees,
            lexed_source.source_id,
            &lexed_source.tokens,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: {}",
                lexed_source.path.display(),
                error.message
            ))]
        })?;

        parsed.push(ParsedSource {
            source_id: lexed_source.source_id,
            path: lexed_source.path.clone(),
            source: lexed_source.source.clone(),
            root_items,
        });
    }

    let batch = parsed_sources.insert_many(parsed);

    Ok(ParsedSources {
        sources: parsed_sources,
        batch,
    })
}

pub fn discover_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    root_path: &Path,
    selected_target_name: Option<&str>,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let root_dir = root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let parsed_sources = parsed.sources.span_or_empty(parsed.batch);
    let mut imports = Vec::with_capacity(parsed_sources.len());

    for parsed_source in parsed_sources {
        for root_item in &parsed_source.root_items {
            let item = syntax_trees.root_item(*root_item);
            match item {
                Item::Use(use_item) => {
                    imports.push(normalize_path(&resolve_source_path(
                        &root_dir,
                        syntax_trees.items.identifier_path_members(use_item.path),
                    ))?);
                }
                Item::Target(target) => {
                    let target_is_selected = selected_target_name
                        .is_none_or(|target_name| target.name.as_str() == target_name);

                    if !target_is_selected {
                        continue;
                    }

                    if let Some(host) = &target.host {
                        let provider = syntax_trees.items.identifier_path_members(host.provider);
                        if is_bundled_omega_path(provider) {
                            imports
                                .push(normalize_path(&resolve_source_path(&root_dir, provider))?);
                        }
                    }

                    for boundary_policy in syntax_trees
                        .items
                        .boundary_policies(target.boundary_policies)
                    {
                        let policy_path = syntax_trees
                            .items
                            .identifier_path_members(boundary_policy.path);
                        if is_bundled_omega_path(policy_path) {
                            imports.push(normalize_path(&resolve_source_path(
                                &root_dir,
                                policy_path,
                            ))?);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(imports)
}

pub fn extend_source_storage(
    source_storage: &mut SourceStorage,
    parsed: ParsedSources,
) -> Result<(), Vec<Diagnostic>> {
    source_storage.extend(parsed)
}

fn own_token_stream(tokens: &TokenStream<'_>, source: &Arc<str>) -> TokenStream<'static> {
    let tokens = tokens.as_slice();
    let mut owned_tokens = Vec::with_capacity(tokens.len());

    for token in tokens {
        let lexeme = match &token.lexeme {
            TokenText::Source(_) => TokenText::shared(source.clone(), token.span),
            TokenText::Shared { source, span } => TokenText::shared(source.clone(), *span),
            TokenText::Owned(value) => TokenText::owned(value.clone()),
        };

        owned_tokens.push(Token {
            kind: token.kind,
            lexeme,
            span: token.span,
        });
    }

    TokenStream::new(owned_tokens)
}

fn resolve_source_path(root_dir: &Path, source_path: &[Identifier]) -> PathBuf {
    let mut segments = source_path.iter();
    let mut path = if is_bundled_omega_path(source_path) {
        segments.next();
        bundled_omega_root()
    } else {
        root_dir.to_path_buf()
    };

    for segment in segments {
        path.push(segment.as_str());
    }

    for candidate in source_path_candidates(&path) {
        if candidate.exists() {
            return candidate;
        }
    }

    source_path_candidates(&path)
        .into_iter()
        .next()
        .unwrap_or(path)
}

fn is_bundled_omega_path(path: &[Identifier]) -> bool {
    path.first()
        .is_some_and(|segment| segment.as_str() == "omega")
}

fn bundled_omega_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../omega")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../omega"))
}

fn normalize_path(path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    path.canonicalize().map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to resolve {}: {error}",
            path.display()
        ))]
    })
}

fn source_path_candidates(base_path: &Path) -> Vec<PathBuf> {
    let mut file_omg = base_path.to_path_buf();
    file_omg.set_extension("omg");

    let mut file_omega = base_path.to_path_buf();
    file_omega.set_extension("omega");

    vec![
        file_omg,
        base_path.join("mod.omg"),
        file_omega,
        base_path.join("mod.omega"),
    ]
}
