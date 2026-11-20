//! Select the first Build from the captured source the caller named.

use super::source_imports::SourceStorage;
use diagnostics::Diagnostic;
use source_admission::LoadedSource;
use source_compiler::{RetainedSourceParse, SourceInput, UnmeasuredProduction};
use source_provenance::DependencyScope;
use source_text::SourceId;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, record_storage::Record)]
pub(crate) struct NamedSource {
    pub path: PathBuf,
    pub syntax: Arc<RetainedSourceParse>,
}

impl NamedSource {
    pub fn selected_by(
        &self,
        bindings: &[source_compiler::declaration_selection::ProductEntryBinding],
    ) -> bool {
        use source_compiler::declaration_selection::RootBindingSelection;
        bindings
            .iter()
            .any(|binding| match &binding.implementation {
                RootBindingSelection::Described(description) => {
                    description.declaration.source().path == self.path
                }
                RootBindingSelection::AuthoredPath(name) => {
                    name != "build" && self.syntax.free_machines(name).next().is_some()
                }
            })
    }

    pub fn product_main(
        &self,
        files: &source_compiler::source_inputs::SourceMap,
    ) -> Result<Option<source_admission::RetainedDeclaration>, Vec<Diagnostic>> {
        let Some(file) = files.selected_files().find(|file| {
            file.path == self.path && file.dependency_scope == DependencyScope::Product
        }) else {
            return Ok(None);
        };
        let mut candidates = self.syntax.free_machines("main");
        let Some(name) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err(vec![Diagnostic::error(format!(
                "multiple free main machines in {}",
                self.path.display()
            ))]);
        }
        Ok(Some(
            source_admission::RetainedDeclaration::from_source_occurrence(
                file.clone(),
                name,
                symbols::SymbolKind::Machine,
            ),
        ))
    }
}

/// Source roots for the first Build and the Product it can open.
pub(crate) struct StageRoots {
    pub build: Option<(PathBuf, source_text::Span)>,
    pub product: Vec<PathBuf>,
}

impl StageRoots {
    pub fn prepare(root: &Path, storage: &mut SourceStorage) -> Result<Self, Vec<Diagnostic>> {
        let filesystem = &storage.filesystem;
        let path = if filesystem.is_dir(root) {
            let build = root.join("build.omg");
            if filesystem.is_file(&build) {
                build
            } else {
                root.join("main.omg")
            }
        } else {
            root.to_owned()
        };
        let path = filesystem.canonicalize(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot select source {}: {error}",
                path.display()
            ))]
        })?;
        let source = filesystem.read_to_string(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot read selected source {}: {error}",
                path.display()
            ))]
        })?;
        let unit = LoadedSource {
            source_id: SourceId(usize::MAX),
            path: path.clone(),
            source,
            origin: None,
            scope: DependencyScope::Build,
        };
        let syntax = SourceInput([unit].into_iter().collect())
            .into_parsed_units(&mut UnmeasuredProduction)?
            .pop()
            .ok_or_else(|| vec![Diagnostic::error("parser lost the selected source")])?;
        let build_name = {
            let mut names = syntax.free_machines("build");
            let first = names.next();
            if names.next().is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "multiple free build machines in {}",
                    path.display()
                ))]);
            }
            first
        };
        let has_main = syntax.free_machines("main").next().is_some();
        let product = if build_name.is_none() || has_main {
            vec![path.clone()]
        } else {
            let main = path.with_file_name("main.omg");
            (main != path && filesystem.is_file(&main))
                .then_some(main)
                .into_iter()
                .collect()
        };
        storage.entry = Some(NamedSource {
            path: path.clone(),
            syntax: Arc::new(syntax),
        });
        Ok(Self {
            build: build_name.map(|name| (path, name)),
            product,
        })
    }

    pub fn build_entry(
        &self,
        storage: &SourceStorage,
    ) -> Result<Option<source_text::SourceSpan>, Vec<Diagnostic>> {
        self.build
            .as_ref()
            .map(|(path, name)| {
                storage
                    .sources
                    .selected_files()
                    .find(|file| {
                        file.path == *path && file.dependency_scope == DependencyScope::Build
                    })
                    .map(|file| file.source_span(*name))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "selected build source {} was not admitted",
                            path.display()
                        ))]
                    })
            })
            .transpose()
    }
}
