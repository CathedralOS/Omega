//! Assemble one source forest with exact package, import and generated-source custody.
//!
//! `ImmutableSourceParseCheckpoint::prepare` discovers and parses immutable
//! physical sources. `assemble` admits a child's inputs, appends dependency
//! outputs, resolves deferred imports, and injects entry contracts and the Build
//! prelude. Both discover ordered frontiers; the Psi facade owns their parsing.
//! `GeneratedSourceExtension::parse` consumes the activation's admitted output
//! through that same facade, preserving separate generated-unit forests.
//! No Build machine executes here; resolution and typing consume the result.

use crate::PackageCompilationInputs;
use crate::source_imports::{
    GeneratedSourceExtension, ImportQueue, PackageImportPhase, PendingPackageImport,
    ProjectSourceSelection, SourceStorage, discover_imports, discover_imports_with_packages,
    discover_package_imports, validate_package_source_frontier, validate_selected_build_role,
};
use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use diagnostics::Diagnostic;
use psi_compiler::{PsiCompilationObserver, PsiCompilationStage, SourceInput};
use source::{LoadedSource, SourceId, SourceOrigin};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::SyntaxTrees;

/// Exact companion Build source chosen by project discovery and retained by
/// source assembly. No consumer can substitute a source id or recreate this
/// association from a filename after imports extend the frontier.
#[derive(Clone)]
pub struct SelectedBuildSource {
    source_id: source::SourceId,
    owner: Arc<source::SourceMap>,
    application: Option<build_declarations::ApplicationDeclaration>,
}

impl SelectedBuildSource {
    pub fn application(&self) -> Option<&build_declarations::ApplicationDeclaration> {
        self.application.as_ref()
    }

    pub(crate) fn source_id(&self) -> source::SourceId {
        self.source_id
    }

    pub(crate) fn owner(&self) -> &Arc<source::SourceMap> {
        &self.owner
    }
}

#[derive(Clone)]
pub struct AssembledSyntax {
    pub syntax_trees: SyntaxTrees,
    pub sources: Arc<source::SourceMap>,
    /// Exact companion `build.omg` selected during project discovery. Build
    /// authority is attached to this source, never reconstructed from a leaf
    /// filename after imports have expanded the source frontier.
    pub selected_build_source: Option<SelectedBuildSource>,
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

/// Immutable captured source and parse frontier formed before exact target
/// generated sources or target-scoped imports join compilation.
#[derive(Clone)]
pub struct ImmutableSourceParseCheckpoint {
    /// The original request's absolute routing coordinate. Capture may resolve
    /// its parent to a different selected root; both belong to this one owner.
    requested_root_path: PathBuf,
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
    /// Original unmanaged input capture, produced before any parser or Build
    /// execution. The same filesystem owner is retained in assembled sources.
    pub fn unmanaged_source_capture(
        &self,
    ) -> Option<&crate::source_imports::UnmanagedSourceCapture> {
        self.source_storage.unmanaged_source_capture()
    }

    /// Original compiler-supplied source input shared by prepared children.
    /// The selected injection consumes this exact owner. It is one input of
    /// anonymous provenance, not a completed language/default semantic record.
    pub fn original_build_vocabulary(&self) -> Option<&Arc<str>> {
        self.source_storage.original_build_vocabulary()
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Admit only coordinates recorded by original source preparation. Do not
    /// reopen a host alias to decide whether a child owns this captured input.
    pub fn admit_root(&self, root_path: &Path) -> Result<(), Vec<Diagnostic>> {
        let requested = absolute_root_coordinate(root_path)?;
        if requested != self.requested_root_path && requested != self.root_path {
            return Err(vec![Diagnostic::error(
                "checked child compilation root does not match its prepared source checkpoint",
            )]);
        }
        Ok(())
    }

    /// Prepare shared source inputs without admitting a child's target
    /// attachments. Target-local generated bundles join later during assembly.
    pub fn prepare_from_sources(
        root_path: &Path,
        package_sources: Option<Arc<crate::PackageCompilationSourceInputs>>,
        build_directory: Option<&Path>,
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
        Self::prepare(root_path, package_inputs.as_ref(), build_directory, timings)
    }

    pub fn prepare(
        root_path: &Path,
        package_inputs: Option<&PackageCompilationInputs>,
        build_directory: Option<&Path>,
        timings: &mut CompileTimings,
    ) -> Result<Self, Vec<Diagnostic>> {
        let requested_root_path = absolute_root_coordinate(root_path)?;
        let mut source_storage =
            SourceStorage::prepare(root_path, package_inputs, build_directory)?;
        let root_path = source_storage
            .unmanaged_source_capture()
            .map(|capture| capture.selected_root().to_path_buf())
            .unwrap_or_else(|| requested_root_path.clone());
        let project = ProjectSourceSelection::prepare(&root_path, &source_storage.filesystem)?;
        let mut imports = ImportQueue::default();
        for (path, scope) in project.sources() {
            imports.seed(path.clone(), *scope);
        }
        let package_imports = load_imports(
            &mut source_storage,
            &mut imports,
            package_inputs,
            PackageImportPhase::TargetIndependent,
            timings,
        )?;
        let build_source_id = project.build_source_id(&source_storage)?;
        let application = validate_selected_build_role(&source_storage, build_source_id)?;
        Ok(Self {
            requested_root_path,
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
                package_inputs,
                PackageImportPhase::ExactTarget(None),
                timings,
            )?;
        }
        let mut source_scoped_top_level_bindings = Vec::new();
        if let Some(prelude) =
            crate::source_imports::prepare_build_prelude(&source_storage, self.build_source_id)?
        {
            let parsed = SourceInput::Retained(
                [LoadedSource {
                    source_id: SourceId(source_storage.next_source_id()),
                    path: PathBuf::from("<build-prelude>"),
                    source: prelude.source,
                    origin: Some(SourceOrigin::Toolchain),
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
        let sources = Arc::new(source_storage.sources);
        let selected_build_source = self.build_source_id.map(|source_id| SelectedBuildSource {
            source_id,
            owner: Arc::clone(&sources),
            application: self.application.clone(),
        });
        let syntax = AssembledSyntax {
            syntax_trees: source_storage.syntax_trees,
            sources,
            selected_build_source,
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

/// Anchor a request without folding parent traversal through unknown aliases.
/// The capture's selected coordinate is retained separately; host paths are
/// never resolved again when a child joins this original checkpoint.
fn absolute_root_coordinate(root_path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    if root_path.is_absolute() {
        return Ok(root_path.to_path_buf());
    }
    std::env::current_dir()
        .map(|directory| directory.join(root_path))
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "could not anchor compilation source root {}: {error}",
                root_path.display()
            ))]
        })
}

fn load_imports(
    source_storage: &mut SourceStorage,
    imports: &mut ImportQueue,
    package_inputs: Option<&PackageCompilationInputs>,
    phase: PackageImportPhase,
    timings: &mut CompileTimings,
) -> Result<Vec<PendingPackageImport>, Vec<Diagnostic>> {
    let mut retained_requests = Vec::new();
    let default_source_root = source_storage.default_source_root().to_path_buf();
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
                &default_source_root,
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

impl PsiCompilationObserver for ParsingTimings<'_> {
    fn record_result<Value, Error>(
        &mut self,
        stage: PsiCompilationStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        let (name, input, output) = stage.observation_labels();
        self.0.record_result(
            StageMeta::new(name, input, output, TimingCategory::Pipeline),
            operation,
        )
    }
}
