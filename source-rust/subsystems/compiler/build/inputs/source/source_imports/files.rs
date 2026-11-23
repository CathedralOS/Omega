//! Resolve an authored filename in its package or a directly granted dependency.
//! A missing rooted Build import can wait for preparation. A missing local file
//! cannot acquire meaning from an unrelated package or another file in its directory.

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
    Unprepared,
}

impl<'a> FileImport<'a> {
    pub fn from(source: &'a ParsedSourceSummary, import: &'a SourceImport) -> Option<Self> {
        let SourceImportTarget::File { root, path, alias } = &import.target else {
            return None;
        };
        Some(Self {
            root: root.as_deref(),
            path,
            alias,
            import,
            source,
        })
    }

    fn occurrence(&self) -> ImportOccurrence {
        let spelling = self.root.map_or_else(
            || self.path.to_owned(),
            |root| format!("{{{root}}}/{}", self.path),
        );
        ImportOccurrence::file(
            self.source.source_id,
            self.import.ordinal,
            spelling,
            self.alias,
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

    fn core(&self, filesystem: &SourceFileSystem) -> Result<FileSelection, Vec<Diagnostic>> {
        let relative = self.path.strip_prefix("language/core/").ok_or_else(|| {
            self.error("the toolchain root {omega} supplies only language/core/ files".into())
        })?;
        self.select_captured(&super::bundled_core_root()?, relative, filesystem)
    }

    fn unavailable(&self, purpose: DependencyPurpose) -> Result<FileSelection, Vec<Diagnostic>> {
        let root = self
            .root
            .expect("only a rooted import waits for a dependency");
        if purpose == DependencyPurpose::Build {
            return Ok(FileSelection::Unprepared);
        }
        Err(self.error(format!(
            "no direct {} dependency named `{root}` supplies `{}`",
            purpose.name(), self.path
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
        current_root: &Path,
        mounted: Option<&PackageCompilationInputs>,
        filesystem: &SourceFileSystem,
    ) -> Result<FileSelection, Vec<Diagnostic>> {
        match self.root {
            Some("omega") => self.core(filesystem),
            Some(_) if let Some(inputs) = mounted => self.package(inputs, inputs.root(), true),
            Some(_) => self.unavailable(match self.source.scope { DependencyScope::Build => DependencyPurpose::Build, DependencyScope::Product => DependencyPurpose::Product }),
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
        if self.root == Some("omega") {
            return self.core(filesystem);
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
            return self.standalone(root, None, filesystem);
        };
        match self.root {
            None => self.package(inputs, requester, false),
            Some(root) => match inputs.dependency_target_for_purpose(requester, purpose, root) {
                Some(target) => self.package(inputs, target, true),
                None if inputs
                    .dependency_target_for_purpose(
                        requester,
                        super::package_selection::other_purpose(purpose),
                        root,
                    )
                    .is_some() =>
                {
                    Err(self.error(format!(
                        "dependency `{root}` is unavailable in the {} scope",
                        purpose.name()
                    )))
                }
                None => self.unavailable(purpose),
            },
        }
    }
}

/// Only a regular member at the requested spelling in the immutable capture
/// can supply an authored file. In particular a symlink cannot change its owner.
pub(super) fn captured_file(
    root: &Path,
    relative: &Path,
    filesystem: &SourceFileSystem,
) -> Result<Option<PathBuf>, Vec<Diagnostic>> {
    if !filesystem.has_captured_namespace(root) {
        return Err(vec![Diagnostic::error(format!(
            "logical file import requires an admitted capture for {}",
            root.display()
        ))]);
    }
    let path = root.join(relative);
    Ok((filesystem.is_file(&path)
        && filesystem
            .canonicalize(&path)
            .is_ok_and(|resolved| resolved == path))
    .then_some(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildDeclarationKind, PackageDependencyBinding, PackageSourceBinding};
    use source_capture::{CapturedSourceNode, CapturedSourceTree};
    use source_compiler::{CompileTimings, SourceInput, SourceSyntax};
    use std::sync::Arc;

    #[test]
    fn local_and_dependency_file_roots_select_only_their_exact_admitted_source() {
        let app = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let dependency = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        let package = |key, name, paths: &[&str]| {
            PackageSourceBinding::new(key, name, Path::new(env!("CARGO_MANIFEST_DIR")).join(name))
                .with_captured_source(Arc::new(CapturedSourceTree::new(paths.iter().map(|path| {
                    (PathBuf::from(path), CapturedSourceNode::File { bytes: Arc::from(b"machine f() {}".as_slice()), executable: false })
                })).unwrap())).unwrap()
        };
        let graph = PackageCompilationInputs::new(app, BuildDeclarationKind::Package,
            vec![package(app, "app", &["sub/main.omg","kit/math/v.omg","kit/math/other.omg"]), package(dependency, "lib", &["math/v.omg","math/other.omg"])],
            vec![PackageDependencyBinding::new(app, "kit", dependency)]).unwrap();
        let root = graph.package_root(app).unwrap();
        let imports = "use kit/math/v.omg as local; use {kit}/math/v.omg as dep; use {kit}/other/v.omg as absent; use {lib}/math/v.omg as indirect;";
        let parsed = SourceInput([source_admission::LoadedSource { source_id: source_text::SourceId(1), path: root.join("sub/main.omg"), source: Arc::from(imports), origin: None, scope: DependencyScope::Product }].into_iter().collect()).append(&mut SourceSyntax::default(), &mut CompileTimings::default()).unwrap();
        let unit = &parsed.sources.span_or_empty(parsed.batch)[0];
        let resolve = |index: usize| -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
            let file = FileImport::from(unit, &unit.imports[index]).unwrap();
            let selected = file.reconciled(&graph, Some(app), DependencyPurpose::Product, None, graph.source_filesystem())?;
            let FileSelection::Package(request) = selected else { panic!("a selected package file must come from its owner") };
            request.resolve_for_exact_target(&graph, &mut Vec::new())
        };
        assert_eq!(resolve(0).unwrap().into_iter().map(|file| file.path).collect::<Vec<_>>(), [root.join("kit/math/v.omg")]);
        assert_eq!(resolve(1).unwrap().into_iter().map(|file| file.path).collect::<Vec<_>>(), [graph.package_root(dependency).unwrap().join("math/v.omg")]);
        assert!(resolve(2).unwrap_err()[0].message.contains("no admitted logical file `{kit}/other/v.omg`"));
        assert!(resolve(3).unwrap_err()[0].message.contains("no direct product dependency named `lib`"));
    }
}
