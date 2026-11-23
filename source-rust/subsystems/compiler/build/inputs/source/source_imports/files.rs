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
        let relative = core_relative_path(self.path).map_err(|reason| self.error(reason.into()))?;
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
        match self.package_owner(inputs, requester, purpose)? {
            Some(owner) => self.package(inputs, owner, self.root.is_some()),
            None => Ok(FileSelection::Unprepared),
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
        if inputs
            .dependency_target_for_purpose(
                requester,
                super::package_selection::other_purpose(purpose),
                root,
            )
            .is_some()
        {
            return Err(self.error(format!(
                "dependency `{root}` is unavailable in the {} scope",
                purpose.name()
            )));
        }
        if purpose == DependencyPurpose::Build {
            return Ok(None);
        }
        Err(self.error(format!(
            "no direct {} dependency named `{root}` supplies `{}`",
            purpose.name(),
            self.path
        )))
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum FileOwner {
    Package(PackageKeyIdentity),
    Root(PathBuf),
}

impl FileOwner {
    fn of(source: &source_admission::SourceFile) -> Self {
        source
            .package_identity
            .map_or_else(|| Self::Root(source.package_root.clone()), Self::Package)
    }
}

struct AdmittedFileAlias<'a> {
    source: source_text::SourceId,
    span: source_text::Span,
    alias: &'a str,
    owner: FileOwner,
    filename: &'a str,
}

impl AdmittedFileAlias<'_> {
    fn error(&self, message: String) -> Vec<Diagnostic> {
        vec![
            Diagnostic::error(message)
                .with_source_span(source_text::SourceSpan::new(self.source, self.span)),
        ]
    }
}

enum ImportUse {
    GeneratedProduct,
    BuildProductOperand,
}

/// An extension and a designated Product operand can select only a Product
/// source already admitted in that package. The two kinds of use grant different
/// lookup permissions to the referring file.
fn bind_admitted(
    sources: &source_compiler::representations::source_inputs::SourceMap,
    aliases: &[AdmittedFileAlias<'_>],
    use_site: ImportUse,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
    use std::collections::{BTreeMap, BTreeSet};
    if aliases.is_empty() {
        return Ok(Vec::new());
    }
    let owners = aliases
        .iter()
        .map(|entry| &entry.owner)
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeMap::new();
    for file in sources
        .selected_files()
        .filter(|file| file.dependency_scope == DependencyScope::Product)
    {
        let owner = FileOwner::of(file);
        if !owners.contains(&owner) {
            continue;
        }
        let logical = file
            .logical_filename()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        selected
            .entry((owner, logical))
            .and_modify(|target| *target = None)
            .or_insert(Some(file.source_id));
    }
    let mut bindings = Vec::new();
    for import in aliases {
        let target = selected.get(&(import.owner.clone(), import.filename.to_owned()));
        match (target, &use_site) {
            (Some(None), _) => {
                return Err(import.error(format!(
                    "more than one source was admitted as logical file `{}`",
                    import.filename
                )));
            }
            (None, ImportUse::GeneratedProduct) => {
                return Err(import.error(format!("no admitted logical file `{}`", import.filename)));
            }
            (None, ImportUse::BuildProductOperand) => continue,
            (Some(Some(target)), ImportUse::BuildProductOperand) => {
                bindings.push(SourceScopedTopLevelBinding::product_file_import(
                    import.source,
                    *target,
                    import.alias,
                ))
            }
            (Some(Some(target)), ImportUse::GeneratedProduct) => bindings.push(
                SourceScopedTopLevelBinding::file_import(import.source, *target, import.alias),
            ),
        }
    }
    Ok(bindings)
}

pub(crate) fn generated_file_imports(
    parsed: &source_compiler::ParsedSourceBatch,
    sources: &source_compiler::representations::source_inputs::SourceMap,
    inputs: &PackageCompilationInputs,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    let mut aliases = Vec::new();
    for unit in parsed.sources.span_or_empty(parsed.batch) {
        for import in &unit.imports {
            let Some(file) = FileImport::from(unit, import) else {
                continue;
            };
            let (owner, filename) = if file.root == Some("omega") {
                (
                    FileOwner::Root(super::bundled_core_root()?),
                    core_relative_path(file.path).map_err(|reason| file.error(reason.into()))?,
                )
            } else {
                let owner = file
                    .package_owner(inputs, inputs.root(), DependencyPurpose::Product)?
                    .expect("only Build dependencies can remain unprepared");
                (FileOwner::Package(owner), file.path)
            };
            aliases.push(AdmittedFileAlias {
                source: unit.source_id,
                span: import.span,
                alias: file.alias,
                owner,
                filename,
            });
        }
    }
    bind_admitted(sources, &aliases, ImportUse::GeneratedProduct)
}

pub(super) fn build_product_file_imports(
    storage: &super::SourceStorage,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    let mut aliases = Vec::new();
    for (_, roots) in storage.source_roots.iter() {
        if !storage.sources.source_is_selected(roots.source_id) {
            continue;
        }
        let Some(source) = storage.sources.get(roots.source_id) else {
            continue;
        };
        for import in &roots.build_imports {
            let SourceImportTarget::File { root, path, alias } = &import.target else {
                continue;
            };
            let (owner, filename) = match root.as_deref() {
                Some("omega") => (
                    FileOwner::Root(super::bundled_core_root()?),
                    core_relative_path(path).map_err(|reason| {
                        vec![
                            Diagnostic::error(reason)
                                .with_source_span(source.source_span(import.span)),
                        ]
                    })?,
                ),
                Some(root) => {
                    let Some(package) = storage
                        .sources
                        .product_dependency_at(source.source_span(import.span), root)
                    else {
                        continue;
                    };
                    (FileOwner::Package(package), path.as_str())
                }
                None => (FileOwner::of(source), path.as_str()),
            };
            aliases.push(AdmittedFileAlias {
                source: roots.source_id,
                span: import.span,
                alias,
                owner,
                filename,
            });
        }
    }
    bind_admitted(&storage.sources, &aliases, ImportUse::BuildProductOperand)
}

pub(super) fn core_relative_path(path: &str) -> Result<&str, &'static str> {
    path.strip_prefix("language/core/")
        .ok_or("the toolchain root {omega} supplies only language/core/ files")
}

/// Only a regular member at the requested spelling in the immutable capture
/// can supply an authored file. In particular a symlink cannot change its owner.
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
                .with_captured_source(Arc::new(
                    CapturedSourceTree::new(paths.iter().map(|path| {
                        (
                            PathBuf::from(path),
                            CapturedSourceNode::File {
                                bytes: Arc::from(b"machine f() {}".as_slice()),
                                executable: false,
                            },
                        )
                    }))
                    .unwrap(),
                ))
                .unwrap()
        };
        let graph = PackageCompilationInputs::new(
            app,
            BuildDeclarationKind::Package,
            vec![
                package(
                    app,
                    "app",
                    &["sub/main.omg", "kit/math/v.omg", "kit/math/other.omg"],
                ),
                package(dependency, "lib", &["math/v.omg", "math/other.omg"]),
            ],
            vec![PackageDependencyBinding::new(app, "kit", dependency)],
        )
        .unwrap();
        let root = graph.package_root(app).unwrap();
        let imports = "use kit/math/v.omg as local; use {kit}/math/v.omg as dep; use {kit}/other/v.omg as absent; use {lib}/math/v.omg as indirect;";
        let parsed = SourceInput(
            [source_admission::LoadedSource {
                source_id: source_text::SourceId(1),
                path: root.join("sub/main.omg"),
                source: Arc::from(imports),
                origin: None,
                scope: DependencyScope::Product,
            }]
            .into_iter()
            .collect(),
        )
        .append(&mut SourceSyntax::default(), &mut CompileTimings::default())
        .unwrap();
        let unit = &parsed.sources.span_or_empty(parsed.batch)[0];
        let resolve = |index: usize| -> Result<Vec<ResolvedSourceImport>, Vec<Diagnostic>> {
            let file = FileImport::from(unit, &unit.imports[index]).unwrap();
            let selected = file.reconciled(
                &graph,
                Some(app),
                DependencyPurpose::Product,
                None,
                graph.source_filesystem(),
            )?;
            let FileSelection::Package(request) = selected else {
                panic!("a selected package file must come from its owner")
            };
            request.resolve_for_exact_target(&graph, &mut Vec::new())
        };
        assert_eq!(
            resolve(0)
                .unwrap()
                .into_iter()
                .map(|file| file.path)
                .collect::<Vec<_>>(),
            [root.join("kit/math/v.omg")]
        );
        assert_eq!(
            resolve(1)
                .unwrap()
                .into_iter()
                .map(|file| file.path)
                .collect::<Vec<_>>(),
            [graph.package_root(dependency).unwrap().join("math/v.omg")]
        );
        assert!(
            resolve(2).unwrap_err()[0]
                .message
                .contains("no admitted logical file `{kit}/other/v.omg`")
        );
        assert!(
            resolve(3).unwrap_err()[0]
                .message
                .contains("no direct product dependency named `lib`")
        );
    }
}

#[cfg(test)]
mod generated_files {
    use super::*;
    use crate::{BuildDeclarationKind, PackageSourceBinding};
    use build_output::PackageGeneratedSource;
    use source_compiler::representations::source_inputs::SourceCatalogue;
    use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
    use std::sync::Arc;

    #[test]
    fn generated_files_can_import_the_checked_base_and_one_another() {
        let key = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("generated-local-import");
        let graph = PackageCompilationInputs::new(
            key,
            BuildDeclarationKind::Package,
            vec![PackageSourceBinding::new(key, "generated", root.clone())],
            vec![],
        )
        .unwrap();
        let mut catalogue = SourceCatalogue::default();
        let mut base = catalogue.begin_selection();
        let authored = catalogue
            .admit_checked_instance(
                &mut base,
                root.join("types/base.omg"),
                "public data Base {}",
                root.clone(),
                Some(key),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                DependencyScope::Product,
            )
            .unwrap()
            .source_id;
        let base = Arc::new(base);
        let generated = |path: &str, text: &str| {
            PackageGeneratedSource::from_retained_parts(
                path.as_bytes().into(),
                Arc::from(text.as_bytes()),
            )
        };
        let units = [
            generated("math/value.omg", "public machine value() -> u8 { 7 }"),
            generated(
                "entry.omg",
                "use math/value.omg; use types/base.omg; machine make() -> u8 { |value|::value() }",
            ),
        ];
        let admit = |units: &[PackageGeneratedSource], catalogue: &mut SourceCatalogue| {
            super::super::admit_generated_extension(&base, &root, Some(key), units, catalogue)
        };
        let extension = admit(&units, &mut catalogue).unwrap();
        let value = extension.custody()[0].0;
        let entry = extension.custody()[1].0;
        let request = extension
            .prepare_source_request(&base, &graph, &mut Default::default())
            .unwrap();
        assert_eq!(
            request.top_level_bindings,
            [
                SourceScopedTopLevelBinding::file_import(entry, value, "value"),
                SourceScopedTopLevelBinding::file_import(entry, authored, "base"),
            ]
        );
        let duplicate = [generated("types/base.omg", "public data Other {}")];
        let Err(errors) = admit(&duplicate, &mut catalogue) else {
            panic!("the authored and generated files have the same logical name");
        };
        assert!(
            errors[0]
                .message
                .contains("collides with an existing source")
        );
    }
}
