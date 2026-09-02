use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::source::SourceStorage;
use crate::{lexer, parser};
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::Diagnostic;
use psi_source::{SourceId, SourceOrigin, SourcePosition};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::item::{Item, ItemHandle};
use psi_tokens::{Token, TokenStream, TokenText};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
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
    pub origin: Option<SourceOrigin>,
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
    pub origin: Option<SourceOrigin>,
    pub root_items: Vec<ItemHandle>,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            origin: None,
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
            origin: None,
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
        origin: Some(SourceOrigin::Toolchain),
    }]);
    LoadedSources { sources, batch }
}

/// Load one compiler-retained package source with a logical package-relative
/// path and no physical file access.
pub fn load_package_generated_source(path: PathBuf, text: &str, source_id: usize) -> LoadedSources {
    let mut sources = Arena::with_capacity(1);
    let batch = sources.insert_many([LoadedSource {
        source_id: SourceId(source_id),
        path,
        source: Arc::from(text),
        origin: Some(SourceOrigin::User),
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
            origin: loaded_source.origin,
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
            origin: lexed_source.origin,
            root_items,
        });
    }

    let batch = parsed_sources.insert_many(parsed);

    Ok(ParsedSources {
        sources: parsed_sources,
        batch,
    })
}

/// Discover standalone imports without interpreting dependency declarations.
/// Package aliases are meaningful only on the reconciled package-aware path.
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
        let source_root = standalone_source_root(&root_dir, &parsed_source.path);
        for root_item in &parsed_source.root_items {
            let item = syntax_trees.root_item(*root_item);
            match item {
                Item::Use(use_item) => {
                    let members = syntax_trees.items.identifier_path_members(use_item.path);
                    imports.push(normalize_path(&resolve_source_path(&source_root, members))?);
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

/// Preserve standalone compilation of the bundled std sources while their
/// authored self-imports use ordinary package-local paths. This is standalone
/// toolchain routing only; package-aware compilation resolves the same paths
/// through exact package custody.
fn standalone_source_root(default_root: &Path, source: &Path) -> PathBuf {
    let standard_library_root = bundled_omega_root().join("std");
    source
        .canonicalize()
        .ok()
        .filter(|source| source.starts_with(&standard_library_root))
        .map(|_| standard_library_root)
        .unwrap_or_else(|| default_root.to_path_buf())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReconciledPackageImport {
    Toolchain(PathBuf),
    Package(ReconciledPackageImportRequest),
}

/// One syntax-derived package import before an exact target contributes its
/// generated-source bundles.
///
/// Physical source lookup is target-independent. Generated-source selection
/// and physical/generated collision rejection remain exact-child work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReconciledPackageImportRequest {
    package: psi_core::PackageKeyIdentity,
    expected_root: PathBuf,
    relative_path: PathBuf,
    dependency_build_forbidden: bool,
    requesting_source: PathBuf,
}

impl ReconciledPackageImportRequest {
    pub(super) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<PathBuf, Vec<Diagnostic>> {
        if packages.package_root(self.package) != Some(self.expected_root.as_path()) {
            return Err(vec![Diagnostic::error(
                "exact-target package source root does not match its retained import request",
            )]);
        }
        let relative_candidates = source_path_candidates(&self.relative_path);
        let generated = packages
            .generated_source_import_path(self.package, &relative_candidates)
            .map_err(|error| vec![Diagnostic::error(error)])?;
        let physical = source_path_candidates(&self.expected_root.join(&self.relative_path))
            .into_iter()
            .find(|candidate| candidate.exists());
        let resolved = match (generated, physical) {
            (Some(generated), Some(physical)) => {
                return Err(vec![Diagnostic::error(format!(
                    "generated package source {} collides with physical package source {}",
                    generated.display(),
                    physical.display(),
                ))]);
            }
            (Some(generated), None) => generated,
            (None, _) => resolve_reconciled_relative_import(
                self.expected_root.clone(),
                &self.relative_path,
                "package",
            )?,
        };
        self.reject_dependency_build_import(&resolved)?;
        Ok(resolved)
    }

    fn reject_dependency_build_import(&self, resolved: &Path) -> Result<(), Vec<Diagnostic>> {
        if self.dependency_build_forbidden
            && resolved.file_name().and_then(|name| name.to_str()) == Some("build.omg")
        {
            return Err(vec![Diagnostic::error(format!(
                "package import in {} may not load dependency build file {}",
                self.requesting_source.display(),
                resolved.display(),
            ))]);
        }
        Ok(())
    }
}

pub(super) fn reconciled_package_import(
    requesting_source: &Path,
    members: &[Identifier],
    requester: Option<psi_core::PackageKeyIdentity>,
    packages: &PackageCompilationInputs,
) -> Result<ReconciledPackageImport, Vec<Diagnostic>> {
    let Some(first) = members.first() else {
        return Err(vec![Diagnostic::error(format!(
            "{} contains an empty import path",
            requesting_source.display()
        ))]);
    };
    if is_bundled_core_path(members) {
        return resolve_reconciled_import(bundled_omega_root(), &members[2..], "toolchain")
            .map(ReconciledPackageImport::Toolchain);
    }
    if is_bundled_omega_path(members) {
        return Err(vec![Diagnostic::error(format!(
            "package-aware import `{}` in {} cannot use a bundled library; declare an ordinary package dependency and import it through its requester-local alias",
            identifier_path_text(members),
            requesting_source.display(),
        ))]);
    }
    let Some(requester) = requester else {
        return Err(vec![Diagnostic::error(format!(
            "cannot establish the reconciled package identity for import in {}",
            requesting_source.display()
        ))]);
    };
    let (package, source_root, path_members) =
        match packages.dependency_target(requester, first.as_str()) {
            Some(package) => (
                package,
                packages
                    .package_root(package)
                    .expect("validated dependency target retains a source root"),
                &members[1..],
            ),
            None if packages.package_name(requester) == Some(first.as_str()) => (
                requester,
                packages
                    .package_root(requester)
                    .expect("validated requester retains a source root"),
                &members[1..],
            ),
            None => (
                requester,
                packages
                    .package_root(requester)
                    .expect("validated requester retains a source root"),
                members,
            ),
        };
    let relative_path = path_members
        .iter()
        .fold(PathBuf::new(), |mut path, member| {
            path.push(member.as_str());
            path
        });
    Ok(ReconciledPackageImport::Package(
        ReconciledPackageImportRequest {
            package,
            expected_root: source_root.to_path_buf(),
            relative_path,
            dependency_build_forbidden: package != requester,
            requesting_source: requesting_source.to_path_buf(),
        },
    ))
}

/// Resolve imports exclusively through a reconciled, requester-local package
/// graph. This path never reads or combines dependency rows from `build.omg`.
pub fn discover_imports_with_packages(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    selected_target_name: Option<&str>,
    packages: &PackageCompilationInputs,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let parsed_sources = parsed.sources.span_or_empty(parsed.batch);
    let mut imports = Vec::with_capacity(parsed_sources.len());

    for parsed_source in parsed_sources {
        let canonical_source = parsed_source.path.canonicalize().ok();
        let requester = canonical_source
            .as_deref()
            .and_then(|source| packages.package_for_source(source))
            .or_else(|| {
                packages
                    .is_generated_source_logical_path(&parsed_source.path)
                    .then(|| packages.package_for_source(&parsed_source.path))
                    .flatten()
            });

        for root_item in &parsed_source.root_items {
            let item = syntax_trees.root_item(*root_item);
            match item {
                Item::Use(use_item) => {
                    let members = syntax_trees.items.identifier_path_members(use_item.path);
                    match reconciled_package_import(
                        &parsed_source.path,
                        members,
                        requester,
                        packages,
                    )? {
                        ReconciledPackageImport::Toolchain(imported) => imports.push(imported),
                        ReconciledPackageImport::Package(request) => {
                            imports.push(request.resolve_for_exact_target(packages)?)
                        }
                    }
                }
                Item::Target(target) => {
                    let target_is_selected = selected_target_name
                        .is_none_or(|target_name| target.name.as_str() == target_name);
                    if !target_is_selected {
                        continue;
                    }

                    if let Some(host) = &target.host {
                        let provider = syntax_trees.items.identifier_path_members(host.provider);
                        if is_bundled_core_path(provider) {
                            imports.push(resolve_reconciled_import(
                                bundled_omega_root(),
                                &provider[2..],
                                "toolchain",
                            )?);
                        } else if is_bundled_omega_path(provider) {
                            return Err(vec![Diagnostic::error(format!(
                                "package-aware target host `{}` in {} cannot use a bundled library; declare an ordinary package dependency and name its requester-local alias",
                                identifier_path_text(provider),
                                parsed_source.path.display(),
                            ))]);
                        }
                    }

                    for boundary_policy in syntax_trees
                        .items
                        .boundary_policies(target.boundary_policies)
                    {
                        let policy = syntax_trees
                            .items
                            .identifier_path_members(boundary_policy.path);
                        if is_bundled_core_path(policy) {
                            imports.push(resolve_reconciled_import(
                                bundled_omega_root(),
                                &policy[2..],
                                "toolchain",
                            )?);
                        } else if is_bundled_omega_path(policy) {
                            return Err(vec![Diagnostic::error(format!(
                                "package-aware boundary policy `{}` in {} cannot use a bundled library; declare an ordinary package dependency and name its requester-local alias",
                                identifier_path_text(policy),
                                parsed_source.path.display(),
                            ))]);
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

fn resolve_reconciled_import(
    expected_root: PathBuf,
    source_path: &[Identifier],
    source_kind: &str,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let mut relative_path = PathBuf::new();
    for segment in source_path {
        relative_path.push(segment.as_str());
    }

    resolve_reconciled_relative_import(expected_root, &relative_path, source_kind)
}

fn resolve_reconciled_relative_import(
    expected_root: PathBuf,
    relative_path: &Path,
    source_kind: &str,
) -> Result<PathBuf, Vec<Diagnostic>> {
    let path = expected_root.join(relative_path);

    let candidate = source_path_candidates(&path)
        .into_iter()
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| {
            source_path_candidates(&path)
                .into_iter()
                .next()
                .unwrap_or(path)
        });
    let canonical = normalize_path(&candidate)?;
    if !canonical.starts_with(&expected_root) {
        return Err(vec![Diagnostic::error(format!(
            "resolved {source_kind} import {} escapes expected source root {}",
            canonical.display(),
            expected_root.display()
        ))]);
    }
    Ok(canonical)
}

fn is_bundled_omega_path(path: &[Identifier]) -> bool {
    path.first()
        .is_some_and(|segment| segment.as_str() == "omega")
        && path
            .get(1)
            .is_some_and(|segment| segment.as_str() == "language")
}

fn is_bundled_core_path(path: &[Identifier]) -> bool {
    is_bundled_omega_path(path)
        && path
            .get(2)
            .is_some_and(|segment| segment.as_str() == "core")
}

fn identifier_path_text(path: &[Identifier]) -> String {
    path.iter()
        .map(Identifier::as_str)
        .collect::<Vec<_>>()
        .join("::")
}

pub(crate) fn bundled_omega_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../source/library")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../source/library")
        })
}

pub(crate) fn bundled_core_root() -> PathBuf {
    bundled_omega_root().join("core")
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
