//! Select a Build and its Product roots from the current frozen source.

use super::source_imports::SourceStorage;
use diagnostics::Diagnostic;
use source_admission::LoadedSource;
use source_compiler::{CompileTimings, RetainedSourceParse, SourceInput};
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
        bindings: &[source_compiler::representations::declaration_selection::ProductEntryBinding],
    ) -> bool {
        use source_compiler::representations::declaration_selection::RootBindingSelection;
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
        files: &source_compiler::representations::source_inputs::SourceMap,
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

/// The command operand can name a directory or a Product source. A returned
/// Build entry selects one particular file in the already acquired source.
#[derive(Clone, Copy)]
pub enum CapturedBuildEntry<'source> {
    Initial(&'source Path),
    Successor {
        path: &'source Path,
        previous_build_files: &'source [PathBuf],
        /// Identity issued from this capture after its anonymous Build returned.
        source_owner: Option<&'source crate::CompletedAnonymousSource>,
    },
}

impl<'a> CapturedBuildEntry<'a> {
    pub(crate) fn path(self) -> &'a Path {
        match self {
            Self::Initial(path) | Self::Successor { path, .. } => path,
        }
    }

    pub(crate) fn check_capture(
        self,
        package_inputs: Option<&crate::PackageCompilationInputs>,
        source_capture: Option<&crate::CapturedLocalSource>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Self::Successor {
            path, source_owner, ..
        } = self
        else {
            return Ok(());
        };
        if let Some(owner) = source_owner {
            owner.require_capture(source_capture)?;
        }
        let package_root = source_capture
            .map(|capture| capture.canonical_live_root())
            .or_else(|| {
                let inputs = package_inputs?;
                let root = inputs.package_root(inputs.root())?;
                (inputs.source_filesystem().has_captured_namespace(root)
                    && inputs.package_for_source(path) == Some(inputs.root()))
                .then_some(root)
            })
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "a successor Build entry requires its issuing stage's frozen source",
                )]
            })?;
        let filename = path.strip_prefix(package_root).ok().and_then(Path::to_str);
        if !filename.is_some_and(|name| {
            crate::declarations::BuildEntryPath::validate(
                &name.replace(std::path::MAIN_SEPARATOR, "/"),
            )
            .is_ok()
        }) {
            return Err(vec![Diagnostic::error(format!(
                "successor Build entry {} must name a canonical .omg file in its frozen source",
                path.display()
            ))]);
        }
        Ok(())
    }
}

/// Source roots for this Build and the Product it can open.
pub(crate) struct StageRoots {
    pub build: Option<(PathBuf, source_text::Span)>,
    pub product: Vec<PathBuf>,
}

impl StageRoots {
    pub fn prepare(
        entry: CapturedBuildEntry<'_>,
        storage: &mut SourceStorage,
    ) -> Result<Self, Vec<Diagnostic>> {
        let root = entry.path();
        let successor = matches!(entry, CapturedBuildEntry::Successor { .. });
        let filesystem = &storage.filesystem;
        let path = if !successor && filesystem.is_dir(root) {
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
        if successor && (!same_file_spelling(&path, root) || !filesystem.is_file(&path)) {
            return Err(vec![Diagnostic::error(format!(
                "successor Build entry {} must select a regular file directly",
                root.display()
            ))]);
        }
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
            .into_parsed_units(&mut CompileTimings::default())?
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
        if successor && build_name.is_none() {
            return Err(vec![Diagnostic::error(format!(
                "successor Build entry {} declares no free build machine",
                path.display()
            ))]);
        }
        let has_main = syntax.free_machines("main").next().is_some();
        let product = if build_name.is_none() || has_main {
            vec![path.clone()]
        } else {
            let main = if successor {
                storage.default_package_root().join("main.omg")
            } else {
                path.with_file_name("main.omg")
            };
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

// Comparing Path components would silently discard "." and repeated separators.
// Windows accepts both slash spellings of the same captured logical separator.
fn same_file_spelling(left: &Path, right: &Path) -> bool {
    let portable = |&byte: &u8| {
        if byte == std::path::MAIN_SEPARATOR as u8 {
            b'/'
        } else {
            byte
        }
    };
    let left = left.as_os_str().as_encoded_bytes();
    let right = right.as_os_str().as_encoded_bytes();
    left.iter().map(portable).eq(right.iter().map(portable))
}
