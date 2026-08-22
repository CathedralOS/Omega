use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::pipeline::source::SourceStorage;
use crate::{lexer, parser};
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_source::{SourceId, SourcePosition};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{Item, ItemHandle};
use psi_tokens::{Token, TokenStream, TokenText};

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

/// Load a COMPILER-PROVIDED source (a virtual file with no on-disk backing):
/// the build-vocabulary prelude and its future siblings. The synthetic path
/// names the provider in diagnostics.
pub fn load_injected_source(name: &str, text: &str, first_source_id: usize) -> LoadedSources {
    let mut sources = Arena::with_capacity(1);
    let batch = sources.insert_many([LoadedSource {
        source_id: SourceId(first_source_id),
        path: PathBuf::from(name),
        source: Arc::from(text),
    }]);
    LoadedSources { sources, batch }
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
                let position = SourcePosition::of(loaded_source.source.as_ref(), error.span.start);
                vec![Diagnostic::error(format!(
                    "{}:{}:{}: {}",
                    loaded_source.path.display(),
                    position.line,
                    position.column,
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

/// depend-mapping (M2 blocker 3): collect `b.depend("alias", path("dir"))`
/// rows from every `machine build(b: &mut Build)` in this batch, resolving
/// the directory against the DECLARING FILE's parent (each package's
/// build.omg maps its own reach). Purely syntactic -- the vocabulary types
/// exist for validation; the toolchain reads the calls as data
/// (build_and_package_model.md: authored as code, consumed as data).
/// Out-of-band scan of a DEPENDED package's build.omg for ITS depend rows
/// (transitive reach). The file parses into a throwaway tree -- its `build`
/// machine never joins the program (each package has one; two would
/// collide). Unreadable/unparsable package builds are skipped here; the
/// package's own compile surfaces them.
fn collect_package_build_aliases(directory: &Path, depend_aliases: &mut Vec<(String, PathBuf)>) {
    let package_build = directory.join("build.omg");
    if !package_build.is_file() {
        return;
    }
    let Ok(loaded) = load_sources(vec![package_build], 0) else {
        return;
    };
    let Ok(lexed) = lex_sources(loaded) else {
        return;
    };
    let mut scratch = SyntaxTrees::default();
    let Ok(parsed) = parse_sources(lexed, &mut scratch) else {
        return;
    };
    collect_depend_aliases(&parsed, &scratch, depend_aliases);
}

pub fn collect_depend_aliases(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    depend_aliases: &mut Vec<(String, PathBuf)>,
) {
    use psi_syntax_trees::expression::ExpressionNode;
    use psi_syntax_trees::statement::StatementNode;
    for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
        let base_dir = parsed_source
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        for root_item in &parsed_source.root_items {
            let Item::Machine(machine) = syntax_trees.root_item(*root_item) else {
                continue;
            };
            if machine.name.as_str() != "build" || machine.attached_data.is_some() {
                continue;
            }
            for state_handle in syntax_trees.items.state_handles(machine.states) {
                let state = syntax_trees.items.state(*state_handle);
                let Some(build_param) = syntax_trees
                    .items
                    .state_parameters(state.parameters)
                    .first()
                    .map(|handle| syntax_trees.items.state_parameter(*handle).name.clone())
                else {
                    continue;
                };
                for statement_handle in syntax_trees.items.statements(state.statements) {
                    let StatementNode::Call(call) =
                        syntax_trees.statements.statement(*statement_handle)
                    else {
                        continue;
                    };
                    if call.target.as_str() != "depend" {
                        continue;
                    }
                    let receiver_is_build_param = syntax_trees
                        .statements
                        .identifier_path_members(call.receiver)
                        .last()
                        .is_some_and(|name| name.as_str() == build_param.as_str());
                    if !receiver_is_build_param {
                        continue;
                    }
                    let arguments = syntax_trees.statements.expression_handles(call.arguments);
                    let [alias, location] = arguments else {
                        continue;
                    };
                    let ExpressionNode::String(alias) = syntax_trees.expressions.expression(*alias)
                    else {
                        continue;
                    };
                    // The location is `path("dir")` (the committed spelling)
                    // or a bare string.
                    let location = match syntax_trees.expressions.expression(*location) {
                        ExpressionNode::String(location) => Some(location.clone()),
                        ExpressionNode::Call(path_call) if path_call.target.as_str() == "path" => {
                            syntax_trees
                                .expressions
                                .expression_handles(path_call.arguments)
                                .first()
                                .and_then(|argument| {
                                    match syntax_trees.expressions.expression(*argument) {
                                        ExpressionNode::String(location) => Some(location.clone()),
                                        _ => None,
                                    }
                                })
                        }
                        _ => None,
                    };
                    let Some(location) = location else {
                        continue;
                    };
                    let directory = base_dir.join(location.as_str());
                    let alias = alias.as_str().to_string();
                    if !depend_aliases
                        .iter()
                        .any(|(existing, _)| existing == &alias)
                    {
                        depend_aliases.push((alias, directory));
                    }
                }
            }
        }
    }
}

pub fn discover_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    root_path: &Path,
    selected_target_name: Option<&str>,
    depend_aliases: &mut Vec<(String, PathBuf)>,
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
                    let members = syntax_trees.items.identifier_path_members(use_item.path);
                    // depend-mapping: a use whose FIRST segment is a declared
                    // alias resolves into the aliased directory (and that
                    // package's build.omg loads with it, so its own depends
                    // chain transitively).
                    let aliased = members.first().and_then(|first| {
                        depend_aliases
                            .iter()
                            .find(|(alias, _)| alias == first.as_str())
                            .map(|(_, directory)| directory.clone())
                    });
                    if let Some(directory) = aliased {
                        let mut path = directory.clone();
                        for segment in &members[1..] {
                            path.push(segment.as_str());
                        }
                        for candidate in source_path_candidates(&path) {
                            if candidate.exists() {
                                imports.push(normalize_path(&candidate)?);
                                break;
                            }
                        }
                        // The depended package's build.omg is read AS DATA
                        // (each package declares its own `machine build`;
                        // loading two into one program would collide) --
                        // parse it out-of-band purely for its depend rows,
                        // so chains resolve transitively.
                        collect_package_build_aliases(&directory, depend_aliases);
                        continue;
                    }
                    imports.push(normalize_path(&resolve_source_path(&root_dir, members))?);
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
            TokenText::OwnedBytes(value) => TokenText::owned_bytes(value.clone()),
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

pub(crate) fn bundled_omega_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../omega")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../omega"))
}

/// Read a bundled std module's source text (`omega/language/std/<module>.omg`).
/// Target-specific provider substitution uses this to inject bundled provider
/// modules, such as `macos_gui`, that application source does not import itself.
pub(crate) fn read_bundled_std_source(module: &str) -> Result<String, Vec<Diagnostic>> {
    let mut path = bundled_omega_root();
    path.push("language");
    path.push("std");
    path.push(format!("{module}.omg"));
    std::fs::read_to_string(&path).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to read bundled std module {}: {error}",
            path.display()
        ))]
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
