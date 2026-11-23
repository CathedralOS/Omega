//! Bind file aliases in a checked Product source set without acquiring any new inputs.

use super::{FileImport, core_relative_path};
use crate::PackageCompilationInputs;
use build_declarations::DependencyPurpose;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::SourceImportTarget;
use source_provenance::DependencyScope;
use std::path::PathBuf;

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
                    FileOwner::Root(super::super::bundled_core_root()?),
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

pub(crate) fn build_product_file_imports(
    storage: &crate::source::source_imports::SourceStorage,
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
