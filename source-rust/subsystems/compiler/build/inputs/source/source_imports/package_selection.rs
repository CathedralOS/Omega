//! Select an exact package file under the requesting stage’s dependency grants.

use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use source_provenance::DependencyScope;
use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub(crate) struct ReconciledPackageImportRequest {
    pub(super) package: semantic_vocabulary::PackageKeyIdentity,
    pub(super) expected_root: PathBuf,
    pub(super) relative_path: PathBuf,
    dependency_build_forbidden: bool,
    requesting_source: PathBuf,
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
        }
    }

    pub(super) fn exact_file(
        &self,
        packages: &PackageCompilationInputs,
    ) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
        let selected = self.expected_root.join(&self.relative_path);
        self.reject_dependency_build_import(&selected, packages)?;
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
            self.reject_dependency_build_import(path, packages)?;
        }
        Ok(resolved)
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

    fn reject_dependency_build_import(
        &self,
        resolved: &Path,
        packages: &PackageCompilationInputs,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.dependency_build_forbidden
            && packages.dependency_build_file(self.package, resolved)
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
