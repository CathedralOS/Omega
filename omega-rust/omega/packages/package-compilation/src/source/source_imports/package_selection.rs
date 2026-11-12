// Reconciled requester-local dependency alias resolution (`reconciled_package_import`)
// and cross-scope / missing-edge diagnostics.
//
// Resolves `use` paths in package-aware compilations without crossing `Build` and
// `Product` dependency scopes:
// - Only the root package carries `Build`-purpose edges (`source_import_scope`);
//   every dependency package's own files resolve through its `Product`-purpose
//   edges even when loaded into the host `Build` context.
// - If an import names an alias declared only in the opposite scope (`other_purpose`),
//   we reject immediately and name the `Build::dependency` / `Build::build_dependency`
//   registration needed to authorize it rather than falling back to a requester-local path.
// - If an import has no requester-local source and its first segment spells a known
//   package in the closure or bundled library roster, we retain `MissingDependencyEdge`
//   diagnostic metadata so exact-target resolution reports the missing dependency
//   declaration when neither generated nor physical source exists.

use super::paths::{identifier_path_text, is_bundled_core_path, is_bundled_omega_path};
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use source::DependencyScope;
use std::path::{Path, PathBuf};
use syntax_trees::identifier::Identifier;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReconciledPackageImport {
    Toolchain {
        root: PathBuf,
        logical_path: Vec<String>,
    },
    ToolchainContract {
        root: PathBuf,
        logical_path: Vec<String>,
    },
    Package(ReconciledPackageImportRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct ReconciledPackageImportRequest {
    pub(super) package: semantic_vocabulary::PackageKeyIdentity,
    pub(super) expected_root: PathBuf,
    pub(super) relative_path: PathBuf,
    dependency_build_forbidden: bool,
    requesting_source: PathBuf,
    missing_edge: Option<MissingDependencyEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct MissingDependencyEdge {
    authored_path: String,
    alias: String,
    package: String,
    requester: String,
    purpose: DependencyPurpose,
}

impl MissingDependencyEdge {
    fn diagnostic(&self, requesting_source: &Path) -> Diagnostic {
        Diagnostic::error(format!(
            "{}: import `{}` names package {} as `{}`, but package {} declares no {} dependency under that alias -- register it with `Build::{}(builder, description)` in its build.omg, configuring the description with `dependency::with_alias(&mut description, \"{}\")` when the package's own name differs, to add that dependency edge for the {} scope",
            requesting_source.display(),
            self.authored_path,
            self.package,
            self.alias,
            self.requester,
            self.purpose.name(),
            self.purpose.operation().name(),
            self.alias,
            self.purpose.name(),
        ))
    }
}

impl ReconciledPackageImportRequest {
    pub(crate) fn logical_sources(
        &self,
        packages: &PackageCompilationInputs,
        discovery: &mut impl super::LogicalSourceDiscovery,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let logical_path = self
            .relative_path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let excluded = packages
            .packages()
            .filter(|(owner, root)| *owner != self.package && root.starts_with(&self.expected_root))
            .map(|(_, root)| root.to_path_buf())
            .collect();
        let paths = discovery.select_owned(
            &self.expected_root,
            &logical_path,
            packages.source_filesystem(),
            excluded,
        )?;
        if paths.is_empty()
            && let Some(edge) = &self.missing_edge
        {
            return Err(vec![edge.diagnostic(&self.requesting_source)]);
        }
        paths
            .into_iter()
            .filter(|path| packages.package_for_source(path) == Some(self.package))
            .map(|path| {
                self.reject_foreign_source_owner(&path, packages)?;
                self.reject_dependency_build_import(&path)?;
                Ok(path)
            })
            .collect()
    }

    fn reject_foreign_source_owner(
        &self,
        resolved: &Path,
        packages: &PackageCompilationInputs,
    ) -> Result<(), Vec<Diagnostic>> {
        if packages.package_for_source(resolved) != Some(self.package) {
            return Err(vec![Diagnostic::error(format!(
                "package source {} belongs to a different reconciled package than the import requested",
                resolved.display(),
            ))]);
        }
        Ok(())
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

fn other_purpose(purpose: DependencyPurpose) -> DependencyPurpose {
    match purpose {
        DependencyPurpose::Product => DependencyPurpose::Build,
        DependencyPurpose::Build => DependencyPurpose::Product,
    }
}

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
    syntax_trees: &syntax_trees::SyntaxTrees,
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
    if is_bundled_core_path(syntax_trees, members) {
        return Ok(ReconciledPackageImport::Toolchain {
            root: super::bundled_core_root()?,
            logical_path: members[3..]
                .iter()
                .map(|name| name.as_str(syntax_trees).to_owned())
                .collect(),
        });
    }
    if is_bundled_omega_path(syntax_trees, members) {
        return Err(vec![Diagnostic::error(format!(
            "package-aware import `{}` in {} cannot use a bundled library; declare an ordinary package dependency and import it through its requester-local alias",
            identifier_path_text(syntax_trees, members),
            requesting_source.display(),
        ))]);
    }
    let Some(requester) = requester else {
        if let Some(contract_root) = contract_root {
            return Ok(ReconciledPackageImport::ToolchainContract {
                root: contract_root.to_path_buf(),
                logical_path: members
                    .iter()
                    .map(|name| name.as_str(syntax_trees).to_owned())
                    .collect(),
            });
        }
        let core = super::bundled_core_root()?;
        if requesting_source.starts_with(&core) {
            return Ok(ReconciledPackageImport::Toolchain {
                root: core,
                logical_path: members
                    .iter()
                    .map(|name| name.as_str(syntax_trees).to_owned())
                    .collect(),
            });
        }
        return Err(vec![Diagnostic::error(format!(
            "cannot establish the reconciled package identity for import in {}",
            requesting_source.display()
        ))]);
    };
    let (package, source_root, path_members, missing_edge) = match packages
        .dependency_target_for_purpose(requester, purpose, first.as_str(syntax_trees))
    {
        Some(package) => (
            package,
            packages
                .package_root(package)
                .expect("validated dependency target retains a source root"),
            &members[1..],
            None,
        ),
        None if packages.package_name(requester) == Some(first.as_str(syntax_trees)) => (
            requester,
            packages
                .package_root(requester)
                .expect("validated requester retains a source root"),
            &members[1..],
            None,
        ),
        None => {
            let other = other_purpose(purpose);
            if let Some(target) =
                packages.dependency_target_for_purpose(requester, other, first.as_str(syntax_trees))
            {
                let declaration = purpose.operation().name();
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` in {} names {} dependency `{}` (package {}); a {} import may only select {} dependencies -- register it with `Build::{}(builder, description)` under alias `{}` in the root build.omg to select that package for the {} scope",
                    identifier_path_text(syntax_trees, members),
                    requesting_source.display(),
                    other.name(),
                    first.as_str(syntax_trees),
                    packages.package_label(target),
                    purpose.name(),
                    purpose.name(),
                    declaration,
                    first.as_str(syntax_trees),
                    purpose.name(),
                ))]);
            }
            let requester_root = packages
                .package_root(requester)
                .expect("validated requester retains a source root");
            let missing_edge =
                known_package_spelling(packages, requester, first.as_str(syntax_trees)).map(
                    |package| MissingDependencyEdge {
                        authored_path: identifier_path_text(syntax_trees, members),
                        alias: first.as_str(syntax_trees).to_owned(),
                        package,
                        requester: packages.package_label(requester),
                        purpose,
                    },
                );
            (requester, requester_root, members, missing_edge)
        }
    };
    let relative_path = path_members
        .iter()
        .fold(PathBuf::new(), |mut path, member| {
            path.push(member.as_str(syntax_trees));
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
        .map(|(name, _)| format!("`{name}` (bundled)"))
}

fn default_alias_spelling(package_name: &str) -> &str {
    package_name
}

fn bundled_package_roster() -> Vec<(String, PathBuf)> {
    let Some(entries) = super::bundled_omega_root()
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

fn declared_package_name(build_source: &str) -> Option<String> {
    let build_declarations::BuildDeclaration::Package(declaration) =
        build_declarations::project_build_declaration_from_source(build_source).ok()?
    else {
        return None;
    };
    Some(declaration.name.into_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildDeclarationKind, PackageSourceBinding};
    use psi_compiler::{SourceInput, UnmeasuredProduction};
    use source::{CapturedSourceNode, CapturedSourceTree, LoadedSource, SourceId};
    use std::sync::Arc;
    use syntax_trees::{SyntaxTrees, item::Item};

    #[test]
    fn core_local_import_keeps_its_logical_path_and_issued_contract_precedence() {
        let identity = semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let package = PackageSourceBinding::new(
            identity,
            "local_route_test",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("local-route-test"),
        )
        .with_captured_source(Arc::new(
            CapturedSourceTree::new([(
                PathBuf::from("main.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"data Main {}".as_slice()),
                    executable: false,
                },
            )])
            .unwrap(),
        ))
        .unwrap();
        let packages = PackageCompilationInputs::new(
            identity,
            BuildDeclarationKind::Package,
            vec![package],
            Vec::new(),
        )
        .unwrap();
        let mut syntax = SyntaxTrees::default();
        let parsed = SourceInput(
            [LoadedSource {
                source_id: SourceId(0),
                path: PathBuf::from("unit.omg"),
                source: Arc::from("use integer::X;"),
                origin: None,
                scope: DependencyScope::Product,
            }]
            .into_iter()
            .collect(),
        )
        .append(&mut syntax, &mut UnmeasuredProduction)
        .unwrap();
        let unit = &parsed.sources.span_or_empty(parsed.batch)[0];
        let Item::Use(import) = syntax.root_item(unit.root_items[0]) else {
            panic!("expected authored import");
        };
        let members = syntax.items.identifier_path_members(import.path);
        let core = super::super::bundled_core_root().unwrap();
        let path = core.join("nested/unit.omg");
        let relative = reconciled_package_import(
            &syntax,
            &path,
            members,
            None,
            DependencyPurpose::Product,
            None,
            &packages,
        )
        .unwrap();
        assert_eq!(
            relative,
            ReconciledPackageImport::Toolchain {
                root: core.clone(),
                logical_path: vec!["integer".into(), "X".into()]
            }
        );
        let issued = core.join("issued-contract");
        let contract = reconciled_package_import(
            &syntax,
            &path,
            members,
            None,
            DependencyPurpose::Product,
            Some(&issued),
            &packages,
        )
        .unwrap();
        assert_eq!(
            contract,
            ReconciledPackageImport::ToolchainContract {
                root: issued,
                logical_path: vec!["integer".into(), "X".into()]
            }
        );
        assert!(
            reconciled_package_import(
                &syntax,
                Path::new("/unknown/unit.omg"),
                members,
                None,
                DependencyPurpose::Product,
                None,
                &packages
            )
            .is_err()
        );
    }
}
