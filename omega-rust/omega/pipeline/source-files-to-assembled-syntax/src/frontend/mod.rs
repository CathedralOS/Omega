use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::source::{SourceStorage, ToolchainContractCustody};
use arena::{Arena, HandleSpan};
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use source::{DependencyScope, SourceId, SourceOrigin, SourcePosition};
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, ItemHandle};
use tokens::{PunctuationKind, Token, TokenKind, TokenStream, TokenText};

mod import_bindings;
use import_bindings::{ImportOccurrence, direct_source_import};
pub(crate) use import_bindings::{
    PendingPackageImport, ResolvedSourceImport, retain_module_import_bindings,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
    /// The checked instance this load produces. The same path may appear in
    /// one frontier once per scope.
    pub scope: DependencyScope,
}

impl Default for LoadedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            origin: None,
            scope: DependencyScope::Product,
        }
    }
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
    pub scope: DependencyScope,
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
    /// Which dependency context checked this instance. An identical path may
    /// have a second [`ParsedSource`] under the other scope.
    pub scope: DependencyScope,
}

impl Default for ParsedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            origin: None,
            root_items: Vec::new(),
            scope: DependencyScope::Product,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedSources {
    pub sources: Arena<ParsedSource>,
    pub batch: HandleSpan<ParsedSource>,
}

pub fn load_sources(
    frontier: Vec<(PathBuf, DependencyScope)>,
    first_source_id: usize,
) -> Result<LoadedSources, Vec<Diagnostic>> {
    let source_count = frontier.len();
    let mut sources = Arena::with_capacity(source_count);
    let mut loaded = Vec::with_capacity(source_count);

    for (index, (path, scope)) in frontier.into_iter().enumerate() {
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
            scope,
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
    // Injected toolchain vocabulary is compiler-owned, not a dependency-
    // selected instance; it carries product scope and joins a build entry
    // through its explicit source-scoped binding instead.
    let batch = sources.insert_many([LoadedSource {
        source_id: SourceId(first_source_id),
        path: PathBuf::from(name),
        source: Arc::from(text),
        origin: Some(SourceOrigin::Toolchain),
        scope: DependencyScope::Product,
    }]);
    LoadedSources { sources, batch }
}

/// Load one compiler-retained package source with a logical package-relative
/// path and no physical file access. `scope` is the checked instance the
/// generated bundle joins: a bundle embodies its producing package's own
/// context (host context for a build-only package), so generated sources
/// keep their owning package's scope rather than the importer's.
pub fn load_package_generated_source(
    path: PathBuf,
    text: &str,
    source_id: usize,
    scope: DependencyScope,
) -> LoadedSources {
    let mut sources = Arena::with_capacity(1);
    let batch = sources.insert_many([LoadedSource {
        source_id: SourceId(source_id),
        path,
        source: Arc::from(text),
        origin: Some(SourceOrigin::User),
        scope,
    }]);
    LoadedSources { sources, batch }
}

pub fn lex_sources(sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let mut lexed = Vec::with_capacity(source_count);

    for loaded_source in loaded_sources {
        let tokens = source_files_to_tokens::Lexer::new(loaded_source.source.as_ref())
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
            scope: loaded_source.scope,
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
        let root_items = tokens_to_syntax_trees::parse(
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
            scope: lexed_source.scope,
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
pub(crate) fn discover_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    root_path: &Path,
    retained: &mut Vec<ResolvedSourceImport>,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
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
                // Standalone imports inherit the importer's checked
                // instance: a build-scope source's imports are host-context
                // content, never product content by accident of sharing
                // bytes.
                imports.push((resolved, parsed_source.scope));
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
pub(crate) enum ReconciledPackageImport {
    Toolchain(PathBuf),
    /// An import authored inside a closed toolchain contract source. It owns
    /// no package identity, so its path closes inside the registered contract
    /// root rather than selecting reconciled package content.
    ToolchainContract {
        root: PathBuf,
        path: PathBuf,
    },
    Package(ReconciledPackageImportRequest),
}

/// One syntax-derived package import before an exact target contributes its
/// generated-source bundles.
///
/// Physical source lookup is target-independent. Generated-source selection
/// and physical/generated collision rejection remain exact-child work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReconciledPackageImportRequest {
    package: semantic_vocabulary::PackageKeyIdentity,
    expected_root: PathBuf,
    relative_path: PathBuf,
    dependency_build_forbidden: bool,
    requesting_source: PathBuf,
    /// Set when the import fell back to a requester-local path although its
    /// first segment spells a package the requester never declared in this
    /// scope. Reported only if that local fallback finds no source.
    missing_edge: Option<MissingDependencyEdge>,
}

/// A dependency edge an import spelled but its requester never declared: the
/// first path segment is the requester-local alias spelling of a package the
/// compilation already knows, yet no edge in the import's scope binds it.
/// This is diagnostic metadata only. It never adds the edge or resolves the
/// import; the compile still rejects, but by naming the declaration that would
/// close the gap instead of an unresolvable requester-relative source path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingDependencyEdge {
    authored_path: String,
    alias: String,
    package: String,
    requester: String,
    purpose: DependencyPurpose,
}

impl MissingDependencyEdge {
    fn diagnostic(&self, requesting_source: &Path) -> Diagnostic {
        let (declaration, aliased_declaration) = match self.purpose {
            DependencyPurpose::Product => ("depend", "depend_as"),
            DependencyPurpose::Build => ("build_depend", "build_depend_as"),
        };
        Diagnostic::error(format!(
            "import `{}` in {} names package {} as `{}`, but package {} declares no {} dependency under that alias -- declare `builder.{}(...)` or `builder.{}(\"{}\", ...)` in its build.omg to add that dependency edge for the {} scope",
            self.authored_path,
            requesting_source.display(),
            self.package,
            self.alias,
            self.requester,
            self.purpose.name(),
            declaration,
            aliased_declaration,
            self.alias,
            self.purpose.name(),
        ))
    }
}

impl ReconciledPackageImportRequest {
    pub(crate) fn physical_source(&self) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
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

    pub(crate) fn resolve_for_exact_target(
        &self,
        packages: &PackageCompilationInputs,
        scope: DependencyScope,
    ) -> Result<PathBuf, Vec<Diagnostic>> {
        if packages.package_root(self.package) != Some(self.expected_root.as_path()) {
            return Err(vec![Diagnostic::error(
                "exact-target package source root does not match its retained import request",
            )]);
        }
        let relative_candidates = source_import_candidates(&self.relative_path);
        let generated = packages
            .generated_source_import_path(
                self.package,
                match scope {
                    DependencyScope::Product => DependencyPurpose::Product,
                    DependencyScope::Build => DependencyPurpose::Build,
                },
                &relative_candidates,
            )
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
            (None, physical) => {
                // Neither the exact bundle nor the requester's own tree has
                // the source. When the import spelled an undeclared package,
                // the missing edge is the actual gap.
                if physical.is_none()
                    && let Some(missing_edge) = &self.missing_edge
                {
                    return Err(vec![missing_edge.diagnostic(&self.requesting_source)]);
                }
                resolve_reconciled_relative_import(
                    self.expected_root.clone(),
                    &self.relative_path,
                    "package",
                )?
            }
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

/// The other dependency scope. Product and build scopes are independent:
/// neither falls back to the other.
fn other_purpose(purpose: DependencyPurpose) -> DependencyPurpose {
    match purpose {
        DependencyPurpose::Product => DependencyPurpose::Build,
        DependencyPurpose::Build => DependencyPurpose::Product,
    }
}

/// The dependency edge purpose `source_path`'s own imports resolve under.
///
/// Only the compilation root's package carries build-scope edges: a
/// root-owned source's checked instance selects `build_depend`/
/// `build_depend_as` targets when that instance is build scope. Every other
/// package's sources — including build-scope instances of dependency
/// sources — resolve product-scope imports: a build dependency's own files
/// use its ordinary `depend` edges even though they execute in the host
/// context (wiki/spec/build/scoped_execution.md). Which execution profile a
/// package's target-scoped rows select against follows its checked source
/// instance, independently of the edges used to resolve names inside it.
pub(crate) fn source_import_scope(
    packages: &PackageCompilationInputs,
    instance_scope: DependencyScope,
    source_path: &Path,
) -> DependencyPurpose {
    if packages.package_for_source(source_path) == Some(packages.root()) {
        match instance_scope {
            DependencyScope::Product => DependencyPurpose::Product,
            DependencyScope::Build => DependencyPurpose::Build,
        }
    } else {
        DependencyPurpose::Product
    }
}

pub(crate) fn reconciled_package_import(
    requesting_source: &Path,
    members: &[Identifier],
    requester: Option<semantic_vocabulary::PackageKeyIdentity>,
    purpose: DependencyPurpose,
    contract_root: Option<&Path>,
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
        // A toolchain contract source owns no package identity; its authored
        // imports close inside the registered contract root and keep toolchain
        // custody rather than selecting reconciled package content.
        if let Some(contract_root) = contract_root {
            return resolve_reconciled_import(
                contract_root.to_path_buf(),
                members,
                "toolchain contract",
            )
            .map(|path| ReconciledPackageImport::ToolchainContract {
                root: contract_root.to_path_buf(),
                path,
            });
        }
        return Err(vec![Diagnostic::error(format!(
            "cannot establish the reconciled package identity for import in {}",
            requesting_source.display()
        ))]);
    };
    let (package, source_root, path_members, missing_edge) = match packages
        .dependency_target_for_purpose(requester, purpose, first.as_str())
    {
        Some(package) => (
            package,
            packages
                .package_root(package)
                .expect("validated dependency target retains a source root"),
            &members[1..],
            None,
        ),
        None if packages.package_name(requester) == Some(first.as_str()) => (
            requester,
            packages
                .package_root(requester)
                .expect("validated requester retains a source root"),
            &members[1..],
            None,
        ),
        None => {
            // An alias declared only in the other scope is not a missing
            // module below the requester: name the actual authorization gap
            // and the declaration that would close it, so cross-scope
            // spellings never fall back to local paths.
            let other = other_purpose(purpose);
            if let Some(target) =
                packages.dependency_target_for_purpose(requester, other, first.as_str())
            {
                let declaration = match purpose {
                    DependencyPurpose::Product => "depend_as",
                    DependencyPurpose::Build => "build_depend_as",
                };
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` in {} names {} dependency `{}` (package {}); a {} import may only select {} dependencies -- declare `builder.{}(\"{}\", ...)` in the root build.omg to select that package for the {} scope",
                    identifier_path_text(members),
                    requesting_source.display(),
                    other.name(),
                    first.as_str(),
                    packages.package_label(target),
                    purpose.name(),
                    purpose.name(),
                    declaration,
                    first.as_str(),
                    purpose.name(),
                ))]);
            }
            let requester_root = packages
                .package_root(requester)
                .expect("validated requester retains a source root");
            // A requester-local module keeps precedence. Only an import that
            // has no local source and spells a known package name records
            // the edge it would need, so the eventual failure names it.
            let missing_edge = (!local_source_exists(requester_root, members))
                .then(|| known_package_spelling(packages, requester, first.as_str()))
                .flatten()
                .map(|package| MissingDependencyEdge {
                    authored_path: identifier_path_text(members),
                    alias: first.as_str().to_owned(),
                    package,
                    requester: packages.package_label(requester),
                    purpose,
                });
            (requester, requester_root, members, missing_edge)
        }
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
            missing_edge,
        },
    ))
}

/// Whether `members`, read as a requester-local path below `root`, reaches an
/// existing source through any direct or prefix candidate.
fn local_source_exists(root: &Path, members: &[Identifier]) -> bool {
    let relative = members.iter().fold(PathBuf::new(), |mut path, member| {
        path.push(member.as_str());
        path
    });
    source_import_candidates(&relative)
        .into_iter()
        .any(|candidate| root.join(candidate).exists())
}

/// The package `first` spells as a requester-local alias, if the compilation
/// knows one: a package of the reconciled closure other than the requester,
/// or a bundled library package the requester could declare by path. Returns
/// the package's diagnostic label. Matching is by the default alias spelling
/// of the canonical package name; it grants nothing.
fn known_package_spelling(
    packages: &PackageCompilationInputs,
    requester: semantic_vocabulary::PackageKeyIdentity,
    first: &str,
) -> Option<String> {
    let mut closure = packages
        .packages()
        .map(|(identity, _)| identity)
        .filter(|identity| *identity != requester)
        .filter(|identity| {
            packages
                .package_name(*identity)
                .is_some_and(|name| default_alias_spelling(name) == first)
        })
        .collect::<Vec<_>>();
    closure.sort();
    if let Some(identity) = closure.first() {
        return Some(packages.package_label(*identity));
    }
    bundled_package_roster()
        .into_iter()
        .find(|(name, _)| default_alias_spelling(name) == first)
        .map(|(name, root)| format!("`{name}` (bundled at {})", root.display()))
}

/// The alias a dependency edge binds when the declaration names none: the
/// canonical kebab-case package name with `-` written as `_`, the same rule
/// the package manager applies when it reconciles `builder.depend(...)`.
fn default_alias_spelling(package_name: &str) -> String {
    package_name.replace('-', "_")
}

/// The declared names of the bundled library packages: the packages a root
/// could declare through `Source::Path` without acquiring anything. Read for
/// diagnostics only; an undeclared import that spells one of these names is
/// reported as a missing dependency edge, never resolved through it.
fn bundled_package_roster() -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(bundled_omega_root()) else {
        return Vec::new();
    };
    let mut roster = entries
        .flatten()
        .filter_map(|entry| {
            let root = entry.path();
            let build_source = std::fs::read_to_string(root.join("build.omg")).ok()?;
            let name = declared_package_name(&build_source)?;
            Some((name, root))
        })
        .collect::<Vec<_>>();
    roster.sort();
    roster
}

/// The literal of the one direct `builder.package("kebab-name")` statement a
/// package build entry must carry (build-declarations pins that canonical
/// shape). This reads the spelling for a diagnostic; it does not evaluate the
/// build machine or validate the declaration.
fn declared_package_name(build_source: &str) -> Option<String> {
    let tokens = source_files_to_tokens::Lexer::new(build_source)
        .tokenize()
        .ok()?;
    let significant = tokens
        .iter()
        .filter(|token| !token.is_whitespace() && !matches!(token.kind, TokenKind::Comment(_)))
        .collect::<Vec<_>>();
    significant.windows(3).find_map(|window| {
        let [method, open, literal] = window else {
            return None;
        };
        (method.is_identifier()
            && method.lexeme == "package"
            && open.punctuation() == Some(PunctuationKind::LeftParen)
            && literal.is_string_literal())
        .then(|| literal.lexeme.as_str().to_owned())
    })
}

/// Resolve imports exclusively through a reconciled, requester-local package
/// graph. This path never reads or combines dependency rows from `build.omg`.
pub(crate) fn discover_imports_with_packages(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    generated_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
) -> Result<Vec<(PathBuf, DependencyScope)>, Vec<Diagnostic>> {
    let (imports, _) = discover_package_imports(
        parsed,
        syntax_trees,
        packages,
        PackageImportPhase::ExactTarget(generated_owner),
        retained,
        contract_custody,
    )?;
    Ok(imports)
}

pub(crate) enum PackageImportPhase {
    TargetIndependent,
    ExactTarget(Option<semantic_vocabulary::PackageKeyIdentity>),
}

/// Retain package requests until the exact child checks generated-source collisions.
///
/// Each returned path carries the checked instance it joins under: the
/// importer's own scope. A source both scopes import loads twice — the two
/// instances share source bytes and package identity but nothing else.
pub(crate) fn discover_package_imports(
    parsed: &ParsedSources,
    syntax_trees: &SyntaxTrees,
    packages: &PackageCompilationInputs,
    phase: PackageImportPhase,
    retained: &mut Vec<ResolvedSourceImport>,
    contract_custody: &ToolchainContractCustody,
) -> Result<(Vec<(PathBuf, DependencyScope)>, Vec<PendingPackageImport>), Vec<Diagnostic>> {
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
        let purpose = source_import_scope(packages, parsed_source.scope, &parsed_source.path);
        let contract_root = contract_custody.root_for(&parsed_source.path);
        for (ordinal, root_item) in parsed_source.root_items.iter().enumerate() {
            let Item::Use(use_item) = syntax_trees.root_item(*root_item) else {
                continue;
            };
            let members = syntax_trees.items.identifier_path_members(use_item.path);
            match reconciled_package_import(
                &parsed_source.path,
                members,
                requester,
                purpose,
                contract_root,
                packages,
            )? {
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
                    imports.push((path, parsed_source.scope));
                }
                ReconciledPackageImport::ToolchainContract { root, path } => {
                    retained.push(ResolvedSourceImport {
                        occurrence: ImportOccurrence::new(
                            parsed_source.source_id,
                            ordinal,
                            members,
                            0,
                        ),
                        requires_module: !direct_source_import(&root, members, &path),
                        path: path.clone(),
                    });
                    imports.push((path, parsed_source.scope));
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
                        scope: parsed_source.scope,
                        request,
                    };
                    match phase {
                        PackageImportPhase::TargetIndependent => {
                            if let Some(physical) = pending.physical_source()? {
                                imports.push((physical, parsed_source.scope));
                            }
                            requests.push(pending);
                        }
                        PackageImportPhase::ExactTarget(_) => {
                            let resolved = pending.resolve_for_exact_target(packages)?;
                            imports.push((resolved.path.clone(), parsed_source.scope));
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
mod missing_dependency_edge_tests {
    use super::{
        PackageImportPhase, bundled_package_roster, declared_package_name,
        discover_package_imports, lex_sources, load_sources, parse_sources,
    };
    use crate::source::ToolchainContractCustody;
    use diagnostics::Diagnostic;
    use package_compilation::{
        PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
    };
    use std::path::PathBuf;
    use syntax_trees::SyntaxTrees;

    struct Packages {
        directory: PathBuf,
        consumer: PathBuf,
        middle: PathBuf,
        leaf: PathBuf,
    }

    impl Packages {
        fn new() -> Self {
            static NEXT_FIXTURE: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "omega-missing-dependency-edge-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&directory).expect("unique temporary package directory");
            for package in ["consumer", "middle", "leaf"] {
                std::fs::create_dir(directory.join(package)).expect("package directory");
            }
            Self {
                consumer: directory.join("consumer").canonicalize().unwrap(),
                middle: directory.join("middle").canonicalize().unwrap(),
                leaf: directory.join("leaf").canonicalize().unwrap(),
                directory,
            }
        }

        /// The consumer with `middle` declared and `leaf` reached only through
        /// `middle`: `leaf` is in the reconciled closure, but no consumer edge
        /// binds that alias.
        fn inputs(&self, include_leaf: bool) -> PackageCompilationInputs {
            let root = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32])
                .expect("nonzero root identity");
            let middle = semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32])
                .expect("nonzero middle identity");
            let leaf = semantic_vocabulary::PackageKeyIdentity::from_digest([3; 32])
                .expect("nonzero leaf identity");
            let mut packages = vec![PackageSourceBinding::new(
                root,
                "consumer",
                self.consumer.clone(),
            )];
            let mut dependencies = Vec::new();
            if include_leaf {
                packages.push(PackageSourceBinding::new(
                    middle,
                    "middle",
                    self.middle.clone(),
                ));
                packages.push(PackageSourceBinding::new(leaf, "leaf", self.leaf.clone()));
                dependencies.push(PackageDependencyBinding::new(root, "middle", middle));
                dependencies.push(PackageDependencyBinding::new(middle, "leaf", leaf));
            }
            PackageCompilationInputs::new_package(root, packages, dependencies)
                .expect("closed package graph")
        }

        /// Parse one consumer source and reconcile its imports under an exact
        /// target, the phase that reports unresolved package imports.
        fn discover(
            &self,
            source: &str,
            inputs: &PackageCompilationInputs,
        ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
            let main = self.consumer.join("main.omg");
            std::fs::write(&main, source).expect("write consumer source");
            let mut syntax_trees = SyntaxTrees::default();
            let parsed = parse_sources(
                lex_sources(load_sources(
                    vec![(main, source::DependencyScope::Product)],
                    0,
                )?)?,
                &mut syntax_trees,
            )?;
            discover_package_imports(
                &parsed,
                &syntax_trees,
                inputs,
                PackageImportPhase::ExactTarget(None),
                &mut Vec::new(),
                &ToolchainContractCustody::default(),
            )
            .map(|(imports, _)| imports.into_iter().map(|(path, _)| path).collect())
        }
    }

    impl Drop for Packages {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    fn single_message(diagnostics: Vec<Diagnostic>) -> String {
        let [diagnostic] = diagnostics.as_slice() else {
            panic!("expected one diagnostic, found {diagnostics:#?}");
        };
        diagnostic.message.clone()
    }

    #[test]
    fn undeclared_bundled_standard_library_import_names_the_missing_edge() {
        let packages = Packages::new();
        let inputs = packages.inputs(false);
        let message = single_message(
            packages
                .discover("use omega_language_std::console;\n", &inputs)
                .expect_err("an undeclared std import must not resolve"),
        );
        assert!(
            message.contains("import `omega_language_std::console` in")
                && message.contains(&packages.consumer.join("main.omg").display().to_string()),
            "{message}"
        );
        assert!(
            message.contains("names package `omega-language-std` (bundled at")
                && message.contains("as `omega_language_std`"),
            "{message}"
        );
        assert!(
            message.contains("package `consumer`")
                && message.contains("declares no product dependency under that alias"),
            "{message}"
        );
        assert!(
            message.contains(
                "declare `builder.depend(...)` or `builder.depend_as(\"omega_language_std\", ...)` in its build.omg"
            ) && message.ends_with("for the product scope"),
            "{message}"
        );
        assert!(
            !message.contains("failed to resolve"),
            "the missing edge must replace the path report: {message}"
        );
    }

    #[test]
    fn undeclared_closure_package_import_names_the_missing_edge() {
        let packages = Packages::new();
        std::fs::write(packages.leaf.join("first.omg"), "module first;\n")
            .expect("write leaf source");
        let inputs = packages.inputs(true);
        let message = single_message(
            packages
                .discover("use leaf::first;\n", &inputs)
                .expect_err("a closure package without an edge must not resolve"),
        );
        assert!(
            message.contains("import `leaf::first` in")
                && message.contains("names package `leaf` (")
                && message.contains("as `leaf`, but package `consumer`")
                && message.contains("`builder.depend_as(\"leaf\", ...)`"),
            "{message}"
        );
    }

    #[test]
    fn requester_local_module_keeps_precedence_over_a_known_package_spelling() {
        let packages = Packages::new();
        std::fs::write(packages.consumer.join("leaf.omg"), "module leaf;\n")
            .expect("write local module");
        let inputs = packages.inputs(true);
        let imports = packages
            .discover("use leaf;\n", &inputs)
            .expect("a local module resolves before any package spelling");
        assert_eq!(imports, vec![packages.consumer.join("leaf.omg")]);
    }

    #[test]
    fn unknown_first_segment_keeps_the_unresolvable_path_report() {
        let packages = Packages::new();
        let inputs = packages.inputs(true);
        let message = single_message(
            packages
                .discover("use nonexistent::missing;\n", &inputs)
                .expect_err("an unknown local path must not resolve"),
        );
        // The report shows the host path; Windows spells it with backslashes.
        let portable = message.replace('\\', "/");
        assert!(
            message.starts_with("failed to resolve")
                && portable.contains("/consumer/nonexistent/missing.omg")
                && !message.contains("declares no"),
            "{message}"
        );
    }

    #[test]
    fn bundled_roster_reads_the_declared_standard_library_name() {
        let roster = bundled_package_roster();
        let standard_library = roster
            .iter()
            .find(|(name, _)| name == "omega-language-std")
            .expect("the bundled std package declares its name");
        assert_eq!(
            standard_library
                .1
                .file_name()
                .and_then(|name| name.to_str()),
            Some("std")
        );
        assert_eq!(
            declared_package_name(
                "machine build(builder: &mut Build) {\n    // the package\n    builder.package(\n        \"spaced-name\"\n    );\n}\n"
            )
            .as_deref(),
            Some("spaced-name")
        );
        assert_eq!(
            declared_package_name(
                "machine build(builder: &mut Build) { builder.application(\"app\"); }"
            ),
            None
        );
    }
}

#[cfg(test)]
mod token_retention_tests {
    use super::own_token_stream;
    use source::Span;
    use std::sync::Arc;
    use tokens::{Token, TokenKind, TokenStream, TokenText};

    #[test]
    fn retention_moves_decoded_literal_allocations_and_preserves_tokens() {
        let source: Arc<str> = Arc::from(r#"name "line\ntext" "\xFF\0""#);
        let tokens = source_files_to_tokens::Lexer::new(&source)
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
        let tokens = source_files_to_tokens::Lexer::new(&source)
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
