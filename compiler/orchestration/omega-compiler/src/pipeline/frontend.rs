use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::pipeline::source::SourceStorage;
use crate::{lexer, parser};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::source::SourceId;
use omega_syntax_trees::identifier::IdentifierPath;
use omega_syntax_trees::item::Item;
use omega_syntax_trees::SyntaxTrees;
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
    pub syntax_trees: SyntaxTrees,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            syntax_trees: SyntaxTrees {
                source_id: SourceId::default(),
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

pub fn load_sources(
    frontier: Vec<PathBuf>,
    first_source_id: usize,
) -> Result<LoadedSources, Vec<Diagnostic>> {
    let mut sources = Arena::new();
    let loaded = frontier
        .into_iter()
        .enumerate()
        .map(|(index, path)| {
            let source = std::fs::read_to_string(&path).map_err(|error| {
                Diagnostic::error(format!("failed to read {}: {error}", path.display()))
            })?;

            Ok(LoadedSource {
                source_id: SourceId(first_source_id + index),
                path,
                source: Arc::from(source),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(|diagnostic| vec![diagnostic])?;

    let batch = sources.insert_many(loaded);

    Ok(LoadedSources { sources, batch })
}

pub fn lex_sources(sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let mut lexed_sources = Arena::new();
    let batch = lexed_sources.insert_many(
        sources
            .sources
            .span_or_empty(sources.batch)
            .iter()
            .map(|loaded_source| {
                let tokens = lexer::Lexer::new(loaded_source.source.as_ref())
                    .tokenize()
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "{}: {}",
                            loaded_source.path.display(),
                            error.message
                        ))
                    })?;

                Ok(LexedSource {
                    source_id: loaded_source.source_id,
                    path: loaded_source.path.clone(),
                    source: loaded_source.source.clone(),
                    tokens: own_token_stream(&tokens),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()
            .map_err(|diagnostic| vec![diagnostic])?,
    );

    Ok(LexedSources {
        sources: lexed_sources,
        batch,
    })
}

pub fn parse_sources(lexed: LexedSources) -> Result<ParsedSources, Vec<Diagnostic>> {
    let mut parsed_sources = Arena::new();
    let batch = parsed_sources.insert_many(
        lexed.sources
            .span_or_empty(lexed.batch)
            .iter()
            .map(|lexed_source| {
                let syntax_trees = parser::parse_syntax_trees_with_id(
                    lexed_source.source_id,
                    &lexed_source.tokens,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "{}: {}",
                        lexed_source.path.display(),
                        error.message
                    ))
                })?;

                Ok(ParsedSource {
                    source_id: lexed_source.source_id,
                    path: lexed_source.path.clone(),
                    source: lexed_source.source.clone(),
                    syntax_trees,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()
            .map_err(|diagnostic| vec![diagnostic])?,
    );

    Ok(ParsedSources {
        sources: parsed_sources,
        batch,
    })
}

pub fn discover_imports(
    parsed: &ParsedSources,
    root_path: &Path,
    selected_target_name: Option<&str>,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let root_dir = root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut imports = Vec::new();

    for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
        for item in &parsed_source.syntax_trees.items {
            match item {
                Item::Use(use_item) => {
                    imports.push(normalize_path(&resolve_source_path(&root_dir, &use_item.path))?);
                }
                Item::Target(target) => {
                    let target_is_selected = selected_target_name
                        .is_none_or(|target_name| target.name.as_str() == target_name);

                    if !target_is_selected {
                        continue;
                    }

                    if let Some(host) = &target.host {
                        if is_bundled_omega_path(&host.provider) {
                            imports.push(normalize_path(&resolve_source_path(
                                &root_dir,
                                &host.provider,
                            ))?);
                        }
                    }

                    for trust_policy in &target.trust_policies {
                        if is_bundled_omega_path(&trust_policy.path) {
                            imports.push(normalize_path(&resolve_source_path(
                                &root_dir,
                                &trust_policy.path,
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

fn own_token_stream(tokens: &TokenStream<'_>) -> TokenStream<'static> {
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

fn resolve_source_path(root_dir: &Path, source_path: &IdentifierPath) -> PathBuf {
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

fn is_bundled_omega_path(path: &IdentifierPath) -> bool {
    path.first().is_some_and(|segment| segment.as_str() == "omega")
}

fn bundled_omega_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../omega")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../omega")
        })
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
