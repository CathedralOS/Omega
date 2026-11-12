/*
We assemble one syntax forest (`AssembledSyntax`) with exact package, import, and generated-source custody across the two-phase `Build` and `Product` compilation lifecycle.

`ImmutableSourceParseCheckpoint::prepare` consumes an acquired source capture (or captures physical sources), then parses the authored `build.omg` import closure or the compiler-supplied `<default-build>` template while holding `Product` roots deferred. Calling `assemble` on that checkpoint produces the initial `Build`-scope `AssembledSyntax` along with a `DeferredProductSources` continuation. After the `Build` machine returns and its dependencies settle, `DeferredProductSources::assemble` joins the settled `PackageCompilationInputs`, opens the `Product` roots, attaches dependency-generated source handoffs, seeds target `ProgramEntry` contract sources and implicit `core` vocabulary (`code_layout.omg`, `float_operations.omg`), seals the `Product` source owner, and produces the final `Product`-scope `AssembledSyntax`.
*/

/*
Why `Product` roots stay unopened until after the `Build` join:
Every compilation executes an authored or compiler-supplied `Build` machine before opening `Product` roots so `Product` imports cannot leak names into the initial `Build` name-resolution context, and so `Product` imports can resolve against the dependency graph and generated-source bundles settled by `Build`. Both phases read from the same captured `SourceFileSystem` snapshot without recapturing host files at the join.

Why each preparation reselects a fresh consumer cohort (`reselect_current_sources`):
Reusing parser `SourceId` coordinates across discovery and final binding would cause the earlier authored-selection ledger to own freshly resolved calls in the new phase. Reselecting current sources mints a fresh consumer cohort while recording `retain_source_rejoin` correspondences so `BuildSourceSelection` and deferred checkpoints track their exact `SourceId` across transitions.
*/

/*
Provenance:
- `BuildSourceSelection` and `<build-prelude>` source identity: `src/source/source_assembly/build_source.rs`.
- Import discovery, frontier storage, and generated-source admission: `src/source/source_imports.rs`.
*/

mod build_source;
pub use build_source::BuildSourceSelection;

use crate::PackageCompilationInputs;
use crate::source::source_imports::{
    GeneratedSourceExtension, ImportQueue, PackageImportPhase, PendingPackageImport,
    ProjectSourceSelection, SourceStorage, discover_imports, discover_imports_with_packages,
    discover_package_imports, validate_package_source_frontier,
};
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use psi_compiler::{PsiCompilationObserver, PsiCompilationStage, SourceInput};
use source::{SourceContents, SourceOrigin, SourceResolutionStratum};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

#[derive(Clone)]
pub struct AssembledSyntax {
    pub root_sources: Vec<source::SourceId>, // Selected root-package `SourceId`s.
    pub syntax_trees: SyntaxTrees,           // Parsed syntax forest.
    pub source_items: Vec<(source::SourceId, Vec<syntax_trees::item::ItemHandle>)>, // Parser-issued authored item roots per source.
    pub sources: Arc<source::SourceMap>, // Selected `SourceMap` cohort.
    pub anonymous_owner: Option<crate::AnonymousSourceCapture>, // Unbound anonymous capture carried until `Product` sealing.
    pub product_sources: Option<DeferredProductSources>, // Deferred `Product` continuation held during `Build` evaluation.
    pub build_source: Option<BuildSourceSelection>, // Exact authored or compiler-default `Build` source selection.
    pub source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>, // Build-prelude aliases and module import bindings.
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>, // Admitted dependency-generated sources keyed by `SourceId`.
    pub build_scope_sources: HashSet<source::SourceId>, // Sources owned by the `Build` execution scope.
}

#[derive(compiler_storage::Record)]
pub struct ImmutableSourceParseCheckpoint {
    root_path: PathBuf,
    source_storage: Arc<SourceStorage>,
    build_source: Option<BuildSourceSelection>,
    package_imports: Arc<[PendingPackageImport]>,
    package_source_inputs: Option<Arc<crate::PackageCompilationSourceInputs>>,
    product_roots: Option<Arc<[PathBuf]>>,
}

#[derive(compiler_storage::Record)]
pub struct DeferredProductSources {
    checkpoint: ImmutableSourceParseCheckpoint,
}

impl Clone for DeferredProductSources {
    fn clone(&self) -> Self {
        Self {
            checkpoint: self.checkpoint.clone_shared(),
        }
    }
}

impl DeferredProductSources {
    pub fn assemble_query(
        &self,
        inputs: &crate::source::source_imports::ProductQueryInputs,
        timings: &mut CompileTimings,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<AssembledSyntax, Vec<Diagnostic>> {
        let checkpoint = &self.checkpoint;
        checkpoint.validate_child(inputs.original())?;
        let mut storage = checkpoint.source_storage.as_ref().clone();
        let loaded = storage.reselect_current_sources(catalogue)?;
        inputs.mount(&mut storage)?;
        let build_source = checkpoint
            .build_source
            .map(|original| {
                storage
                    .sources
                    .get(original.source_id())
                    .and_then(|file| storage.sources.rejoined_source_id(file))
                    .map(|source_id| original.rejoined(source_id))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "Product query lost its authored Build correspondence",
                        )]
                    })
            })
            .transpose()?;
        let parsed = SourceInput(loaded.into_iter().collect())
            .append(&mut storage.syntax_trees, &mut ParsingTimings(timings))?;
        let discovered = inputs.discover(&parsed, &mut storage)?;
        storage.attach_parsed(parsed);
        let mut imports = ImportQueue::default();
        for source in storage.sources.selected_files() {
            imports.mark_loaded(&source.path, source.dependency_scope);
        }
        imports.enqueue(discovered)?;
        for root in checkpoint
            .product_roots
            .iter()
            .flat_map(|roots| roots.iter())
        {
            imports.seed(root.clone(), source::DependencyScope::Product);
        }
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        imports.seed(
            crate::source::source_imports::bundled_core_root()?.join("code_layout.omg"),
            source::DependencyScope::Product,
        );
        for scope in [
            source::DependencyScope::Build,
            source::DependencyScope::Product,
        ] {
            if sources_mention_floats(&storage, scope) {
                imports.seed(
                    crate::source::source_imports::bundled_core_root()?
                        .join("float_operations.omg"),
                    scope,
                );
            }
        }
        for seed in inputs.entry_contract_seeds(&storage)? {
            imports.seed(
                seed.register_custody(&mut storage),
                source::DependencyScope::Product,
            );
        }
        load_source_frontiers(
            &mut storage,
            &mut imports,
            SourceRouting::Query(inputs),
            timings,
            catalogue,
        )?;
        let mut bindings = append_build_prelude(&mut storage, build_source, timings, catalogue)?;
        bindings.extend(crate::source::source_imports::retain_module_import_bindings(&storage)?);
        let root_sources = storage.root_sources();
        let source_items = storage
            .source_roots
            .iter()
            .map(|(_, roots)| (roots.source_id, roots.root_items.clone()))
            .collect();
        let build_scope_sources = storage
            .sources
            .selected_files()
            .filter(|file| file.dependency_scope == source::DependencyScope::Build)
            .map(|file| file.source_id)
            .collect();
        Ok(AssembledSyntax {
            root_sources,
            source_items,
            syntax_trees: storage.syntax_trees,
            sources: Arc::new(storage.sources),
            anonymous_owner: None,
            product_sources: None,
            build_source,
            source_scoped_top_level_bindings: bindings,
            generated_source_custody: Vec::new(),
            build_scope_sources,
        })
    }

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
        timings: &mut CompileTimings,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        let mut checkpoint = self.checkpoint;
        checkpoint.join_product_inputs(package_inputs, timings, catalogue)?;
        let roots = checkpoint.product_roots.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "Product source continuation has already been consumed",
            )]
        })?;
        let mut storage = Arc::unwrap_or_clone(checkpoint.source_storage);
        let mut imports = ImportQueue::default();
        for source in storage.sources.selected_files() {
            imports.mark_loaded(&source.path, source.dependency_scope);
        }
        for root in roots.iter() {
            imports.seed(root.clone(), source::DependencyScope::Product);
        }
        let mut package_imports = checkpoint.package_imports.to_vec();
        package_imports.extend(load_imports(
            &mut storage,
            &mut imports,
            &checkpoint.root_path,
            package_inputs,
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?);
        checkpoint.source_storage = Arc::new(storage);
        checkpoint.package_imports = package_imports.into();
        checkpoint.assemble(package_inputs, timings, catalogue)
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
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(inputs) = package_inputs.filter(|_| self.package_source_inputs.is_none()) else {
            return self.validate_child(package_inputs);
        };
        self.admit_product_inputs(Some(inputs))?;
        let storage = Arc::make_mut(&mut self.source_storage);
        let default_source = self
            .build_source
            .filter(|choice| choice.is_default())
            .map(|choice| {
                storage
                    .sources
                    .get(choice.source_id())
                    .cloned()
                    .map(|file| (choice, file))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "default Build source disappeared before package join",
                        )]
                    })
            })
            .transpose()?;
        let loaded = storage.reselect_current_sources(catalogue)?;
        storage
            .sources
            .retain_product_dependency_scope(inputs.dependencies());
        let parsed = SourceInput(loaded.into_iter().collect())
            .append(&mut storage.syntax_trees, &mut ParsingTimings(timings))?;
        let (discovered, mut requests) = discover_package_imports(
            &parsed,
            &storage.syntax_trees,
            inputs,
            PackageImportPhase::TargetIndependent,
            &mut storage.resolved_imports,
            &storage.toolchain_contract_custody,
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
            &self.root_path,
            Some(inputs),
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?);
        self.build_source = match default_source {
            Some((choice, file)) => Some(choice.rejoined(
                storage.sources.rejoined_source_id(&file).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "default Build source lost its package-join correspondence",
                    )]
                })?,
            )),
            None => ProjectSourceSelection::prepare(&self.root_path, &storage.filesystem)?
                .build_source_id(storage)?
                .map(BuildSourceSelection::authored),
        };
        self.package_imports = requests.into();
        Ok(())
    }

    pub fn fork(&mut self) -> Self {
        if let Some(storage) = Arc::get_mut(&mut self.source_storage) {
            storage.syntax_trees.share_pages();
        }
        self.clone_shared()
    }

    fn clone_shared(&self) -> Self {
        Self {
            root_path: self.root_path.clone(),
            source_storage: self.source_storage.clone(),
            build_source: self.build_source,
            package_imports: self.package_imports.clone(),
            package_source_inputs: self.package_source_inputs.clone(),
            product_roots: self.product_roots.clone(),
        }
    }

    pub fn with_toolchain_capture(
        mut self,
        capture: &crate::CapturedLocalSource,
    ) -> Result<Self, Vec<Diagnostic>> {
        Arc::make_mut(&mut self.source_storage).admit_toolchain_capture(capture)?;
        if self.product_roots.is_none() {
            return Err(vec![Diagnostic::error(
                "toolchain capture admission must precede Product typing",
            )]);
        }
        Ok(self)
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn prepare_from_sources(
        root_path: &Path,
        package_sources: Option<Arc<crate::PackageCompilationSourceInputs>>,
        source_capture: Option<&crate::CapturedLocalSource>,
        timings: &mut CompileTimings,
        catalogue: &mut source::SourceCatalogue,
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
            root_path,
            package_inputs.as_ref(),
            source_capture,
            timings,
            catalogue,
        )
    }

    pub fn prepare(
        root_path: &Path,
        package_inputs: Option<&PackageCompilationInputs>,
        source_capture: Option<&crate::CapturedLocalSource>,
        timings: &mut CompileTimings,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut source_storage =
            SourceStorage::prepare(root_path, package_inputs, source_capture, catalogue)?;
        let project = ProjectSourceSelection::prepare(root_path, &source_storage.filesystem)?;
        let mut imports = ImportQueue::default();
        let mut product_roots = Vec::new();
        for (path, scope) in project.sources() {
            if *scope == source::DependencyScope::Product {
                product_roots.push(path.clone());
            } else {
                imports.seed(path.clone(), *scope);
            }
        }
        let package_imports = load_imports(
            &mut source_storage,
            &mut imports,
            root_path,
            package_inputs,
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?;
        let build_source = Some(match project.build_source_id(&source_storage)? {
            Some(source_id) => BuildSourceSelection::authored(source_id),
            None => append_default_build(&mut source_storage, timings, catalogue)?,
        });
        Ok(Self {
            root_path: root_path.to_path_buf(),
            source_storage: Arc::new(source_storage),
            build_source,
            package_imports: package_imports.into(),
            package_source_inputs: package_inputs.map(PackageCompilationInputs::source_inputs),
            product_roots: Some(product_roots.into()),
        })
    }

    pub fn assemble(
        mut self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.validate_child(package_inputs)?;
        let deferred_product = self.product_roots.is_some().then(|| self.fork());
        let source_scope = if deferred_product.is_some() {
            source::DependencyScope::Build
        } else {
            source::DependencyScope::Product
        };
        let mut source_storage = Arc::unwrap_or_clone(self.source_storage);
        let mut imports = ImportQueue::default();
        for source in source_storage.sources.selected_files() {
            imports.mark_loaded(&source.path, source.dependency_scope);
        }
        let generated_source_custody = match package_inputs {
            Some(package_inputs) => append_dependency_build_handoffs_to_storage(
                &mut source_storage,
                &mut imports,
                package_inputs,
                source_scope,
                timings,
                catalogue,
            )?,
            None => Vec::new(),
        };
        macro_rules! load_phase {
            ($phase:expr) => {
                load_imports(
                    &mut source_storage,
                    &mut imports,
                    &self.root_path,
                    package_inputs,
                    $phase,
                    timings,
                    catalogue,
                )
            };
        }
        if let Some(package_inputs) = package_inputs {
            for request in self.package_imports.iter() {
                let resolved = request.resolve_for_exact_target(package_inputs)?;
                imports.enqueue(vec![(resolved.path.clone(), request.importer_scope())])?;
                source_storage.resolved_imports.push(resolved);
            }
        }
        load_phase!(PackageImportPhase::ExactTarget(None))?;
        let seeds = if deferred_product.is_none() {
            crate::source::source_imports::entry_contract_seeds(package_inputs, &source_storage)?
        } else {
            Vec::new()
        };
        if !seeds.is_empty() {
            for seed in seeds {
                imports.seed(
                    seed.register_custody(&mut source_storage),
                    source::DependencyScope::Product,
                );
            }
            load_phase!(PackageImportPhase::ExactTarget(None))?;
        }
        if deferred_product.is_none()
            && let Some(inputs) = package_inputs
        {
            loop {
                let seeds = inputs.product_selection_source_seeds(Some(&source_storage.sources))?;
                let mut added = false;
                for seed in seeds {
                    let already_selected = source_storage.sources.selected_files().any(|file| {
                        file.dependency_scope == source::DependencyScope::Product
                            && file.path == seed
                    });
                    if !already_selected {
                        imports.seed(seed, source::DependencyScope::Product);
                        added = true;
                    }
                }
                if !added {
                    break;
                }
                load_phase!(PackageImportPhase::ExactTarget(None))?;
            }
        }
        let core_root = crate::source::source_imports::bundled_core_root()?;
        let query_types = core_root.join("code_layout.omg");
        imports.seed(query_types, source_scope);
        if sources_mention_floats(&source_storage, source::DependencyScope::Build) {
            imports.seed(
                core_root.join("float_operations.omg"),
                source::DependencyScope::Build,
            );
        }
        if sources_mention_floats(&source_storage, source::DependencyScope::Product)
            || (deferred_product.is_none()
                && sources_mention_floats(&source_storage, source::DependencyScope::Build))
        {
            imports.seed(
                core_root.join("float_operations.omg"),
                source::DependencyScope::Product,
            );
        }
        load_phase!(PackageImportPhase::ExactTarget(None))?;
        if deferred_product.is_none() {
            source_storage.seal_product_owner(catalogue)?;
        }
        let loaded = source_storage.reselect_current_sources(catalogue)?;
        if let Some(inputs) = package_inputs {
            source_storage
                .sources
                .retain_product_dependency_scope(inputs.dependencies());
        }
        let parsed = SourceInput(loaded.into_iter().collect()).append(
            &mut source_storage.syntax_trees,
            &mut ParsingTimings(timings),
        )?;
        let discovered = match package_inputs {
            Some(inputs) => {
                discover_package_imports(
                    &parsed,
                    &source_storage.syntax_trees,
                    inputs,
                    PackageImportPhase::ExactTarget(None),
                    &mut source_storage.resolved_imports,
                    &source_storage.toolchain_contract_custody,
                )?
                .0
            }
            None => discover_imports(
                &parsed,
                &source_storage.syntax_trees,
                &self.root_path,
                &source_storage.filesystem,
                &mut source_storage.resolved_imports,
            )?,
        };
        source_storage.attach_parsed(parsed);
        imports.enqueue(discovered)?;
        load_phase!(PackageImportPhase::ExactTarget(None))?;
        if let Some(original) = self.build_source {
            let file = source_storage
                .sources
                .get(original.source_id())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "source preparation lost its authored Build source",
                    )]
                })?;
            self.build_source = Some(
                source_storage
                    .sources
                    .rejoined_source_id(file)
                    .map(|source_id| original.rejoined(source_id))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected Build source has no admitted consumer correspondence",
                        )]
                    })?,
            );
        }
        let product_sources = deferred_product
            .map(|mut checkpoint| {
                checkpoint.rejoin_sources(
                    &source_storage.sources,
                    package_inputs,
                    timings,
                    catalogue,
                )?;
                Ok::<_, Vec<Diagnostic>>(DeferredProductSources { checkpoint })
            })
            .transpose()?;
        let mut source_scoped_top_level_bindings =
            append_build_prelude(&mut source_storage, self.build_source, timings, catalogue)?;
        source_scoped_top_level_bindings
            .extend(crate::source::source_imports::retain_module_import_bindings(&source_storage)?);
        let root_sources = source_storage.root_sources();
        let source_file_count = source_storage.file_count();
        let build_scope_sources = source_storage
            .sources
            .selected_files()
            .filter(|file| file.dependency_scope == source::DependencyScope::Build)
            .map(|file| file.source_id)
            .collect();
        let capture = source_storage.retain_anonymous_owner(&generated_source_custody)?;
        let (sources, anonymous_owner) = match capture {
            Some(capture) => (capture.sources().clone(), Some(capture)),
            None => (Arc::new(source_storage.sources), None),
        };
        let syntax = AssembledSyntax {
            root_sources,
            source_items: source_storage
                .source_roots
                .iter()
                .map(|(_, roots)| (roots.source_id, roots.root_items.clone()))
                .collect(),
            syntax_trees: source_storage.syntax_trees,
            sources,
            anonymous_owner,
            product_sources,
            build_source: self.build_source,
            source_scoped_top_level_bindings,
            generated_source_custody,
            build_scope_sources,
        };
        Ok((source_file_count, syntax))
    }

    fn rejoin_sources(
        &mut self,
        current: &source::SourceMap,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
        catalogue: &mut source::SourceCatalogue,
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
            .get(self.source_storage.syntax_trees.source_id)
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
            .map(|file| source::LoadedSource {
                source_id: file.source_id,
                path: file.path.clone(),
                source: file.source.clone(),
                origin: Some(file.origin),
                scope: file.dependency_scope,
            })
            .collect::<source::LoadedSources>();
        let storage = Arc::make_mut(&mut self.source_storage);
        storage.sources = sources;
        storage.syntax_trees = SyntaxTrees::new(primary);
        storage.source_roots = Default::default();
        storage.resolved_imports.clear();
        let parsed =
            SourceInput(loaded).append(&mut storage.syntax_trees, &mut ParsingTimings(timings))?;
        let (discovered, mut requests) = match package_inputs {
            Some(inputs) => discover_package_imports(
                &parsed,
                &storage.syntax_trees,
                inputs,
                PackageImportPhase::TargetIndependent,
                &mut storage.resolved_imports,
                &storage.toolchain_contract_custody,
            )?,
            None => (
                discover_imports(
                    &parsed,
                    &storage.syntax_trees,
                    &self.root_path,
                    &storage.filesystem,
                    &mut storage.resolved_imports,
                )?,
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
            &self.root_path,
            package_inputs,
            PackageImportPhase::TargetIndependent,
            timings,
            catalogue,
        )?);
        self.package_imports = requests.into();
        Ok(())
    }

    fn validate_child(
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

impl GeneratedSourceExtension {
    pub fn parse(
        self,
        base_sources: &Arc<source::SourceMap>,
        timings: &mut CompileTimings,
    ) -> Result<(Arc<source::SourceMap>, Vec<SyntaxTrees>), Vec<Diagnostic>> {
        let (sources, custody) = self.into_source_inputs(base_sources)?;
        let mut generated_sources = Vec::with_capacity(custody.len());
        for (source_id, _) in &custody {
            let source = sources.get(*source_id).ok_or_else(|| {
                vec![Diagnostic::error(
                    "admitted generated source has no retained source instance",
                )]
            })?;
            generated_sources.push(source::LoadedSource {
                source_id: *source_id,
                path: source.path.clone(),
                source: source.source.clone(),
                origin: Some(source.origin),
                scope: source.dependency_scope,
            });
        }
        let units = SourceInput(generated_sources.into_iter().collect())
            .into_units(&mut ParsingTimings(timings))?;
        Ok((sources, units))
    }
}

fn append_dependency_build_handoffs_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: &PackageCompilationInputs,
    source_scope: source::DependencyScope,
    timings: &mut CompileTimings,
    catalogue: &mut source::SourceCatalogue,
) -> Result<Vec<(source::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    let entries = crate::source::source_imports::admit_dependency_build_handoffs(
        package_inputs,
        source_scope,
    )?;

    for entry in &entries {
        imports.mark_loaded(&entry.logical_path, entry.scope);
    }

    let mut retained = Vec::with_capacity(entries.len());
    for entry in entries {
        let text = entry.source_text()?;
        let entry_base = entry.authored_base(package_inputs)?;
        let crate::source::source_imports::DependencyGeneratedSource {
            logical_path,
            source,
            package,
            scope,
        } = entry;
        let admitted = source_storage.admit_source(
            SourceContents {
                path: logical_path,
                source: Arc::from(text),
                scope,
            },
            Some(SourceOrigin::User),
            SourceResolutionStratum::DependencyActivationExtension,
            catalogue,
        )?;
        let source_id = admitted.source_id;
        let parsed = SourceInput([admitted].into_iter().collect()).append(
            &mut source_storage.syntax_trees,
            &mut ParsingTimings(timings),
        )?;
        let discovered = discover_imports_with_packages(
            &parsed,
            &source_storage.syntax_trees,
            package_inputs,
            Some(package),
            &mut source_storage.resolved_imports,
            &source_storage.toolchain_contract_custody,
        )?;
        imports.enqueue(discovered)?;
        imports.enqueue(entry_base)?;
        source_storage.attach_parsed(parsed);
        retained.push((source_id, source.clone()));
    }
    Ok(retained)
}

fn load_imports(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    phase: PackageImportPhase,
    timings: &mut CompileTimings,
    catalogue: &mut source::SourceCatalogue,
) -> Result<Vec<PendingPackageImport>, Vec<Diagnostic>> {
    load_source_frontiers(
        source_storage,
        imports,
        SourceRouting::Compilation {
            root_path,
            package_inputs,
            phase,
        },
        timings,
        catalogue,
    )
}

enum SourceRouting<'inputs> {
    Compilation {
        root_path: &'inputs Path,
        package_inputs: Option<&'inputs PackageCompilationInputs>,
        phase: PackageImportPhase,
    },
    Query(&'inputs crate::source::source_imports::ProductQueryInputs),
}

fn load_source_frontiers(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    routing: SourceRouting<'_>,
    timings: &mut CompileTimings,
    catalogue: &mut source::SourceCatalogue,
) -> Result<Vec<PendingPackageImport>, Vec<Diagnostic>> {
    let mut retained_requests = Vec::new();
    while imports.has_pending() {
        let frontier = imports.take_frontier();
        let frontier = match &routing {
            SourceRouting::Compilation {
                package_inputs: Some(package_inputs),
                ..
            } => validate_package_source_frontier(frontier, package_inputs, source_storage)?,
            SourceRouting::Query(inputs) => inputs.validate_frontier(frontier, source_storage)?,
            _ => frontier,
        };
        let loaded = psi_compiler::load_source_files(
            frontier,
            &source_storage.filesystem,
            &mut ParsingTimings(timings),
        )?;
        let admitted = loaded
            .into_iter()
            .map(|contents| {
                source_storage.admit_source(
                    contents,
                    None,
                    SourceResolutionStratum::Base,
                    catalogue,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if source_storage.source_roots.is_empty()
            && let Some(first) = admitted.first()
        {
            source_storage.syntax_trees.source_id = first.source_id;
        }
        let parsed = SourceInput(admitted.into_iter().collect()).append(
            &mut source_storage.syntax_trees,
            &mut ParsingTimings(timings),
        )?;
        let discovered = match &routing {
            SourceRouting::Compilation {
                package_inputs: Some(package_inputs),
                phase,
                ..
            } => {
                let (discovered, mut requests) = discover_package_imports(
                    &parsed,
                    &source_storage.syntax_trees,
                    package_inputs,
                    *phase,
                    &mut source_storage.resolved_imports,
                    &source_storage.toolchain_contract_custody,
                )?;
                retained_requests.append(&mut requests);
                discovered
            }
            SourceRouting::Compilation {
                root_path,
                package_inputs: None,
                ..
            } => discover_imports(
                &parsed,
                &source_storage.syntax_trees,
                root_path,
                &source_storage.filesystem,
                &mut source_storage.resolved_imports,
            )?,
            SourceRouting::Query(inputs) => inputs.discover(&parsed, source_storage)?,
        };
        imports.enqueue(discovered)?;
        source_storage.attach_parsed(parsed);
    }
    Ok(retained_requests)
}

fn append_default_build(
    storage: &mut SourceStorage,
    timings: &mut CompileTimings,
    catalogue: &mut source::SourceCatalogue,
) -> Result<BuildSourceSelection, Vec<Diagnostic>> {
    let admitted = storage.admit_source(
        SourceContents {
            path: PathBuf::from("<default-build>"),
            source: Arc::from(build_declarations::default_build_source()),
            scope: source::DependencyScope::Build,
        },
        Some(SourceOrigin::Toolchain),
        SourceResolutionStratum::Base,
        catalogue,
    )?;
    let selection = BuildSourceSelection::compiler_default(admitted.source_id);
    let parsed = SourceInput([admitted].into_iter().collect())
        .append(&mut storage.syntax_trees, &mut ParsingTimings(timings))?;
    storage.attach_parsed(parsed);
    Ok(selection)
}

fn append_build_prelude(
    storage: &mut SourceStorage,
    build_source: Option<BuildSourceSelection>,
    timings: &mut CompileTimings,
    catalogue: &mut source::SourceCatalogue,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    let Some(prelude) = crate::source::source_imports::prepare_build_prelude(
        storage,
        build_source.map(BuildSourceSelection::source_id),
    ) else {
        return Ok(Vec::new());
    };
    let admitted = storage.admit_source(
        SourceContents {
            path: PathBuf::from(build_source::BUILD_PRELUDE_PATH),
            source: Arc::from(prelude.source),
            scope: source::DependencyScope::Build,
        },
        Some(SourceOrigin::Toolchain),
        SourceResolutionStratum::Base,
        catalogue,
    )?;
    let bindings = prelude
        .alias_for
        .map(|build_source| {
            symbols::SourceScopedTopLevelBinding::new(build_source, admitted.source_id, "Build")
        })
        .into_iter()
        .collect();
    let parsed = SourceInput([admitted].into_iter().collect())
        .append(&mut storage.syntax_trees, &mut ParsingTimings(timings))?;
    storage.attach_parsed(parsed);
    Ok(bindings)
}

struct ParsingTimings<'timings>(&'timings mut CompileTimings);

fn sources_mention_floats(
    source_storage: &crate::source::source_imports::SourceStorage,
    scope: source::DependencyScope,
) -> bool {
    source_storage
        .sources
        .selected_files()
        .filter(|file| file.dependency_scope == scope)
        .any(|file| {
            let text = file.source.as_ref();
            text.contains("f32")
                || text.contains("f64")
                || text.contains("Float")
                || text.contains("F32")
                || text.contains("F64")
        })
        || source_storage
            .syntax_trees
            .tables
            .expressions
            .iter_expressions()
            .any(|(handle, node)| {
                matches!(node, syntax_trees::expression::ExpressionNode::Float(_))
                    && source_storage
                        .sources
                        .file_at(
                            source_storage
                                .syntax_trees
                                .tables
                                .expressions
                                .source_span(handle),
                        )
                        .is_some_and(|file| {
                            source_storage.sources.source_is_selected(file.source_id)
                                && file.dependency_scope == scope
                        })
            })
}

impl PsiCompilationObserver for ParsingTimings<'_> {
    fn record_result<Value, Error>(
        &mut self,
        stage: PsiCompilationStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let (name, input, output) = stage.observation_labels();
        self.0
            .record_result(StageMeta::new(name, input, output), operation)
    }
}
