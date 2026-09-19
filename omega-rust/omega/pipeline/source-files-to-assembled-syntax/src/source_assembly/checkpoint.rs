use super::{
    AssembledSyntax, append_dependency_generated_sources_to_storage, assemble_syntax,
    inject_build_prelude, load_pending_imports, validate_package_source_frontier,
    validate_selected_build_role,
};
use crate::frontend::{
    PackageImportPhase, PendingPackageImport, discover_imports, discover_package_imports,
    extend_source_storage, lex_sources, load_sources, parse_sources,
};
use crate::source::project::project_roots;
use crate::source::{ImportQueue, SourceStorage};
use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Immutable physical source and parse frontier formed before exact target
/// generated sources or target-scoped imports join compilation.
#[derive(Clone)]
pub struct ImmutableSourceParseCheckpoint {
    root_path: PathBuf,
    source_storage: Arc<SourceStorage>,
    build_source_id: Option<source::SourceId>,
    /// The validated `builder.application` declaration retained at
    /// parse-checkpoint custody; its name supplies the `.app` basename and
    /// inner executable leaf, and its artifact-only intent narrows the
    /// admitted route, all without revisiting source text.
    application: Option<build_declarations::ApplicationDeclaration>,
    package_imports: Arc<[PendingPackageImport]>,
    package_source_inputs: Option<Arc<package_compilation::PackageCompilationSourceInputs>>,
}

/// One exact-target child consuming its checkpoint reference. Cloned checkpoints
/// share immutable storage until assembly needs an independently owned child.
pub struct ExactTargetSourceAssembly<'a> {
    checkpoint: ImmutableSourceParseCheckpoint,
    target_name: &'a str,
    package_inputs: Option<&'a PackageCompilationInputs>,
}

impl ImmutableSourceParseCheckpoint {
    pub fn prepare(
        root_path: &Path,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<Self, Vec<Diagnostic>> {
        let project_roots = project_roots(root_path);
        let selected_build_path = project_roots
            .build
            .as_deref()
            .map(|path| {
                path.canonicalize().map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "failed to establish exact build source {}: {error}",
                        path.display()
                    ))]
                })
            })
            .transpose()?;
        let mut imports = ImportQueue::default();
        // Every project root joins as a product-scope instance except the
        // selected build entry, which joins as a build-scope instance only:
        // it and the root-local files its instance transitively imports
        // resolve `build_depend`/`build_depend_as` edges, never product
        // edges. A root-local file imported from both contexts loads one
        // checked instance per scope.
        for root in &project_roots.sources {
            let canonical = root.canonicalize().ok();
            let scope = if canonical.is_some() && canonical == selected_build_path {
                source::DependencyScope::Build
            } else {
                source::DependencyScope::Product
            };
            imports.seed(root.clone(), scope);
        }

        let mut source_storage = initialize_source_storage(root_path, package_inputs)?;
        let package_imports = load_target_independent_imports(
            &mut source_storage,
            &mut imports,
            root_path,
            package_inputs,
            timings,
        )?;
        let build_source_id = selected_build_path
            .as_deref()
            .map(|selected| {
                source_storage
                    .files
                    .iter()
                    .find(|(_, file)| {
                        file.path
                            .canonicalize()
                            .is_ok_and(|loaded| loaded == selected)
                            && source_storage
                                .sources
                                .get(file.source_id)
                                .is_some_and(|source| {
                                    source.dependency_scope == source::DependencyScope::Build
                                })
                    })
                    .map(|(_, file)| file.source_id)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "selected build source {} disappeared from the loaded frontier",
                            selected.display()
                        ))]
                    })
            })
            .transpose()?;
        let application = validate_selected_build_role(&source_storage, build_source_id)?;
        Ok(Self {
            root_path: root_path.to_path_buf(),
            source_storage: Arc::new(source_storage),
            build_source_id,
            application,
            package_imports: package_imports.into(),
            package_source_inputs: package_inputs.map(PackageCompilationInputs::source_inputs),
        })
    }

    pub fn for_exact_target<'a>(
        self,
        target_name: &'a str,
        package_inputs: Option<&'a PackageCompilationInputs>,
    ) -> Result<ExactTargetSourceAssembly<'a>, Vec<Diagnostic>> {
        self.validate_child(package_inputs)?;
        Ok(ExactTargetSourceAssembly {
            checkpoint: self,
            target_name,
            package_inputs,
        })
    }

    pub fn assemble_targetless(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.validate_child(package_inputs)?;
        self.assemble(None, package_inputs, timings)
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
            package_inputs
                .validate_for_compilation(&self.root_path, &crate::frontend::bundled_core_root())?;
        }
        Ok(())
    }

    fn assemble(
        self,
        target_name: Option<&str>,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        let mut source_storage = Arc::unwrap_or_clone(self.source_storage);
        let mut imports = ImportQueue::default();
        for (_, source) in source_storage.files.iter() {
            let scope = source_storage
                .sources
                .get(source.source_id)
                .map(|file| file.dependency_scope)
                .unwrap_or(source::DependencyScope::Product);
            imports.mark_loaded(&source.path, scope);
        }
        let generated_source_custody = match package_inputs {
            Some(package_inputs) => append_dependency_generated_sources_to_storage(
                &mut source_storage,
                &mut imports,
                target_name,
                package_inputs,
                timings,
            )?,
            None => Vec::new(),
        };
        if let Some(package_inputs) = package_inputs {
            for request in self.package_imports.iter() {
                let resolved = request.resolve_for_exact_target(package_inputs)?;
                // A retained request joins its target under the importer's
                // own checked instance — the same scope its discovery ran
                // under, so a dual-purpose path loads a second instance
                // rather than failing a scope claim.
                imports.enqueue(vec![(resolved.path.clone(), request.importer_scope())])?;
                source_storage.resolved_imports.push(resolved);
            }
        }
        load_pending_imports(
            &mut source_storage,
            &mut imports,
            &self.root_path,
            package_inputs,
            timings,
        )?;
        if let Some(seed) =
            super::hosted_entry_contract_seed(target_name, package_inputs, &source_storage)
        {
            if seed.closed_subtree {
                // Only the bundled fallback owns toolchain provenance. A seed
                // selected from the reconciled graph retains its supplier's
                // identity so accepted binding replay can verify that owner.
                source_storage
                    .register_toolchain_contract_root(seed.source.clone(), seed.root.clone());
                source_storage.register_toolchain_contract_dir(seed.root);
            }
            imports.seed(seed.source, source::DependencyScope::Product);
            load_pending_imports(
                &mut source_storage,
                &mut imports,
                &self.root_path,
                package_inputs,
                timings,
            )?;
        }
        let mut source_scoped_top_level_bindings = inject_build_prelude(
            &mut source_storage,
            self.build_source_id,
            target_name.is_some(),
            timings,
        )?;
        source_scoped_top_level_bindings.extend(crate::frontend::retain_module_import_bindings(
            &source_storage,
        )?);
        let source_file_count = source_storage.file_count();
        let syntax = assemble_syntax(
            source_storage,
            self.build_source_id,
            self.application.clone(),
            source_scoped_top_level_bindings,
            generated_source_custody,
        )?;
        Ok((source_file_count, syntax))
    }
}

impl ExactTargetSourceAssembly<'_> {
    pub fn assemble(
        self,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.checkpoint
            .assemble(Some(self.target_name), self.package_inputs, timings)
    }
}

fn initialize_source_storage(
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<SourceStorage, Vec<Diagnostic>> {
    match package_inputs {
        Some(package_inputs) => {
            package_inputs
                .validate_for_compilation(root_path, &crate::frontend::bundled_core_root())?;
            let root_package = package_inputs
                .package_root(package_inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf();
            let mut storage = SourceStorage::for_package_compilation(
                root_package,
                package_inputs.root(),
                crate::frontend::bundled_core_root(),
            );
            for (identity, source_root) in package_inputs.packages() {
                storage.register_reconciled_package_root(source_root.to_path_buf(), identity);
            }
            // The authorized product dependency roster rides with the
            // retained source map so later evaluator queries resolve
            // `alias::path` under each query occurrence's own product scope.
            // `dependencies()` yields product-purpose edges only; build edges
            // never authorize product selection.
            storage
                .sources
                .retain_product_dependency_scope(package_inputs.dependencies());
            Ok(storage)
        }
        None => {
            let root_package = root_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            Ok(SourceStorage::for_compilation(
                root_package,
                crate::frontend::bundled_omega_root(),
            ))
        }
    }
}

fn load_target_independent_imports(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<Vec<PendingPackageImport>, Vec<Diagnostic>> {
    let mut retained_requests = Vec::new();
    while imports.has_pending() {
        let frontier = imports.take_frontier();
        let frontier = match package_inputs {
            Some(package_inputs) => {
                validate_package_source_frontier(frontier, package_inputs, source_storage)?
            }
            None => frontier,
        };
        let first_source_id = source_storage.next_source_id();
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            let sources = load_sources(frontier, first_source_id)?;
            lex_sources(sources)
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        let contract_custody = source_storage.toolchain_contract_custody();
        let discovered = match package_inputs {
            Some(package_inputs) => {
                let (discovered, mut requests) = discover_package_imports(
                    &parsed,
                    &source_storage.syntax_trees,
                    package_inputs,
                    PackageImportPhase::TargetIndependent,
                    &mut source_storage.resolved_imports,
                    &contract_custody,
                )?;
                retained_requests.append(&mut requests);
                discovered
            }
            None => discover_imports(
                &parsed,
                &source_storage.syntax_trees,
                root_path,
                &mut source_storage.resolved_imports,
            )?,
        };
        imports.enqueue(discovered)?;
        extend_source_storage(source_storage, parsed)?;
    }
    Ok(retained_requests)
}

#[cfg(test)]
mod tests;
