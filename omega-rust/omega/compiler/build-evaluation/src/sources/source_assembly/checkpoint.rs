use super::{
    AssembledSyntax, append_dependency_generated_sources_to_storage, assemble_syntax,
    load_pending_imports,
};
use artifacts::compile_timings::CompileTimings;
use artifacts::compile_timings::{SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use package_compilation::source_imports::ProjectSourceSelection;
use package_compilation::source_imports::{
    PackageImportPhase, PendingPackageImport, discover_imports, discover_package_imports,
};
use package_compilation::source_imports::{
    SourceStorage, validate_package_source_frontier, validate_selected_build_role,
};
use source_files_to_tokens::ImportQueue;
use source_files_to_tokens::{lex_sources, load_sources};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokens_to_syntax_trees::parse_sources;

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

impl ImmutableSourceParseCheckpoint {
    pub fn prepare(
        root_path: &Path,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<Self, Vec<Diagnostic>> {
        let project = ProjectSourceSelection::prepare(root_path)?;
        let mut imports = ImportQueue::default();
        for (path, scope) in project.sources() {
            imports.seed(path.clone(), *scope);
        }
        let mut source_storage = SourceStorage::prepare(root_path, package_inputs)?;
        let package_imports = load_target_independent_imports(
            &mut source_storage,
            &mut imports,
            root_path,
            package_inputs,
            timings,
        )?;
        let build_source_id = project.build_source_id(&source_storage)?;
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

    /// Assemble the source set for a child with these package inputs: the
    /// parsed physical sources, the retained package imports and the
    /// dependency-generated sources the inputs carry. No target selects
    /// anything here; each target's package inputs were admitted for it.
    pub fn assemble(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.validate_child(package_inputs)?;
        self.assemble_sources(package_inputs, timings)
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
                &package_compilation::source_imports::bundled_core_root()?,
            )?;
        }
        Ok(())
    }

    fn assemble_sources(
        self,
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
        // Every target's entry contract joins the set; the selected target
        // only decides which one realizes the program entry later.
        let seeds = package_compilation::source_imports::entry_contract_seeds(
            package_inputs,
            &source_storage,
        )?;
        if !seeds.is_empty() {
            for seed in seeds {
                imports.seed(
                    seed.register_custody(&mut source_storage),
                    source::DependencyScope::Product,
                );
            }
            load_pending_imports(
                &mut source_storage,
                &mut imports,
                &self.root_path,
                package_inputs,
                timings,
            )?;
        }
        let mut source_scoped_top_level_bindings = Vec::new();
        if let Some(prelude) = package_compilation::source_imports::prepare_build_prelude(
            &source_storage,
            self.build_source_id,
        ) {
            let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
                let sources = source_files_to_tokens::load_injected_source(
                    "<build-prelude>",
                    &prelude.source,
                    source_storage.next_source_id(),
                );
                lex_sources(sources)
            })?;
            let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
                parse_sources(lexed, &mut source_storage.syntax_trees)
            })?;
            source_storage.extend(parsed)?;
            source_scoped_top_level_bindings = prelude.bindings;
        }
        source_scoped_top_level_bindings.extend(
            package_compilation::source_imports::retain_module_import_bindings(&source_storage)?,
        );
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
        source_storage.extend(parsed)?;
    }
    Ok(retained_requests)
}
