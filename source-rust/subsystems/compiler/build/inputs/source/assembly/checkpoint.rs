//! Capture the package sources and rejoin the deferred Product to the completed Build.

use super::imports::{load_imports, parse_admitted};
use super::toolchain::append_default_build;
use super::{
    AssembledSyntax, BuildSourceSelection, DeferredProductSources, ImmutableSourceParseCheckpoint,
};
use crate::PackageCompilationInputs;
use crate::source::entry::{CapturedBuildEntry, StageRoots};
use crate::source::source_imports::{
    ImportQueue, PackageImportPhase, SourceStorage, discover_imports, discover_package_imports,
};
use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use source_compiler::SourceSyntax;
use source_compiler::representations::declaration_selection::{
    ProductDeclarationRequest, ProductEntryBinding, RootBindingSelection,
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

impl Clone for DeferredProductSources {
    fn clone(&self) -> Self {
        Self {
            checkpoint: self.checkpoint.clone_shared(),
        }
    }
}

impl DeferredProductSources {
    pub fn admit_inputs(
        &self,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut checkpoint = self.checkpoint.clone_shared();
        checkpoint.admit_product_inputs(package_inputs)
    }

    pub fn assemble(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        bindings: &[ProductEntryBinding],
        declarations: &[Arc<ProductDeclarationRequest>],
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        let mut checkpoint = self.checkpoint;
        checkpoint.join_product_inputs(package_inputs, timings, catalogue)?;
        let roots = checkpoint.product_roots.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "Product source continuation has already been consumed",
            )]
        })?;
        checkpoint.assemble_sources(
            package_inputs,
            Some(ProductSourceSelection {
                roots,
                bindings,
                declarations,
            }),
            timings,
            catalogue,
        )
    }
}

pub(super) struct ProductSourceSelection<'a> {
    roots: Arc<[std::path::PathBuf]>,
    bindings: &'a [ProductEntryBinding],
    declarations: &'a [Arc<ProductDeclarationRequest>],
}

impl ProductSourceSelection<'_> {
    /// Select Product roots after the dependency Builds have supplied their files.
    /// The import queue already knows which files came from published bytes.
    pub(super) fn seed(
        self,
        storage: &mut SourceStorage,
        imports: &mut ImportQueue,
        packages: Option<&PackageCompilationInputs>,
    ) -> Result<(), Vec<Diagnostic>> {
        use source_provenance::DependencyScope::Product;
        if let Some(named) = &storage.entry
            && named.selected_by(self.bindings)
        {
            imports.seed(named.path.clone(), Product);
        }
        seed_operand_files(imports, storage, packages, self.bindings, self.declarations)?;
        if let Some(packages) = packages {
            seed_described_entries(imports, packages, self.bindings);
            for path in storage.product_subject_sources()? {
                imports.seed(path, Product);
            }
        }
        for root in self.roots.iter() {
            imports.seed(root.clone(), Product);
        }
        Ok(())
    }
}

/// Admit the Product instance of each file named by a declaration operand at its Build source.
fn seed_operand_files(
    imports: &mut ImportQueue,
    storage: &mut SourceStorage,
    packages: Option<&PackageCompilationInputs>,
    bindings: &[ProductEntryBinding],
    declarations: &[Arc<ProductDeclarationRequest>],
) -> Result<(), Vec<Diagnostic>> {
    let entry_operands = bindings.iter().flat_map(|binding| {
        let implementation = match &binding.implementation {
            RootBindingSelection::AuthoredPath(path) => Some(path.as_str()),
            RootBindingSelection::Described(_) => None,
        };
        std::iter::once(binding.slot.as_str())
            .chain(implementation)
            .map(|path| (binding.source_span, path))
    });
    let declaration_operands = declarations.iter().flat_map(|request| {
        request
            .operands
            .iter()
            .map(|operand| (operand.source_span, operand.path.as_str()))
    });
    let mut visited = HashSet::new();
    for (span, path) in entry_operands.chain(declaration_operands) {
        let Some((alias, _)) = path
            .strip_prefix('|')
            .and_then(|path| path.split_once("|::"))
        else {
            continue;
        };
        let source = storage
            .sources
            .file_at(span)
            .and_then(|source| storage.sources.rejoined_source_id(source))
            .ok_or_else(|| {
                vec![
                    Diagnostic::error("product operand lost its authored Build source")
                        .with_source_span(span),
                ]
            })?;
        if visited.insert((source, alias)) {
            let occurrence = source_text::SourceSpan::new(source, span.span);
            for file in crate::source::source_imports::product_file_sources(
                storage, occurrence, alias, packages,
            )? {
                imports.seed(file, source_provenance::DependencyScope::Product);
            }
        }
    }
    Ok(())
}

fn seed_described_entries(
    imports: &mut ImportQueue,
    packages: &PackageCompilationInputs,
    bindings: &[ProductEntryBinding],
) {
    let mut closure = None;
    for binding in bindings {
        let RootBindingSelection::Described(entry) = &binding.implementation else {
            continue;
        };
        let source = entry.declaration.source();
        let Some(owner) = source.package_identity else {
            continue;
        };
        let capture = packages.source_filesystem();
        if source.origin == source_admission::SourceOrigin::User
            && source.resolution_stratum == source_admission::SourceResolutionStratum::Base
            && source.dependency_scope == source_provenance::DependencyScope::Product
            && packages.package_root(owner) == Some(source.package_root.as_path())
            && packages.package_for_source(&source.path) == Some(owner)
            && closure
                .get_or_insert_with(|| packages.dependency_closure())
                .packages()
                .contains(&owner)
            && capture.has_captured_namespace(&source.package_root)
            && capture
                .read_to_string(&source.path)
                .is_ok_and(|bytes| bytes == source.source)
        {
            imports.seed(source.path.clone(), source.dependency_scope);
        }
    }
}

impl ImmutableSourceParseCheckpoint {
    pub fn admit_root(&self, root: &Path) -> Result<(), Vec<Diagnostic>> {
        if root == self.root_path {
            return Ok(());
        }
        let resolve = |path: &Path| {
            self.source_storage
                .filesystem
                .canonicalize(path)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "cannot resolve prepared source entry: {reason}"
                    ))]
                })
        };
        if resolve(root)? != resolve(&self.root_path)? {
            return Err(vec![Diagnostic::error(
                "checked child compilation root does not match its prepared source checkpoint",
            )]);
        }
        Ok(())
    }

    fn admit_product_inputs(
        &mut self,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.package_source_inputs.is_none()
            && let Some(inputs) = package_inputs
        {
            Arc::make_mut(&mut self.source_storage).join_package_inputs(inputs)?;
            self.package_source_inputs = Some(inputs.source_inputs());
        }
        self.validate_child(package_inputs)
    }

    fn join_product_inputs(
        &mut self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(inputs) = package_inputs.filter(|_| self.package_source_inputs.is_none()) else {
            return self.validate_child(package_inputs);
        };
        let selected_build = self
            .build_source
            .map(|choice| {
                self.source_storage
                    .sources
                    .get(choice.source_id())
                    .cloned()
                    .map(|file| (choice, file))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected Build source disappeared before package join",
                        )]
                    })
            })
            .transpose()?;
        self.admit_product_inputs(Some(inputs))?;
        let storage = Arc::make_mut(&mut self.source_storage);
        let loaded = storage.reselect_current_sources(catalogue)?;
        storage
            .sources
            .retain_product_dependency_scope(inputs.dependencies());
        let parsed = parse_admitted(storage, loaded.into_iter().collect(), timings)?;
        let (discovered, mut requests) = discover_package_imports(
            &parsed,
            inputs,
            PackageImportPhase::TargetIndependent,
            &mut storage.resolved_imports,
            &storage.toolchain_contract_custody,
            &mut storage.package_roots,
            &storage.filesystem,
        )?;
        storage.attach_parsed(parsed);
        let mut imports = ImportQueue::default();
        for file in storage.sources.selected_files() {
            imports.mark_loaded(&file.path, file.dependency_scope);
        }
        imports.enqueue(discovered)?;
        requests.extend(load_imports(
            storage,
            &mut imports,
            Some(inputs),
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?);
        self.build_source = selected_build
            .map(|(choice, file)| {
                storage
                    .sources
                    .rejoined_source_id(&file)
                    .map(|id| choice.rejoined(id))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected Build lost its package-join correspondence",
                        )]
                    })
            })
            .transpose()?;
        self.package_imports = requests.into();
        Ok(())
    }

    pub fn fork(&mut self) -> Self {
        if let Some(storage) = Arc::get_mut(&mut self.source_storage) {
            storage.syntax_trees.share_pages();
        }
        self.clone_shared()
    }

    pub(super) fn clone_shared(&self) -> Self {
        Self {
            root_path: self.root_path.clone(),
            source_storage: self.source_storage.clone(),
            build_source: self.build_source,
            package_imports: self.package_imports.clone(),
            package_source_inputs: self.package_source_inputs.clone(),
            product_roots: self.product_roots.clone(),
        }
    }

    pub fn toolchain_capture(&self) -> Option<&crate::CapturedLocalSource> {
        self.source_storage.core_capture()
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn prepare_from_sources(
        root_path: &Path,
        package_sources: Option<Arc<crate::PackageCompilationSourceInputs>>,
        source_capture: Option<&crate::CapturedLocalSource>,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<Self, Vec<Diagnostic>> {
        let package_inputs = package_sources
            .map(|sources| PackageCompilationInputs::from_parts(sources, Default::default()))
            .transpose()
            .map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| Diagnostic::error(error.to_string()))
                    .collect::<Vec<_>>()
            })?;
        Self::prepare(
            CapturedBuildEntry::Initial(root_path),
            package_inputs.as_ref(),
            source_capture,
            timings,
            catalogue,
        )
    }

    pub fn prepare(
        entry: CapturedBuildEntry<'_>,
        package_inputs: Option<&PackageCompilationInputs>,
        source_capture: Option<&crate::CapturedLocalSource>,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<Self, Vec<Diagnostic>> {
        entry.check_capture(package_inputs, source_capture)?;
        let root_path = entry.path();
        let mut source_storage =
            SourceStorage::prepare(root_path, package_inputs, source_capture, catalogue)?;
        if let CapturedBuildEntry::Successor {
            previous_build_files, ..
        } = entry
        {
            source_storage.previous_build_files = previous_build_files.into();
        }
        let project = StageRoots::prepare(entry, &mut source_storage)?;
        let mut imports = ImportQueue::default();
        if let Some((path, _)) = &project.build {
            imports.seed(path.clone(), source_provenance::DependencyScope::Build);
        }
        let package_imports = load_imports(
            &mut source_storage,
            &mut imports,
            package_inputs,
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?;
        let build_source = Some(match project.build_entry(&source_storage)? {
            Some(entry) => BuildSourceSelection::authored(entry),
            None => append_default_build(&mut source_storage, timings, catalogue)?,
        });
        Ok(Self {
            root_path: root_path.to_path_buf(),
            source_storage: Arc::new(source_storage),
            build_source,
            package_imports: package_imports.into(),
            package_source_inputs: package_inputs.map(PackageCompilationInputs::source_inputs),
            product_roots: Some(project.product.into()),
        })
    }

    pub(super) fn rejoin_sources(
        &mut self,
        current: &source_compiler::representations::source_inputs::SourceMap,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
        catalogue: &mut source_compiler::representations::source_inputs::SourceCatalogue,
    ) -> Result<(), Vec<Diagnostic>> {
        let previous = &self.source_storage.sources;
        let selected = previous
            .selected_files()
            .map(|file| {
                current.rejoined_source_id(file).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "deferred Product source lost its executed Build correspondence",
                    )]
                })
            })
            .collect::<Result<HashSet<_>, _>>()?;
        let primary = previous
            .get(self.source_storage.syntax_trees.primary_source())
            .and_then(|file| current.rejoined_source_id(file))
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "deferred Product parser lost its primary source correspondence",
                )]
            })?;
        if let Some(build) = self.build_source {
            self.build_source = Some(
                previous
                    .get(build.source_id())
                    .and_then(|file| current.rejoined_source_id(file))
                    .map(|source_id| build.rejoined(source_id))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "deferred Product lost its authored Build source correspondence",
                        )]
                    })?,
            );
        }
        let mut sources = current.clone();
        sources.retain_selected_sources(|file| selected.contains(&file.source_id));
        let loaded = sources
            .selected_files()
            .map(|file| source_admission::LoadedSource {
                source_id: file.source_id,
                path: file.path.clone(),
                source: file.source.clone(),
                origin: Some(file.origin),
                scope: file.dependency_scope,
            })
            .collect::<source_admission::LoadedSources>();
        let storage = Arc::make_mut(&mut self.source_storage);
        storage.sources = sources;
        storage.syntax_trees = SourceSyntax::new(primary);
        storage.source_roots = Default::default();
        storage.resolved_imports.clear();
        let parsed = parse_admitted(storage, loaded, timings)?;
        let (discovered, mut requests) = match package_inputs {
            Some(inputs) => discover_package_imports(
                &parsed,
                inputs,
                PackageImportPhase::TargetIndependent,
                &mut storage.resolved_imports,
                &storage.toolchain_contract_custody,
                &mut storage.package_roots,
                &storage.filesystem,
            )?,
            None => (
                discover_imports(&parsed, storage)?,
                Vec::new(),
            ),
        };
        storage.attach_parsed(parsed);
        let mut imports = ImportQueue::default();
        for file in storage.sources.selected_files() {
            imports.mark_loaded(&file.path, file.dependency_scope);
        }
        imports.enqueue(discovered)?;
        requests.extend(load_imports(
            storage,
            &mut imports,
            package_inputs,
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?);
        self.package_imports = requests.into();
        Ok(())
    }

    pub(super) fn validate_child(
        &self,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Result<(), Vec<Diagnostic>> {
        if package_inputs
            .map(PackageCompilationInputs::source_inputs)
            .as_deref()
            != self.package_source_inputs.as_deref()
        {
            return Err(vec![Diagnostic::error(
                "package source inputs do not match the immutable source checkpoint",
            )]);
        }
        if let Some(package_inputs) = package_inputs {
            package_inputs.validate_for_compilation(
                &self.root_path,
                &crate::source::source_imports::bundled_core_root()?,
            )?;
        }
        Ok(())
    }
}
