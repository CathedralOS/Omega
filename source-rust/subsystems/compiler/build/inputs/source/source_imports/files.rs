//! Resolve an authored filename in its package or a directly granted dependency.
//! Keep a missing Build binding at its import for admission. Local files resolve
//! within the package that owns the importing source.

mod admitted;
mod product_operand;
pub(crate) use admitted::build_product_file_imports;
pub(crate) use product_operand::product_file_sources;

use super::bindings::{ImportOccurrence, PendingPackageImport, ResolvedSourceImport};
use super::package_selection::ReconciledPackageImportRequest;
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source_capture::SourceFileSystem;
use source_compiler::{ParsedSourceSummary, SourceImport, SourceImportTarget};
use source_provenance::DependencyScope;
use std::path::{Path, PathBuf};

pub(super) struct FileImport<'a> {
    pub root: Option<&'a str>,
    pub path: &'a str,
    pub alias: &'a str,
    import: &'a SourceImport,
    source: &'a ParsedSourceSummary,
}

pub(super) enum FileSelection {
    Selected(ResolvedSourceImport),
    Package(PendingPackageImport),
    Unbound,
}

impl<'a> FileImport<'a> {
    pub fn from(
        source: &'a ParsedSourceSummary,
        import: &'a SourceImport,
    ) -> Result<Option<Self>, Vec<Diagnostic>> {
        let SourceImportTarget::File { root, path, alias } = &import.target else {
            // The installed core and target contracts have their own issued source.
            // A package or installed Build helper must select an exact file before entry.
            if source.scope == DependencyScope::Build
                && !matches!(
                    source.origin,
                    Some(
                        source_admission::SourceOrigin::Toolchain
                            | source_admission::SourceOrigin::ToolchainContract
                    )
                )
            {
                let error = Diagnostic::error(format!(
                    "Build import `{}` requires an exact .omg filename",
                    import.target
                ))
                .with_source_span(source_text::SourceSpan::new(source.source_id, import.span));
                return Err(vec![error]);
            }
            return Ok(None);
        };
        Ok(Some(Self {
            root: root.as_deref(),
            path,
            alias,
            import,
            source,
        }))
    }

    fn occurrence(&self) -> ImportOccurrence {
        let spelling = self.root.map_or_else(
            || self.path.to_owned(),
            |root| format!("{{{root}}}/{}", self.path),
        );
        ImportOccurrence::file(
            self.source.source_id,
            self.import,
            spelling,
            self.alias,
            self.root,
        )
    }

    fn error(&self, message: String) -> Vec<Diagnostic> {
        vec![
            Diagnostic::error(message).with_source_span(source_text::SourceSpan::new(
                self.source.source_id,
                self.import.span,
            )),
        ]
    }

    fn select_captured(
        &self,
        root: &Path,
        relative: &str,
        filesystem: &SourceFileSystem,
    ) -> Result<FileSelection, Vec<Diagnostic>> {
        let path = captured_file(root, Path::new(relative), filesystem)?.ok_or_else(|| {
            self.error(format!(
                "no admitted logical file `{}` in {}",
                relative,
                root.display()
            ))
        })?;
        Ok(FileSelection::Selected(ResolvedSourceImport {
            occurrence: self.occurrence(),
            path,
        }))
    }

    fn toolchain(&self, filesystem: &SourceFileSystem) -> Result<FileSelection, Vec<Diagnostic>> {
        if self.source.scope == DependencyScope::Build
            && let Some(relative) = self.path.strip_prefix("build/")
        {
            return self.select_captured(&super::bundled_build_root()?, relative, filesystem);
        }
        let relative = core_relative_path(self.path).map_err(|reason| self.error(reason.into()))?;
        self.select_captured(&super::bundled_core_root()?, relative, filesystem)
    }

    fn unavailable(&self, purpose: DependencyPurpose) -> Result<FileSelection, Vec<Diagnostic>> {
        let root = self
            .root
            .expect("only a rooted import waits for a dependency");
        if purpose == DependencyPurpose::Build {
            return Ok(FileSelection::Unbound);
        }
        Err(self.error(format!(
            "no direct {} dependency named `{root}` supplies `{}`",
            purpose.name(),
            self.path
        )))
    }

    fn package(
        &self,
        inputs: &PackageCompilationInputs,
        package: PackageKeyIdentity,
        foreign: bool,
    ) -> Result<FileSelection, Vec<Diagnostic>> {
        let root = inputs
            .package_root(package)
            .ok_or_else(|| self.error("imported package has no admitted source root".into()))?;
        let request = ReconciledPackageImportRequest::file(
            package,
            root,
            self.path,
            &self.source.path,
            foreign,
        );
        Ok(FileSelection::Package(PendingPackageImport {
            occurrence: self.occurrence(),
            scope: self.source.scope,
            request,
        }))
    }

    pub fn standalone(
        &self,
        current_root: Option<&Path>,
        mounted: Option<&PackageCompilationInputs>,
        filesystem: &SourceFileSystem,
    ) -> Result<FileSelection, Vec<Diagnostic>> {
        if !self.import.used_in(self.source.scope) {
            return Ok(FileSelection::Unbound);
        }
        if self.root == Some("omega") {
            return self.toolchain(filesystem);
        }
        let current_root = current_root.ok_or_else(|| {
            self.error(format!(
                "file import `{}` has no admitted package root",
                self.import.target
            ))
        })?;
        match self.root {
            Some(_) if let Some(inputs) = mounted => self.package(inputs, inputs.root(), true),
            Some(_) => self.unavailable(match self.source.scope {
                DependencyScope::Build => DependencyPurpose::Build,
                DependencyScope::Product => DependencyPurpose::Product,
            }),
            None => self.select_captured(current_root, self.path, filesystem),
        }
    }

    pub fn reconciled(
        &self,
        inputs: &PackageCompilationInputs,
        requester: Option<PackageKeyIdentity>,
        purpose: DependencyPurpose,
        contract_root: Option<&Path>,
        filesystem: &SourceFileSystem,
    ) -> Result<FileSelection, Vec<Diagnostic>> {
        if !self.import.used_in(self.source.scope) {
            return Ok(FileSelection::Unbound);
        }
        if self.root == Some("omega") {
            return self.toolchain(filesystem);
        }
        let Some(requester) = requester else {
            let core = super::bundled_core_root()?;
            let root = contract_root
                .or_else(|| {
                    self.source
                        .path
                        .starts_with(&core)
                        .then_some(core.as_path())
                })
                .ok_or_else(|| self.error("file import has no admitted package owner".into()))?;
            return self.standalone(Some(root), None, filesystem);
        };
        match self.package_owner(inputs, requester, purpose)? {
            Some(owner) => self.package(inputs, owner, self.root.is_some()),
            None => Ok(FileSelection::Unbound),
        }
    }

    fn package_owner(
        &self,
        inputs: &PackageCompilationInputs,
        requester: PackageKeyIdentity,
        purpose: DependencyPurpose,
    ) -> Result<Option<PackageKeyIdentity>, Vec<Diagnostic>> {
        let Some(root) = self.root else {
            return Ok(Some(requester));
        };
        if let Some(target) = inputs.dependency_target_for_purpose(requester, purpose, root) {
            return Ok(Some(target));
        }
        // The same written import may be used by a designated Product operand.
        // A missing host dependency stays pending until an ordinary Build use demands it.
        if purpose == DependencyPurpose::Build {
            return Ok(None);
        }
        if inputs
            .dependency_target_for_purpose(requester, DependencyPurpose::Build, root)
            .is_some()
        {
            return Err(self.error(format!(
                "dependency `{root}` is unavailable in the {} scope",
                purpose.name()
            )));
        }
        Err(self.error(format!(
            "no direct {} dependency named `{root}` supplies `{}`",
            purpose.name(),
            self.path
        )))
    }
}

pub(super) fn is_build_file(root: Option<&str>, path: &str) -> bool {
    root == Some("omega") && path.starts_with("build/")
}

impl super::SourceStorage {
    /// Capture the selected installation before resolving its first authored Build import.
    /// Closed custody keeps the loaded declarations local to their directory and file bindings.
    pub(super) fn capture_imported_build_library(
        &mut self,
        parsed: &source_compiler::ParsedSourceBatch,
    ) -> Result<(), Vec<Diagnostic>> {
        if parsed.sources.span_or_empty(parsed.batch).iter().any(|unit| {
            unit.scope == DependencyScope::Build && unit.imports.iter().any(|import| {
                import.used_in(DependencyScope::Build) && matches!(&import.target,
                    SourceImportTarget::File { root, path, .. } if is_build_file(root.as_deref(), path))
            })
        }) {
            self.admit_installed_root(&super::bundled_build_root()?, super::frontier::InstalledSourceKind::BuildLibrary)?;
        }
        Ok(())
    }
}

pub(super) fn core_relative_path(path: &str) -> Result<&str, &'static str> {
    if path.starts_with("build/") {
        Err("installed Build files are available only in Build scope")
    } else {
        path.strip_prefix("language/core/")
            .ok_or("expected an installed language/core/ file under {omega}")
    }
}

/// Resolve links inside the package's captured tree so every spelling of a file
/// selects its single admitted source.
pub(super) fn captured_file(
    root: &Path,
    relative: &Path,
    filesystem: &SourceFileSystem,
) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
    if !relative
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
        || relative.extension().and_then(|value| value.to_str()) != Some("omg")
    {
        return Err(vec![Diagnostic::error(
            "a logical file must be a package-relative .omg filename",
        )]);
    }
    if !filesystem.has_captured_namespace(root) {
        return Err(vec![Diagnostic::error(format!(
            "logical file import requires an admitted capture for {}",
            root.display()
        ))]);
    }
    let path = root.join(relative);
    let resolved = match filesystem.canonicalize(&path) {
        Ok(resolved) => resolved,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "cannot resolve captured source {}: {error}",
                path.display()
            ))]);
        }
    };
    if !resolved.starts_with(root) {
        return Err(vec![Diagnostic::error(format!(
            "captured source {} resolves outside its package root",
            path.display()
        ))]);
    }
    Ok(
        (resolved.extension().is_some_and(|ext| ext == "omg") && filesystem.is_file(&resolved))
            .then_some(resolved),
    )
}

#[cfg(test)]
mod capture_tests;
#[cfg(test)]
mod generated_tests;
