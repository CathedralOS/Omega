//! Resolve requester-local dependency aliases without crossing build/product scopes.
//! Physical lookup precedes exact-child generated-source collision checking.

use super::paths::{
    bundled_omega_root, identifier_path_text, is_bundled_core_path, is_bundled_omega_path,
    resolve_reconciled_import, resolve_reconciled_relative_import, source_import_candidates,
};
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use source::DependencyScope;
use std::path::{Path, PathBuf};
use syntax_trees::identifier::Identifier;
use tokens::{PunctuationKind, TokenKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciledPackageImport {
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
pub struct ReconciledPackageImportRequest {
    pub(super) package: semantic_vocabulary::PackageKeyIdentity,
    pub(super) expected_root: PathBuf,
    pub(super) relative_path: PathBuf,
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
pub struct MissingDependencyEdge {
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
    pub fn physical_source(&self) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
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

    pub fn resolve_for_exact_target(
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
pub fn source_import_scope(
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

pub fn reconciled_package_import(
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
        if let Some(root) = requester
            .and_then(|requester| packages.package_root(requester))
            .filter(|root| local_source_exists(root, members))
        {
            return Err(vec![Diagnostic::error(format!(
                "import `{}` in {} collides with a requester-local source below {}; `omega::language::core` names are claimed by the bundled toolchain namespace, so the local source is unreachable -- rename or remove it",
                identifier_path_text(members),
                requesting_source.display(),
                root.display(),
            ))]);
        }
        return resolve_reconciled_import(bundled_omega_root()?, &members[2..], "toolchain")
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
pub(super) fn local_source_exists(root: &Path, members: &[Identifier]) -> bool {
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

/// An implicit dependency alias preserves the canonical package name.
fn default_alias_spelling(package_name: &str) -> &str {
    package_name
}

/// The declared names of the bundled library packages: the packages a root
/// could declare through `Source::Path` without acquiring anything. Read for
/// diagnostics only; an undeclared import that spells one of these names is
/// reported as a missing dependency edge, never resolved through it.
fn bundled_package_roster() -> Vec<(String, PathBuf)> {
    let Some(entries) = bundled_omega_root()
        .ok()
        .and_then(|root| std::fs::read_dir(root).ok())
    else {
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

/// The literal of the one direct `Build::package(builder, "kebab_name")` statement a
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
