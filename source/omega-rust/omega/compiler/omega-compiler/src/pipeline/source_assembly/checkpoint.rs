use super::{
    AssembledSyntax, append_dependency_generated_sources_to_storage, assemble_syntax,
    inject_build_prelude, load_pending_imports, validate_package_source_frontier,
    validate_selected_build_role,
};
use crate::pipeline::PackageCompilationInputs;
use crate::pipeline::frontend::{
    ReconciledPackageImportRequest, discover_target_scoped_imports,
    discover_target_scoped_imports_with_packages, discover_unconditional_imports,
    discover_unconditional_imports_with_packages, extend_source_storage, lex_sources, load_sources,
    parse_sources,
};
use crate::pipeline::project::project_roots;
use crate::pipeline::source::{ImportQueue, SourceStorage};
use crate::pipeline::stage::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use crate::pipeline::timing::CompileTimings;
use psi_diagnostics::Diagnostic;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Immutable physical source and parse frontier formed before exact target
/// generated sources or target-scoped imports join compilation.
#[derive(Clone)]
pub(in crate::pipeline) struct ImmutableSourceParseCheckpoint {
    root_path: PathBuf,
    source_storage: Arc<SourceStorage>,
    build_source_id: Option<psi_source::SourceId>,
    package_imports: Vec<ReconciledPackageImportRequest>,
    package_source_inputs: Option<omega_package_compilation::PackageCompilationSourceInputs>,
}

/// One exact-target child borrowing a shared immutable source checkpoint.
pub(in crate::pipeline) struct ExactTargetSourceAssembly<'a> {
    checkpoint: &'a ImmutableSourceParseCheckpoint,
    target_name: &'a str,
    package_inputs: Option<&'a PackageCompilationInputs>,
}

impl ImmutableSourceParseCheckpoint {
    pub(in crate::pipeline) fn prepare(
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
        for root in project_roots.sources {
            imports.seed(root);
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
        validate_selected_build_role(&source_storage, build_source_id)?;
        Ok(Self {
            root_path: root_path.to_path_buf(),
            source_storage: Arc::new(source_storage),
            build_source_id,
            package_imports,
            package_source_inputs: package_inputs.map(PackageCompilationInputs::source_inputs),
        })
    }

    pub(in crate::pipeline) fn for_exact_target<'a>(
        &'a self,
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

    pub(in crate::pipeline) fn assemble_targetless(
        &self,
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
        if package_inputs.map(PackageCompilationInputs::source_inputs) != self.package_source_inputs
        {
            return Err(vec![Diagnostic::error(
                "package source inputs do not match the immutable source checkpoint",
            )]);
        }
        if let Some(package_inputs) = package_inputs {
            package_inputs.validate_for_compilation(
                &self.root_path,
                &crate::pipeline::frontend::bundled_core_root(),
            )?;
        }
        Ok(())
    }

    fn assemble(
        &self,
        target_name: Option<&str>,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        let mut source_storage = (*self.source_storage).clone();
        let mut imports = ImportQueue::default();
        for (_, source) in source_storage.files.iter() {
            imports.mark_loaded(source.path.clone());
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
            for request in &self.package_imports {
                imports.enqueue(vec![request.resolve_for_exact_target(package_inputs)?])?;
            }
            imports.enqueue(discover_target_scoped_imports_with_packages(
                &source_storage,
                target_name,
            )?)?;
        } else {
            imports.enqueue(discover_target_scoped_imports(
                &source_storage,
                &self.root_path,
                target_name,
            )?)?;
        }
        load_pending_imports(
            &mut source_storage,
            &mut imports,
            &self.root_path,
            target_name,
            package_inputs,
            timings,
        )?;
        let source_scoped_top_level_bindings = inject_build_prelude(
            &mut source_storage,
            self.build_source_id,
            target_name.is_some(),
            timings,
        )?;
        let source_file_count = source_storage.file_count();
        let syntax = assemble_syntax(
            source_storage,
            self.build_source_id,
            source_scoped_top_level_bindings,
            generated_source_custody,
        )?;
        Ok((source_file_count, syntax))
    }
}

impl ExactTargetSourceAssembly<'_> {
    pub(in crate::pipeline) fn assemble(
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
            package_inputs.validate_for_compilation(
                root_path,
                &crate::pipeline::frontend::bundled_core_root(),
            )?;
            let root_package = package_inputs
                .package_root(package_inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf();
            let mut storage = SourceStorage::for_package_compilation(
                root_package,
                package_inputs.root(),
                crate::pipeline::frontend::bundled_core_root(),
            );
            for (identity, source_root) in package_inputs.packages() {
                storage.register_reconciled_package_root(source_root.to_path_buf(), identity);
            }
            Ok(storage)
        }
        None => {
            let root_package = root_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            Ok(SourceStorage::for_compilation(
                root_package,
                crate::pipeline::frontend::bundled_omega_root(),
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
) -> Result<Vec<ReconciledPackageImportRequest>, Vec<Diagnostic>> {
    let mut retained_requests = Vec::new();
    while imports.has_pending() {
        let frontier = imports.take_frontier();
        let frontier = match package_inputs {
            Some(package_inputs) => validate_package_source_frontier(frontier, package_inputs)?,
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
        let discovered = match package_inputs {
            Some(package_inputs) => {
                let (imports, mut requests) = discover_unconditional_imports_with_packages(
                    &parsed,
                    &source_storage.syntax_trees,
                    package_inputs,
                )?;
                retained_requests.append(&mut requests);
                imports
            }
            None => {
                discover_unconditional_imports(&parsed, &source_storage.syntax_trees, root_path)?
            }
        };
        imports.enqueue(discovered)?;
        extend_source_storage(source_storage, parsed)?;
    }
    Ok(retained_requests)
}

#[cfg(test)]
mod tests;
