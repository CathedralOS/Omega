use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::source::SourceStorage;
use crate::{lexer, parser};
use arena::{Arena, HandleSpan};
use diagnostics::Diagnostic;
use source::{SourceId, SourceOrigin, SourcePosition};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, ItemHandle};
use tokens::{Token, TokenStream, TokenText};

mod import_bindings;
use import_bindings::{ImportOccurrence, direct_source_import};
pub(super) use import_bindings::{
    PendingPackageImport, ResolvedSourceImport, retain_module_import_bindings,
};

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
            tokens: own_token_stream(tokens, &loaded_source.source),
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
pub(super) fn discover_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    root_path: &Path,
    retained: &mut Vec<ResolvedSourceImport>,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let root_dir = root_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let parsed_sources = parsed.sources.span_or_empty(parsed.batch);
    let mut imports = Vec::with_capacity(parsed_sources.len());

    for parsed_source in parsed_sources {
        let source_root = standalone_source_root(&root_dir, &parsed_source.path);
        for (ordinal, root_item) in parsed_source.root_items.iter().enumerate() {
            if let Item::Use(use_item) = syntax_trees.root_item(*root_item) {
                let members = syntax_trees.items.identifier_path_members(use_item.path);
                let resolved = normalize_path(&resolve_source_path(&source_root, members))?;
                let prefix = if is_bundled_omega_path(members) { 2 } else { 0 };
                let owner = if prefix == 2 {
                    bundled_omega_root()
                } else {
                    source_root.clone()
                };
                retained.push(ResolvedSourceImport {
                    occurrence: ImportOccurrence::new(
                        parsed_source.source_id,
                        ordinal,
                        members,
                        prefix,
                    ),
                    requires_module: !direct_source_import(&owner, &members[prefix..], &resolved),
                    path: resolved.clone(),
                });
                imports.push(resolved);
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
    package: semantic_vocabulary::PackageKeyIdentity,
    expected_root: PathBuf,
    relative_path: PathBuf,
    dependency_build_forbidden: bool,
    requesting_source: PathBuf,
}

impl ReconciledPackageImportRequest {
    pub(super) fn physical_source(&self) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
        if !source_import_candidates(&self.relative_path)
            .into_iter()
            .any(|candidate| self.expected_root.join(candidate).exists())
        {
            return Ok(None);
        }
        let resolved = resolve_reconciled_relative_import(
            self.expected_root.clone(),
            &self.relative_path,
            "package",
        )?;
        self.reject_dependency_build_import(&resolved)?;
        Ok(Some(resolved))
    }

    pub(super) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<PathBuf, Vec<Diagnostic>> {
        if packages.package_root(self.package) != Some(self.expected_root.as_path()) {
            return Err(vec![Diagnostic::error(
                "exact-target package source root does not match its retained import request",
            )]);
        }
        let relative_candidates = source_import_candidates(&self.relative_path);
        let generated = packages
            .generated_source_import_path(self.package, &relative_candidates)
            .map_err(|error| vec![Diagnostic::error(error)])?;
        let physical = relative_candidates
            .into_iter()
            .map(|candidate| self.expected_root.join(candidate))
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
    requester: Option<semantic_vocabulary::PackageKeyIdentity>,
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
pub(super) fn discover_imports_with_packages(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    generated_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    retained: &mut Vec<ResolvedSourceImport>,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let (imports, _) = discover_package_imports(
        parsed,
        syntax_trees,
        packages,
        PackageImportPhase::ExactTarget(generated_owner),
        retained,
    )?;
    Ok(imports)
}

pub(super) enum PackageImportPhase {
    TargetIndependent,
    ExactTarget(Option<semantic_vocabulary::PackageKeyIdentity>),
}

/// Retain package requests until the exact child checks generated-source collisions.
pub(super) fn discover_package_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    phase: PackageImportPhase,
    retained: &mut Vec<ResolvedSourceImport>,
) -> Result<(Vec<PathBuf>, Vec<PendingPackageImport>), Vec<Diagnostic>> {
    let generated_owner = match phase {
        PackageImportPhase::TargetIndependent => None,
        PackageImportPhase::ExactTarget(owner) => owner,
    };
    let mut imports = Vec::new();
    let mut requests = Vec::new();
    for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
        // Physical loaders normalize paths. Generated producers already own the
        // package association; their virtual paths need no filesystem lookup.
        let requester =
            generated_owner.or_else(|| packages.package_for_source(&parsed_source.path));
        for (ordinal, root_item) in parsed_source.root_items.iter().enumerate() {
            let Item::Use(use_item) = syntax_trees.root_item(*root_item) else {
                continue;
            };
            let members = syntax_trees.items.identifier_path_members(use_item.path);
            match reconciled_package_import(&parsed_source.path, members, requester, packages)? {
                ReconciledPackageImport::Toolchain(path) => {
                    retained.push(ResolvedSourceImport {
                        occurrence: ImportOccurrence::new(
                            parsed_source.source_id,
                            ordinal,
                            members,
                            2,
                        ),
                        requires_module: !direct_source_import(
                            &bundled_omega_root(),
                            &members[2..],
                            &path,
                        ),
                        path: path.clone(),
                    });
                    imports.push(path);
                }
                ReconciledPackageImport::Package(request) => {
                    let prefix = members.len() - request.relative_path.components().count();
                    let pending = PendingPackageImport {
                        occurrence: ImportOccurrence::new(
                            parsed_source.source_id,
                            ordinal,
                            members,
                            prefix,
                        ),
                        request,
                    };
                    match phase {
                        PackageImportPhase::TargetIndependent => {
                            if let Some(physical) = pending.physical_source()? {
                                imports.push(physical);
                            }
                            requests.push(pending);
                        }
                        PackageImportPhase::ExactTarget(_) => {
                            let resolved = pending.resolve_for_exact_target(packages)?;
                            imports.push(resolved.path.clone());
                            retained.push(resolved);
                        }
                    }
                }
            }
        }
    }
    Ok((imports, requests))
}

pub fn extend_source_storage(
    source_storage: &mut SourceStorage,
    parsed: ParsedSources,
) -> Result<(), Vec<Diagnostic>> {
    source_storage.extend(parsed)
}

/// Retain this source's lexer output beyond its borrow, moving decoded literal
/// buffers. Individual shared lexemes also supply source ownership to the
/// parser's independently retained identifiers and source text.
fn own_token_stream(tokens: TokenStream<'_>, source: &Arc<str>) -> TokenStream<'static> {
    TokenStream::new(
        tokens
            .into_tokens()
            .into_iter()
            .map(|token| {
                let lexeme = match token.lexeme {
                    TokenText::Source(_) => TokenText::shared(source.clone(), token.span),
                    TokenText::Shared { source, span } => TokenText::Shared { source, span },
                    TokenText::Owned(value) => TokenText::Owned(value),
                    TokenText::OwnedBytes(value) => TokenText::OwnedBytes(value),
                };
                Token {
                    kind: token.kind,
                    lexeme,
                    span: token.span,
                }
            })
            .collect(),
    )
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

    let relative_path = segments.fold(PathBuf::new(), |mut relative_path, segment| {
        relative_path.push(segment.as_str());
        relative_path
    });
    for relative_candidate in source_import_candidates(&relative_path) {
        let candidate = path.join(relative_candidate);
        if candidate.exists() {
            return candidate;
        }
    }
    path.push(relative_path);
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

    let candidate = source_import_candidates(relative_path)
        .into_iter()
        .map(|candidate| expected_root.join(candidate))
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
        .join("../../../../source/library")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../source/library")
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

/// Route a declaration import through its longest existing source prefix.
/// The parsed module and declaration must still establish its nominal identity.
fn source_import_candidates(relative_path: &Path) -> Vec<PathBuf> {
    let mut candidates = source_path_candidates(relative_path);
    let mut prefix = relative_path.parent();
    while let Some(path) = prefix.filter(|path| !path.as_os_str().is_empty()) {
        candidates.extend(source_path_candidates(path));
        prefix = path.parent();
    }
    candidates
}

#[cfg(test)]
mod token_retention_tests {
    use super::own_token_stream;
    use crate::lexer;
    use source::Span;
    use std::sync::Arc;
    use tokens::{Token, TokenKind, TokenStream, TokenText};

    #[test]
    fn retention_moves_decoded_literal_allocations_and_preserves_tokens() {
        let source: Arc<str> = Arc::from(r#"name "line\ntext" "\xFF\0""#);
        let tokens = lexer::Lexer::new(&source)
            .tokenize()
            .expect("lex literal fixture");
        let expected = tokens
            .iter()
            .map(|token| (token.kind, token.span, token.lexeme.as_bytes().to_vec()))
            .collect::<Vec<_>>();
        let decoded_allocations = tokens
            .iter()
            .filter(|token| token.is_string_literal())
            .map(|token| token.lexeme.as_bytes().as_ptr())
            .collect::<Vec<_>>();
        assert_eq!(decoded_allocations.len(), 2);
        let owned = own_token_stream(tokens, &source);
        let actual = owned
            .iter()
            .map(|token| (token.kind, token.span, token.lexeme.as_bytes().to_vec()))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        let literals = owned
            .iter()
            .filter(|token| token.is_string_literal())
            .collect::<Vec<_>>();
        assert!(matches!(literals[0].lexeme, TokenText::Owned(_)));
        assert!(matches!(literals[1].lexeme, TokenText::OwnedBytes(_)));
        assert_eq!(literals[0].lexeme.as_bytes(), b"line\ntext");
        assert_eq!(literals[1].lexeme.as_bytes(), &[255, 0]);
        for (literal, allocation) in literals.iter().zip(decoded_allocations) {
            assert_eq!(literal.lexeme.as_bytes().as_ptr(), allocation);
        }
    }

    #[test]
    fn retained_token_keeps_source_alive_after_stream_and_owner_drop() {
        let source: Arc<str> = Arc::from("name");
        let weak_source = Arc::downgrade(&source);
        let tokens = lexer::Lexer::new(&source)
            .tokenize()
            .expect("lex identifier");
        let owned = own_token_stream(tokens, &source);
        let retained = owned[0].clone();
        assert_eq!(retained.lexeme.as_bytes().as_ptr(), source.as_ptr());
        drop(owned);
        drop(source);
        assert!(weak_source.upgrade().is_some());
        assert_eq!(retained.lexeme, "name");
        drop(retained);
        assert!(weak_source.upgrade().is_none());
    }

    #[test]
    fn retention_preserves_existing_shared_owner_and_owned_byte_storage() {
        let source: Arc<str> = Arc::from("different source");
        let existing: Arc<str> = Arc::from("shared text");
        let shared_pointer = existing.as_ptr();
        let weak_existing = Arc::downgrade(&existing);
        let bytes = b"valid utf8 bytes".to_vec();
        let bytes_pointer = bytes.as_ptr();
        let tokens = TokenStream::new(vec![
            Token {
                kind: TokenKind::Identifier,
                lexeme: TokenText::Shared {
                    source: existing,
                    span: Span::new(0, 6),
                },
                span: Span::new(0, 6),
            },
            Token {
                kind: TokenKind::StringLiteral,
                lexeme: TokenText::OwnedBytes(bytes),
                span: Span::new(7, 10),
            },
        ]);
        let owned = own_token_stream(tokens, &source);
        assert_eq!(owned[0].lexeme.as_bytes().as_ptr(), shared_pointer);
        assert_eq!(weak_existing.strong_count(), 1);
        assert_eq!(owned[1].lexeme.as_bytes().as_ptr(), bytes_pointer);
        assert!(matches!(owned[1].lexeme, TokenText::OwnedBytes(_)));
        drop(owned);
        assert!(weak_existing.upgrade().is_none());
    }
}
