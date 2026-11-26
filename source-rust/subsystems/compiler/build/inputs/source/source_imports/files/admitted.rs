//! Bind file aliases in a checked Product source set without acquiring any new inputs.

use super::core_relative_path;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source_admission::{SourceFile, SourceResolutionStratum};
use source_capture::SourceFileSystem;
use source_compiler::SourceImportTarget;
use source_provenance::DependencyScope;
use source_text::SourceId;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum FileOwner {
    Package(PackageKeyIdentity),
    Root(PathBuf),
}

impl FileOwner {
    fn of(source: &SourceFile) -> Self {
        source
            .package_identity
            .map_or_else(|| Self::Root(source.package_root.clone()), Self::Package)
    }
}

struct AdmittedFileAlias<'a> {
    source: SourceId,
    span: source_text::Span,
    alias: &'a str,
    package_alias: Option<&'a str>,
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

struct SelectedFiles<'a> {
    root: &'a Path,
    by_filename: BTreeMap<PathBuf, &'a SourceFile>,
}

impl SelectedFiles<'_> {
    fn resolve(
        &self,
        import: &AdmittedFileAlias<'_>,
        filesystem: &SourceFileSystem,
    ) -> Result<Option<SourceId>, Vec<Diagnostic>> {
        let written = Path::new(import.filename);
        let direct = self.by_filename.get(written).copied();
        // The loader already gave every authored source its captured path.
        if direct.is_some_and(|file| file.resolution_stratum == SourceResolutionStratum::Base)
            || !filesystem.has_captured_namespace(self.root)
        {
            return Ok(direct.map(|file| file.source_id));
        }
        let Some(path) = super::captured_file(self.root, written, filesystem)? else {
            return Ok(direct.map(|file| file.source_id));
        };
        let target = path
            .strip_prefix(self.root)
            .expect("captured files belong to the selected root");
        if target != written && direct.is_some() {
            return Err(import.error(format!(
                "more than one source was admitted as logical file `{}`",
                import.filename
            )));
        }
        Ok(self.by_filename.get(target).map(|file| file.source_id))
    }
}

/// A designated Product operand can select a source already admitted in its package.
fn bind_admitted(
    sources: &source_compiler::representations::source_inputs::SourceMap,
    aliases: &[AdmittedFileAlias<'_>],
    filesystem: &SourceFileSystem,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    use source_compiler::representations::source_symbols::SourceScopedTopLevelBinding;
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
        let files = selected.entry(owner).or_insert_with(|| SelectedFiles {
            root: &file.package_root,
            by_filename: BTreeMap::new(),
        });
        if files
            .by_filename
            .insert(PathBuf::from(&logical), file)
            .is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "more than one source was admitted as logical file `{logical}`"
            ))]);
        }
    }
    let mut bindings = Vec::new();
    for import in aliases {
        let target = match selected.get(&import.owner) {
            Some(files) => files.resolve(import, filesystem)?,
            None => None,
        };
        let dependency = match &import.owner {
            FileOwner::Package(package) => import
                .package_alias
                .filter(|alias| *alias != "omega")
                .map(|alias| (alias, *package)),
            FileOwner::Root(_) => None,
        };
        if let Some(target) = target {
            bindings.push(SourceScopedTopLevelBinding::product_file_import(
                import.source, target, import.alias,
            ).from_dependency(dependency));
        }
    }
    Ok(bindings)
}

pub(crate) fn build_product_file_imports(
    storage: &crate::source::source_imports::SourceStorage,
    query: Option<&crate::source::source_imports::ProductQueryInputs>,
) -> Result<
    Vec<source_compiler::representations::source_symbols::SourceScopedTopLevelBinding>,
    Vec<Diagnostic>,
> {
    // A computed locator can use a file alias without spelling it as an expression.
    let queried: Vec<_> = query
        .into_iter()
        .flat_map(|inputs| inputs.queried_file_aliases())
        .filter_map(|(author, alias)| Some((storage.sources.rejoined_source_id(author)?, alias)))
        .collect();
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
            // A Build import has no corresponding Product alias.
            let used = import.used_in(DependencyScope::Product)
                || queried.contains(&(source.source_id, alias.as_str()));
            if !used
                || (import.used_in(DependencyScope::Build)
                    && super::is_build_file(root.as_deref(), path))
            {
                continue;
            }
            let (owner, filename) = match root.as_deref() {
                Some("omega") => (
                    FileOwner::Root(super::super::bundled_core_root()?),
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
                None => (
                    storage
                        .sources
                        .product_owner_at(source.source_span(import.span))
                        .map_or_else(
                            || FileOwner::Root(source.package_root.clone()),
                            FileOwner::Package,
                        ),
                    path.as_str(),
                ),
            };
            aliases.push(AdmittedFileAlias {
                source: roots.source_id,
                span: import.span,
                alias,
                package_alias: root.as_deref(),
                owner,
                filename,
            });
        }
    }
    bind_admitted(
        &storage.sources,
        &aliases,
        &storage.filesystem,
    )
}
