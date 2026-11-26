// Reconciled requester-local dependency alias resolution (`reconciled_package_import`)
// and cross-scope / missing-edge diagnostics.
//
// Resolves `use` paths in package-aware compilations without crossing `Build` and
// `Product` dependency scopes:
// - Only the root package carries `Build`-purpose edges (`source_import_scope`);
//   every dependency package's own files resolve through its `Product`-purpose
//   edges even when loaded into the host `Build` context.
// - If an import names an alias declared only in the opposite scope (`other_purpose`),
//   report who can supply it for the requesting program.
// - If an import has no requester-local source and its first segment spells a known
//   package in the captured closure, we retain `MissingDependencyEdge`
//   diagnostic metadata so exact-target resolution reports the missing dependency
//   declaration when neither generated nor physical source exists.

use super::paths::{identifier_path_text, is_bundled_core_path, is_bundled_omega_path};
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use source_provenance::DependencyScope;
use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(crate) struct ReconciledPackageImportRequest {
    pub(super) package: semantic_vocabulary::PackageKeyIdentity,
    pub(super) expected_root: PathBuf,
    pub(super) relative_path: PathBuf,
    dependency_build_forbidden: bool,
    requesting_source: PathBuf,
    missing_edge: Option<MissingDependencyEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(crate) struct MissingDependencyEdge {
    authored_path: String,
    alias: String,
    package: String,
    requester: String,
    purpose: DependencyPurpose,
}

fn binding_advice(purpose: DependencyPurpose, alias: &str) -> String {
    match purpose {
        DependencyPurpose::Build => {
            format!("The invoker must supply `{alias}` as a Build library before the Build starts.")
        }
        DependencyPurpose::Product => format!(
            "Register the Product dependency with `Build::dependency(builder, description)` in the requesting package's build.omg. If the package name differs from `{alias}`, import `use {{omega}}/build/dependency.omg;` and set the alias with `|dependency|::with_alias(@mutable description, \"{alias}\")` before registration."
        ),
    }
}

impl MissingDependencyEdge {
    fn diagnostic(&self, requesting_source: &Path) -> Diagnostic {
        Diagnostic::error(format!(
            "{}: import `{}` names package {} as `{}`, but package {} has no {} dependency under that alias. {}",
            requesting_source.display(),
            self.authored_path,
            self.package,
            self.alias,
            self.requester,
            self.purpose.name(),
            binding_advice(self.purpose, &self.alias),
        ))
    }
}

impl ReconciledPackageImportRequest {
    pub(super) fn file(
        package: semantic_vocabulary::PackageKeyIdentity,
        root: &Path,
        relative: &str,
        requester: &Path,
        foreign: bool,
    ) -> Self {
        Self {
            package,
            expected_root: root.to_owned(),
            relative_path: relative.into(),
            dependency_build_forbidden: foreign,
            requesting_source: requester.to_owned(),
            missing_edge: None,
        }
    }

    pub(super) fn exact_file(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
        let selected = self.expected_root.join(&self.relative_path);
        self.reject_dependency_build_import(&selected)?;
        self.reject_foreign_source_owner(&selected, packages)?;
        let resolved = super::files::captured_file(
            &self.expected_root,
            &self.relative_path,
            packages.source_filesystem(),
        )?;
        if let Some(path) = &resolved
            && path != &selected
        {
            self.reject_foreign_source_owner(path, packages)?;
            self.reject_dependency_build_import(path)?;
        }
        Ok(resolved)
    }

    /// Leave an unresolved Build name with its source occurrence so admission can
    /// report it at the import. Known foreign packages report their missing edge here.
    pub(super) fn is_unbound_build_import(&self, importer: DependencyScope) -> bool {
        importer == DependencyScope::Build
            && !self.dependency_build_forbidden
            && self.missing_edge.is_none()
    }

    pub(crate) fn logical_sources(
        &self,
        packages: &PackageCompilationInputs,
        discovery: &mut impl super::LogicalSourceDiscovery,
        importer: DependencyScope,
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
        let own_build_import =
            importer == DependencyScope::Build && !self.dependency_build_forbidden;
        let paths = if own_build_import {
            discovery.select_declaring(
                &self.expected_root,
                &logical_path,
                packages.source_filesystem(),
                excluded,
            )?
        } else {
            discovery.select_owned(
                &self.expected_root,
                &logical_path,
                packages.source_filesystem(),
                excluded,
            )?
        };
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

pub(super) fn other_purpose(purpose: DependencyPurpose) -> DependencyPurpose {
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
    let requested = match instance_scope {
        DependencyScope::Product => DependencyPurpose::Product,
        DependencyScope::Build => DependencyPurpose::Build,
    };
    packages
        .package_for_source(source_path)
        .map_or(DependencyPurpose::Product, |requester| {
            packages.declaration_selection_purpose(requester, requested)
        })
}

pub(crate) fn reconciled_package_import(
    requesting_source: &Path,
    members: &[String],
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
        return Ok(ReconciledPackageImport::Toolchain {
            root: super::bundled_core_root()?,
            logical_path: members[3..]
                .iter()
                .map(|name| name.as_str().to_owned())
                .collect(),
        });
    }
    if is_bundled_omega_path(members) {
        return Err(vec![Diagnostic::error(format!(
            "package-aware import `{}` in {} cannot use a bundled library; declare an ordinary package dependency and import it through its requester-local alias",
            identifier_path_text(members),
            requesting_source.display(),
        ))]);
    }
    let Some(requester) = requester else {
        if let Some(contract_root) = contract_root {
            let prefix = super::paths::contract_package_prefix(contract_root, first)?;
            return Ok(ReconciledPackageImport::ToolchainContract {
                root: contract_root.to_path_buf(),
                logical_path: members[prefix..]
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect(),
            });
        }
        let core = super::bundled_core_root()?;
        if requesting_source.starts_with(&core) {
            return Ok(ReconciledPackageImport::Toolchain {
                root: core,
                logical_path: members
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect(),
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
            let other = other_purpose(purpose);
            if let Some(target) =
                packages.dependency_target_for_purpose(requester, other, first.as_str())
            {
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` in {} names {} dependency `{}` (package {}), but a {} import may only select {} dependencies. {}",
                    identifier_path_text(members),
                    requesting_source.display(),
                    other.name(),
                    first.as_str(),
                    packages.package_label(target),
                    purpose.name(),
                    purpose.name(),
                    binding_advice(purpose, first),
                ))]);
            }
            let requester_root = packages
                .package_root(requester)
                .expect("validated requester retains a source root");
            let missing_edge =
                known_package_spelling(packages, requester, first.as_str()).map(|package| {
                    MissingDependencyEdge {
                        authored_path: identifier_path_text(members),
                        alias: first.as_str().to_owned(),
                        package,
                        requester: packages.package_label(requester),
                        purpose,
                    }
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

fn known_package_spelling(
    packages: &PackageCompilationInputs,
    requester: semantic_vocabulary::PackageKeyIdentity,
    first: &str,
) -> Option<String> {
    packages
        .packages()
        .map(|(identity, _)| identity)
        .filter(|identity| *identity != requester && packages.package_name(*identity) == Some(first))
        .min()
        .map(|identity| packages.package_label(identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildDeclarationKind, PackageSourceBinding};
    use source_admission::LoadedSource;
    use source_capture::{CapturedSourceNode, CapturedSourceTree};
    use source_compiler::SourceSyntax;
    use source_compiler::{SourceInput, CompileTimings};
    use source_text::SourceId;
    use std::sync::Arc;

    fn captured_root() -> (semantic_vocabulary::PackageKeyIdentity, PackageCompilationInputs) {
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
        (identity, packages)
    }

    #[test]
    fn unrelated_bundled_packages_do_not_supply_missing_alias_diagnostics() {
        let (root, packages) = captured_root();
        assert_eq!(known_package_spelling(&packages, root, "system"), None);
        assert_eq!(known_package_spelling(&packages, root, "local_route_test"), None);
    }

    #[test]
    fn core_local_import_keeps_its_logical_path_and_issued_contract_precedence() {
        let (_, packages) = captured_root();
        let mut syntax = SourceSyntax::default();
        let parsed = SourceInput(
            [LoadedSource {
                source_id: SourceId(0),
                path: PathBuf::from("unit.omg"),
                source: Arc::from("use integer\\X;"),
                origin: None,
                scope: DependencyScope::Product,
            }]
            .into_iter()
            .collect(),
        )
        .append(&mut syntax, &mut CompileTimings::default())
        .unwrap();
        let unit = &parsed.sources.span_or_empty(parsed.batch)[0];
        let source_compiler::SourceImportTarget::Namespace(members) = &unit.imports[0].target
        else {
            panic!("expected a namespace import")
        };
        let core = super::super::bundled_core_root().unwrap();
        let path = core.join("nested/unit.omg");
        let relative = reconciled_package_import(
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
