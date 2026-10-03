//! Assemble one source forest with exact package, import and generated-source custody.
//!
//! `ImmutableSourceParseCheckpoint::prepare` discovers and parses immutable
//! physical sources. `assemble` admits a child's inputs, appends dependency
//! outputs, resolves deferred imports, and injects entry contracts and the Build
//! prelude. Both use one ordered loading loop and the ordinary Psi 00/01 stages.
//! `GeneratedSourceExtension::parse` consumes the activation's admitted output
//! through those same stages, preserving separate generated-unit forests.
//! No Build machine executes here; resolution and typing consume the result.

use crate::PackageCompilationInputs;
use crate::source_imports::{
    GeneratedSourceExtension, PackageImportPhase, PendingPackageImport, ProjectSourceSelection,
    SourceStorage, discover_imports, discover_imports_with_packages, discover_package_imports,
    validate_package_source_frontier, validate_selected_build_role,
};
use artifacts::compile_timings::{CompileTimings, SOURCE_FILES_TO_TOKENS, TOKENS_TO_SYNTAX_TREES};
use diagnostics::Diagnostic;
use source_files_to_tokens::{
    ImportQueue, lex_sources, load_package_generated_source, load_sources,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;
use tokens_to_syntax_trees::parse_sources;

#[derive(Clone)]
pub struct AssembledSyntax {
    pub syntax_trees: SyntaxTrees,
    pub sources: Arc<source::SourceMap>,
    /// Exact companion `build.omg` selected during project discovery. Build
    /// authority is attached to this source, never reconstructed from a leaf
    /// filename after imports have expanded the source frontier.
    pub build_source_id: Option<source::SourceId>,
    /// The validated authored application declaration (name and artifact-only
    /// intent) when the selected build root declares `Build::application(builder, ...)`;
    /// `None` for package/workspace roles or a missing build root. Publication
    /// never re-derives it.
    pub application: Option<build_declarations::ApplicationDeclaration>,
    pub source_scoped_top_level_bindings: Vec<symbols::SourceScopedTopLevelBinding>,
    pub generated_source_custody: Vec<(source::SourceId, build_output::PackageGeneratedSource)>,
    /// Sources the build scope owns: the selected build entry, the
    /// root-local helpers it transitively imports, and every source of a
    /// package the root reaches only through build-purpose edges (including
    /// such a package's own ordinary dependencies) — its physical files and
    /// the generated source its build handed off alike, since both execute in
    /// the same host context. Their target-scoped declarations select against
    /// the admitted build execution profile, never the product target; every
    /// other source — product packages, dual-purpose packages, and their
    /// generated dependency source — is product scope.
    pub build_scope_sources: HashSet<source::SourceId>,
}

/// Immutable physical source and parse frontier formed before exact target
/// generated sources or target-scoped imports join compilation.
#[derive(Clone)]
pub struct ImmutableSourceParseCheckpoint {
    root_path: PathBuf,
    source_storage: Arc<SourceStorage>,
    build_source_id: Option<source::SourceId>,
    /// The validated `Build::application` declaration retained at
    /// parse-checkpoint custody; its name supplies the `.app` basename and
    /// inner executable leaf, and its artifact-only intent narrows the
    /// admitted route, all without revisiting source text.
    application: Option<build_declarations::ApplicationDeclaration>,
    package_imports: Arc<[PendingPackageImport]>,
    package_source_inputs: Option<Arc<crate::PackageCompilationSourceInputs>>,
}

impl ImmutableSourceParseCheckpoint {
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Prepare shared physical sources without admitting a child's target
    /// attachments. Target-local generated bundles join later during assembly.
    pub fn prepare_from_sources(
        root_path: &Path,
        package_sources: Option<Arc<crate::PackageCompilationSourceInputs>>,
        timings: &mut CompileTimings,
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
        Self::prepare(root_path, package_inputs.as_ref(), timings)
    }

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
        let package_imports = load_imports(
            &mut source_storage,
            &mut imports,
            root_path,
            package_inputs,
            PackageImportPhase::TargetIndependent,
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
        let mut generated_source_custody = match package_inputs {
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
        load_imports(
            &mut source_storage,
            &mut imports,
            &self.root_path,
            package_inputs,
            PackageImportPhase::ExactTarget(None),
            timings,
        )?;
        // Every target's entry contract joins the set; the selected target
        // only decides which one realizes the program entry later.
        let seeds = crate::source_imports::entry_contract_seeds(package_inputs, &source_storage)?;
        if !seeds.is_empty() {
            for seed in seeds {
                imports.seed(
                    seed.register_custody(&mut source_storage),
                    source::DependencyScope::Product,
                );
            }
            load_imports(
                &mut source_storage,
                &mut imports,
                &self.root_path,
                package_inputs,
                PackageImportPhase::ExactTarget(None),
                timings,
            )?;
        }
        let mut source_scoped_top_level_bindings = Vec::new();
        if let Some(prelude) =
            crate::source_imports::prepare_build_prelude(&source_storage, self.build_source_id)
        {
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
        // Wire codec machines are compiler-generated source, admitted by the
        // ordinary pipeline like the build prelude above them. They load as a
        // package source under the product root so generated machines carry
        // the product package identity — toolchain-custody provenance would
        // leave package-private selections (every `Schema::encode` call site)
        // without an owner to match against. Nothing is emitted when no call
        // site is generatable.
        if let Some(codec_source) = crate::wire_codec_sources::generated_wire_codec_source(
            &source_storage.syntax_trees,
            &source_storage.sources,
        ) {
            let codec_root = match package_inputs {
                Some(inputs) => inputs
                    .package_root(inputs.root())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| self.root_path.clone()),
                None => self
                    .root_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from(".")),
            };
            // Package-generated custody mirrors the `.omega/generated/`
            // convention `admit_generated_extension` admits: the join path
            // stays virtual, while the retained bytes answer the
            // consumption-commitment re-read instead of a physical file.
            let codec_path = codec_root.join(".omega/generated/wire-codec.omg");
            let codec_source_id = source::SourceId(source_storage.next_source_id());
            let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
                let sources = source_files_to_tokens::load_package_generated_source(
                    codec_path,
                    &codec_source,
                    codec_source_id.0,
                    source::DependencyScope::Product,
                );
                lex_sources(sources)
            })?;
            let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
                parse_sources(lexed, &mut source_storage.syntax_trees)
            })?;
            source_storage.extend(parsed)?;
            generated_source_custody.push((
                codec_source_id,
                build_output::PackageGeneratedSource::from_retained_parts(
                    b"wire-codec.omg".to_vec(),
                    std::sync::Arc::from(codec_source.as_bytes()),
                ),
            ));
        }
        source_scoped_top_level_bindings.extend(
            crate::source_imports::retain_module_import_bindings(&source_storage)?,
        );
        let source_file_count = source_storage.file_count();
        let build_scope_sources = source_storage
            .sources
            .files()
            .filter(|file| file.dependency_scope == source::DependencyScope::Build)
            .map(|file| file.source_id)
            .collect();
        let syntax = AssembledSyntax {
            syntax_trees: source_storage.syntax_trees,
            sources: Arc::new(source_storage.sources),
            build_source_id: self.build_source_id,
            application: self.application,
            source_scoped_top_level_bindings,
            generated_source_custody,
            build_scope_sources,
        };
        Ok((source_file_count, syntax))
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
                &crate::source_imports::bundled_core_root()?,
            )?;
        }
        Ok(())
    }
}

impl GeneratedSourceExtension {
    /// Parse an admitted generated extension against its exact immutable base.
    /// Each custody row keeps a separate syntax forest: pre-resolution
    /// normalization must not synthesize across generated-unit boundaries.
    /// Base substitution rejects before any loading or parsing.
    pub fn parse(
        self,
        base_sources: &Arc<source::SourceMap>,
        timings: &mut CompileTimings,
    ) -> Result<(Arc<source::SourceMap>, Vec<SyntaxTrees>), Vec<Diagnostic>> {
        let (sources, custody) = self.into_source_inputs(base_sources)?;
        let mut units = Vec::with_capacity(custody.len());
        for (source_id, _) in &custody {
            let source = sources.get(*source_id).ok_or_else(|| {
                vec![Diagnostic::error(
                    "admitted generated source has no retained source instance",
                )]
            })?;
            let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
                lex_sources(load_package_generated_source(
                    source.path.clone(),
                    &source.source,
                    source_id.0,
                    source::DependencyScope::Product,
                ))
            })?;
            let mut syntax = SyntaxTrees::new(*source_id);
            timings.record(TOKENS_TO_SYNTAX_TREES, || parse_sources(lexed, &mut syntax))?;
            units.push(syntax);
        }
        Ok((sources, units))
    }
}

fn append_dependency_generated_sources_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: &PackageCompilationInputs,
    timings: &mut CompileTimings,
) -> Result<Vec<(source::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    let entries = crate::source_imports::admit_dependency_generated_sources(package_inputs)?;

    for entry in &entries {
        imports.mark_loaded(&entry.logical_path, entry.scope);
    }

    let mut retained = Vec::with_capacity(entries.len());
    for entry in entries {
        let text = entry.source_text()?;
        let crate::source_imports::DependencyGeneratedSource {
            logical_path,
            source,
            package,
            scope,
        } = entry;
        let source_id = source::SourceId(source_storage.next_source_id());
        let lexed = timings.record(SOURCE_FILES_TO_TOKENS, || {
            lex_sources(load_package_generated_source(
                logical_path,
                text,
                source_id.0,
                scope,
            ))
        })?;
        let parsed = timings.record(TOKENS_TO_SYNTAX_TREES, || {
            parse_sources(lexed, &mut source_storage.syntax_trees)
        })?;
        let contract_custody = source_storage.toolchain_contract_custody();
        let discovered = discover_imports_with_packages(
            &parsed,
            &source_storage.syntax_trees,
            package_inputs,
            Some(package),
            &mut source_storage.resolved_imports,
            &contract_custody,
        )?;
        imports.enqueue(discovered)?;
        source_storage.extend(parsed)?;
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
                    phase,
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

#[cfg(test)]
mod tests;
