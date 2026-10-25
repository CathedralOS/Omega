//! Assemble one source forest with exact package, import and generated-source custody.
//!
//! `ImmutableSourceParseCheckpoint::prepare` captures sources and parses the
//! authored Build's import closure, or the Product roots for a default Build.
//! `assemble` admits the selected scope's inputs and prelude. An authored Build
//! retains `DeferredProductSources`: after its dependency join, Product imports
//! and target entry contracts join the same captured namespace and parse prefix.
//! Product imports cannot supply names to the initial Build context. Each source
//! frontier goes through Psi's parser; no host source is recaptured at the join.
//! `GeneratedSourceExtension::parse` consumes the activation's admitted output
//! through that same facade, preserving separate generated-unit forests.
//! No Build machine executes here; resolution and typing consume the result.

use crate::PackageCompilationInputs;
use crate::source_imports::{
    GeneratedSourceExtension, ImportQueue, PackageImportPhase, PendingPackageImport,
    ProjectSourceSelection, SourceStorage, discover_imports, discover_imports_with_packages,
    discover_package_imports, validate_package_source_frontier,
};
use artifacts::compile_timings::{CompileTimings, StageMeta};
use diagnostics::Diagnostic;
use psi_compiler::{PsiCompilationObserver, PsiCompilationStage, SourceInput};
use source::{LoadedSource, SourceId, SourceOrigin};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

#[derive(Clone)]
pub struct AssembledSyntax {
    pub syntax_trees: SyntaxTrees,
    pub sources: Arc<source::SourceMap>,
    /// Anonymous ownership inputs. Initial Build leaves these unbound; the
    /// joined Product context establishes ownership before resolution/typing.
    pub anonymous_owner: Option<crate::AnonymousSourceCapture>,
    /// Product roots remain unopened until the authored Build has returned and
    /// its dependencies have joined. Both phases retain the same captured bytes.
    pub product_sources: Option<DeferredProductSources>,
    /// Exact companion `build.omg` selected during project discovery. Build
    /// authority is attached to this source, never reconstructed from a leaf
    /// filename after imports have expanded the source frontier.
    pub build_source_id: Option<source::SourceId>,
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
pub struct ImmutableSourceParseCheckpoint {
    root_path: PathBuf,
    source_storage: Arc<SourceStorage>,
    build_source_id: Option<source::SourceId>,
    package_imports: Arc<[PendingPackageImport]>,
    package_source_inputs: Option<Arc<crate::PackageCompilationSourceInputs>>,
    product_roots: Option<Arc<[PathBuf]>>,
}

/// The captured source namespace and Build parse prefix, not a Product judgment.
/// Loading Product imports consumes this continuation after the Build join.
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
    /// Load Product roots after successful Build return and dependency settlement.
    /// The current route requires the original package inputs; installing newly
    /// acquired dependency custody belongs to the pending coordinator migration.
    pub fn assemble(
        self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        let mut checkpoint = self.checkpoint;
        checkpoint.validate_child(package_inputs)?;
        let roots = checkpoint.product_roots.take().ok_or_else(|| {
            vec![Diagnostic::error(
                "Product source continuation has already been consumed",
            )]
        })?;
        let mut storage = Arc::unwrap_or_clone(checkpoint.source_storage);
        let mut imports = ImportQueue::default();
        for source in storage.sources.files() {
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
        )?);
        checkpoint.source_storage = Arc::new(storage);
        checkpoint.package_imports = package_imports.into();
        checkpoint.assemble(package_inputs, timings)
    }
}

impl ImmutableSourceParseCheckpoint {
    /// Publish page sharing on the first actual fork. A uniquely consumed
    /// checkpoint keeps its original allocations without freezing overhead.
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
            build_source_id: self.build_source_id,
            package_imports: self.package_imports.clone(),
            package_source_inputs: self.package_source_inputs.clone(),
            product_roots: self.product_roots.clone(),
        }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Prepare shared physical sources without Product imports or a child's
    /// target attachments when an authored Build exists. Target-local generated
    /// bundles join their respective scope during assembly.
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
        let mut source_storage = SourceStorage::prepare(root_path, package_inputs)?;
        let project = ProjectSourceSelection::prepare(root_path, &source_storage.filesystem)?;
        let mut imports = ImportQueue::default();
        let has_build = project
            .sources()
            .iter()
            .any(|(_, scope)| *scope == source::DependencyScope::Build);
        let mut product_roots = Vec::new();
        for (path, scope) in project.sources() {
            if has_build && *scope == source::DependencyScope::Product {
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
        )?;
        let build_source_id = project.build_source_id(&source_storage)?;
        Ok(Self {
            root_path: root_path.to_path_buf(),
            source_storage: Arc::new(source_storage),
            build_source_id,
            package_imports: package_imports.into(),
            package_source_inputs: package_inputs.map(PackageCompilationInputs::source_inputs),
            product_roots: has_build.then(|| product_roots.into()),
        })
    }

    /// Assemble the source set for a child with these package inputs: the
    /// parsed physical sources, the retained package imports and the
    /// dependency-generated sources the inputs carry. No target selects
    /// anything here; each target's package inputs were admitted for it.
    pub fn assemble(
        mut self,
        package_inputs: Option<&PackageCompilationInputs>,
        timings: &mut CompileTimings,
    ) -> Result<(usize, AssembledSyntax), Vec<Diagnostic>> {
        self.validate_child(package_inputs)?;
        let product_sources = self
            .product_roots
            .is_some()
            .then(|| DeferredProductSources {
                checkpoint: self.fork(),
            });
        let source_scope = if product_sources.is_some() {
            source::DependencyScope::Build
        } else {
            source::DependencyScope::Product
        };
        let mut source_storage = Arc::unwrap_or_clone(self.source_storage);
        let mut imports = ImportQueue::default();
        for source in source_storage.sources.files() {
            imports.mark_loaded(&source.path, source.dependency_scope);
        }
        let mut generated_source_custody = match package_inputs {
            Some(package_inputs) => append_dependency_generated_sources_to_storage(
                &mut source_storage,
                &mut imports,
                package_inputs,
                source_scope,
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
        // Core query results are ordinary source-backed data. Load their
        // canonical declarations even for an import-free query occurrence;
        // source discovery neither evaluates the query nor selects a machine.
        let core_root = crate::source_imports::bundled_core_root()?;
        let query_types = core_root.join("code_layout.omg");
        imports.seed(query_types, source_scope);
        // Primitive float spellings select ordinary source declarations and
        // their exact target satisfiers (numeric_values.md, Primitive surface).
        // Make that canonical declaration forest available without requiring
        // an authored import. Loading supplies no selection or provider
        // authority: normal operand applicability, ambiguity, checked boundary
        // applications and selected ProviderPlans still own those judgments.
        // The joined Product forest retains Build declarations too. Supply
        // implicit vocabulary for each present checked scope, not only the
        // roots being appended in this phase.
        for scope in [
            source::DependencyScope::Build,
            source::DependencyScope::Product,
        ] {
            if sources_mention_floats(&source_storage, scope) {
                imports.seed(core_root.join("float_operations.omg"), scope);
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
        // Build bindings carry inert paths. Target entry contracts and Product
        // suppliers are not part of the initial Build name-resolution context.
        let seeds = if product_sources.is_none() {
            crate::source_imports::entry_contract_seeds(package_inputs, &source_storage)?
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
            let parsed = SourceInput::Retained(
                [LoadedSource {
                    source_id: SourceId(source_storage.next_source_id()),
                    path: PathBuf::from("<build-prelude>"),
                    source: Arc::from(prelude.source.as_str()),
                    origin: Some(SourceOrigin::Toolchain),
                    scope: source::DependencyScope::Build,
                }]
                .into_iter()
                .collect(),
            )
            .append(
                &mut source_storage.syntax_trees,
                &mut ParsingTimings(timings),
            )?;
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
            let parsed = SourceInput::Retained(
                [LoadedSource {
                    source_id: codec_source_id,
                    path: codec_path,
                    source: Arc::from(codec_source.as_str()),
                    origin: Some(SourceOrigin::User),
                    scope: source::DependencyScope::Product,
                }]
                .into_iter()
                .collect(),
            )
            .append(
                &mut source_storage.syntax_trees,
                &mut ParsingTimings(timings),
            )?;
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
        // Default Build has completed its empty dependency join. Authored
        // Build retains the same source map without assigning an owner yet.
        // Supplied graph owners remain authoritative in either case.
        let capture = source_storage.retain_anonymous_owner(&generated_source_custody)?;
        let (sources, anonymous_owner) = match capture {
            Some(capture) if self.build_source_id.is_none() => {
                (capture.complete_without_dependencies()?, None)
            }
            Some(capture) => (capture.sources().clone(), Some(capture)),
            None => (Arc::new(source_storage.sources), None),
        };
        let syntax = AssembledSyntax {
            syntax_trees: source_storage.syntax_trees,
            sources,
            anonymous_owner,
            product_sources,
            build_source_id: self.build_source_id,
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
        let units = SourceInput::Retained(generated_sources.into_iter().collect())
            .into_units(&mut ParsingTimings(timings))?;
        Ok((sources, units))
    }
}

fn append_dependency_generated_sources_to_storage(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: &PackageCompilationInputs,
    source_scope: source::DependencyScope,
    timings: &mut CompileTimings,
) -> Result<Vec<(source::SourceId, build_output::PackageGeneratedSource)>, Vec<Diagnostic>> {
    let entries =
        crate::source_imports::admit_dependency_generated_sources(package_inputs, source_scope)?;

    for entry in &entries {
        imports.mark_loaded(&entry.logical_path, entry.scope);
    }

    let mut retained = Vec::with_capacity(entries.len());
    for entry in entries {
        let text = entry.source_text()?;
        let entry_base = entry.authored_base(package_inputs)?;
        let crate::source_imports::DependencyGeneratedSource {
            logical_path,
            source,
            package,
            scope,
        } = entry;
        let source_id = source::SourceId(source_storage.next_source_id());
        let parsed = SourceInput::Retained(
            [LoadedSource {
                source_id,
                path: logical_path,
                source: Arc::from(text),
                origin: Some(SourceOrigin::User),
                scope,
            }]
            .into_iter()
            .collect(),
        )
        .append(
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
        source_storage.extend_dependency_generated(parsed)?;
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
        let parsed = SourceInput::Files {
            frontier,
            first_source: source_storage.next_source_id(),
            filesystem: &source_storage.filesystem,
        }
        .append(
            &mut source_storage.syntax_trees,
            &mut ParsingTimings(timings),
        )?;
        let discovered = match package_inputs {
            Some(package_inputs) => {
                let (discovered, mut requests) = discover_package_imports(
                    &parsed,
                    &source_storage.syntax_trees,
                    package_inputs,
                    phase,
                    &mut source_storage.resolved_imports,
                    &source_storage.toolchain_contract_custody,
                )?;
                retained_requests.append(&mut requests);
                discovered
            }
            None => discover_imports(
                &parsed,
                &source_storage.syntax_trees,
                root_path,
                &source_storage.filesystem,
                &mut source_storage.resolved_imports,
            )?,
        };
        imports.enqueue(discovered)?;
        source_storage.extend(parsed)?;
    }
    Ok(retained_requests)
}

/// Observe Psi's source work using the package invocation's existing report.
struct ParsingTimings<'timings>(&'timings mut CompileTimings);

fn sources_mention_floats(
    source_storage: &crate::source_imports::SourceStorage,
    scope: source::DependencyScope,
) -> bool {
    source_storage
        .sources
        .files()
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
                        .is_some_and(|file| file.dependency_scope == scope)
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
