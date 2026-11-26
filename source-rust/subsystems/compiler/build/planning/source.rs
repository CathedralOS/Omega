//! Prepare the complete Build from its captured source and open its Product after settlement.

use super::{PlanningServices, refusal};

#[path = "build_recheck.rs"]
mod build_recheck;
#[path = "product_queries.rs"]
mod product_queries;
#[path = "successor_providers.rs"]
mod successor_providers;
use artifacts::compile_timings::{CompileTimings, counters::BuildExecutionPurpose};
use build_execution::{
    BuildEvaluationRequest, CompileFailure, EvaluatedBuild, PreparedBuildEvaluation,
    TerminalBuildActivation, prepare_build_evaluation,
};
use build_recheck::BuildRecheck;
pub use build_recheck::{
    OwedBuildRecheck, PendingProduct, rechecks_outlive_caller, with_build_rechecks,
};
use diagnostics::Diagnostic;
use package_compilation::source_assembly::{
    AssembledSyntax, DeferredProductSources, ImmutableSourceParseCheckpoint, ProductEntrySources,
};
use package_compilation::source_imports::{ProductQueryInputs, SuppliedBuildLibrary};
use package_compilation::{BuildExecutionInputs, CapturedLocalSource, PackageCompilationInputs};
use review_evidence::permission_acceptance::TerminalPermissionAcceptance;
use source_compiler::TypedBuildInterval;
use source_compiler::representations::source_inputs::SourceMap;
use source_compiler::{ProductDeclarations, SourcePreparation};
use std::{path::Path, sync::Arc};
use target::TargetProfile;

pub enum PlanningSources<'a> {
    Captured(&'a CapturedLocalSource),
    /// The receiving Build has no declaration owner yet. Each library already has its own.
    CapturedWithLibraries {
        source: &'a CapturedLocalSource,
        libraries: Vec<SuppliedBuildLibrary>,
    },
    Successor(super::PreparedSuccessor),
    Package(&'a PackageCompilationInputs),
}

pub struct PlanningRequest<'a> {
    pub root: &'a Path,
    pub sources: PlanningSources<'a>,
    pub execution: BuildExecutionInputs,
    pub artifact_only: bool,
}

/// The Build entry and the unopened Product belong to the same source checkpoint.
pub struct PreparedPlanningStage {
    pub(super) preparations: super::BuildPreparations,
    pub(super) evaluation: PreparedBuildEvaluation,
    pub(super) source: PlanningSource,
}

#[derive(Clone)]
pub(super) struct PlanningSource {
    pub(super) file_imports: source_compiler::FileImportAdvisories,
    query_authors: Option<Arc<SourceMap>>,
    query_declarations: Option<(ProductQueryInputs, ProductDeclarations)>,
    pub(super) deferred: DeferredProductSources,
    pub(super) preparation: SourcePreparation,
    pub(super) timings: CompileTimings,
    pub(super) supplied_libraries: Arc<[SuppliedBuildLibrary]>,
    original: Option<PackageCompilationInputs>,
    profile: TargetProfile,
}

/// The opened Product and the exact Build and package graph that admitted it.
pub struct PlannedProduct {
    file_imports: source_compiler::FileImportAdvisories,
    pub(super) supplied_build_libraries: Arc<[SuppliedBuildLibrary]>,
    pub(crate) evaluated: EvaluatedBuild,
    pub(crate) build_profile: TargetProfile,
    pub(super) runtime_consents: Option<Vec<TerminalPermissionAcceptance>>,
    pub(crate) source: AssembledSyntax,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
    pub(super) suppliers:
        std::collections::BTreeMap<semantic_vocabulary::PackageKeyIdentity, PlannedProduct>,
    pub(crate) preparation: SourcePreparation,
    pub(crate) timings: CompileTimings,
    /// Build work spent opening the dependency Products this one was formed from.
    dependency_timings: CompileTimings,
}

impl PlannedProduct {
    /// Restrict native and runtime requests to the reviewed Product occurrences of this lock.
    pub fn with_runtime_consents(mut self, accepted: Vec<TerminalPermissionAcceptance>) -> Self {
        self.runtime_consents = Some(accepted);
        self
    }

    /// Captured Products from the same resolver traversal as the receiving Build.
    pub fn with_component_suppliers(
        mut self,
        suppliers: std::collections::BTreeMap<semantic_vocabulary::PackageKeyIdentity, Self>,
    ) -> Self {
        self.suppliers = suppliers;
        self
    }

    pub fn evaluated(&self) -> &EvaluatedBuild {
        &self.evaluated
    }
    pub fn build_profile(&self) -> TargetProfile {
        self.build_profile
    }
    pub fn source(&self) -> &AssembledSyntax {
        &self.source
    }
    pub fn package_inputs(&self) -> Option<&PackageCompilationInputs> {
        self.package_inputs.as_ref()
    }
    pub fn build_handoff(
        &self,
        profile: TargetProfile,
    ) -> Result<package_compilation::PackageBuildHandoff, CompileFailure> {
        let inputs = self
            .package_inputs()
            .ok_or_else(|| refusal("dependency has no acquired package graph".into()))?;
        let source = self.source();
        let subject = package_compilation::derive_package_compilation_subject(
            source.parsed.files.selected_files(),
            inputs,
            &source.generated_source_custody,
        )?;
        let build = &self.evaluated.configuration;
        let handoff = package_compilation::PackageBuildHandoff::from_checked(
            inputs.root(),
            inputs.compilation_purpose(),
            profile,
            Some(self.build_profile),
            subject.dependency_closure().clone(),
            subject.source_consumption_commitment(),
            build.generated_sources.clone(),
        ).with_enabled_unsafe_atoms(build.enabled_unsafe_atoms.clone());
        if build.declared_product_selections.is_empty()
            && build.product_selection_requests.is_empty()
            && build.native_formations.is_empty()
            && build.native_binding_requests.is_empty()
        {
            return Ok(handoff);
        }
        let stage = self.product_stage(&[profile])?;
        let declarations = source_compiler::package_handoff_declarations(
            stage.source(),
            &build.native_formations,
        )?;
        let handoff = handoff
            .with_retained_provider_selections(
                declarations.providers,
                build
                    .observation_summary
                    .as_ref()
                    .map(|summary| summary.identity().digest()),
            )
            .map_err(|why| refusal(why.to_owned()))?;
        handoff
            .with_native_binding_outputs(
                declarations.native,
                declarations.intents,
                build.config.native_binding_evaluation.clone(),
            )
            .map_err(refusal)
    }

    pub fn attribute_diagnostics(&self, rows: &mut [Diagnostic]) {
        package_compilation::attribute_diagnostics(
            rows,
            &self.source.parsed.files,
            self.package_inputs(),
        );
    }

    /// Fold all prepared dependencies into the same invocation's import warnings.
    pub fn collect_file_imports(&self, uses: &mut source_compiler::FileImportAdvisories) {
        uses.extend(&self.file_imports);
        for supplier in self.suppliers.values() {
            supplier.collect_file_imports(uses);
        }
    }

    pub fn preparation(&self) -> &SourcePreparation {
        &self.preparation
    }
    pub fn timings(&self) -> &CompileTimings {
        &self.timings
    }

    /// Attribute the Build work of the dependency Products this one consumed.
    pub fn with_dependency_timings(mut self, timings: CompileTimings) -> Self {
        self.dependency_timings = timings;
        self
    }

    /// Phases spent planning and opening this Product's dependencies.
    pub fn dependency_timings(&self) -> &CompileTimings {
        &self.dependency_timings
    }
}

pub fn prepare(
    request: PlanningRequest<'_>,
    preparation: SourcePreparation,
    mut timings: CompileTimings,
) -> Result<PreparedPlanningStage, CompileFailure> {
    use package_compilation::source_assembly::CapturedBuildEntry;
    let mut entry = CapturedBuildEntry::Initial(request.root);
    let mut preparations = super::BuildPreparations::default();
    let successor_source;
    let successor_owner;
    let mut execution = request.execution;
    let (inputs, capture, supplied_libraries) = match request.sources {
        PlanningSources::Captured(capture) => (None, Some(capture), Vec::new()),
        PlanningSources::CapturedWithLibraries { source, libraries } => {
            (None, Some(source), libraries)
        }
        PlanningSources::Successor(successor) => {
            if successor.path != request.root {
                return Err(refusal(
                    "the requested entry differs from the preparing Build's selection".into(),
                ));
            }
            execution.prepared_unsafe_atoms = successor.enabled_unsafe_atoms;
            execution.interpreter_providers = successor.providers;
            successor_source = successor.capture;
            successor_owner = successor.source_owner;
            preparations = successor.preparations;
            // Only the selected package and explicitly returned runtime mounts cross this handoff.
            execution.snapshot = preparations
                .package_capture
                .take()
                .map(package_compilation::BuildSnapshotRequest::new);
            if !successor.directories.is_empty() {
                let snapshot = execution.snapshot.take().unwrap_or_else(|| package_compilation::BuildSnapshotRequest::for_activation(false));
                execution.snapshot = Some(snapshot.with_successor_directories(successor.directories));
            }
            entry = CapturedBuildEntry::Successor {
                path: request.root,
                source_owner: successor_owner.as_ref(),
                previous_build_files: &preparations.files,
            };
            (None, Some(&successor_source), successor.libraries)
        }
        PlanningSources::Package(inputs) => (Some(inputs), None, Vec::new()),
    };
    let supplied_libraries: Arc<[_]> = supplied_libraries.into();
    let mut execution = execution.resolve_compilation_defaults(inputs, capture.is_some());
    preparations.package_capture = execution
        .snapshot
        .as_ref()
        .map(|request| request.capture().clone());
    let profile = execution.profile.ok_or_else(|| {
        vec![Diagnostic::error(
            "Build execution requires an explicit profile on an uncatalogued host",
        )]
    })?;
    for library in supplied_libraries.iter() {
        library
            .require_execution_profile(profile)
            .map_err(refusal)?;
    }
    execution.admit_interpreter_providers(inputs, &supplied_libraries)?;
    let assembled = {
        let mut catalogue = preparation.catalogue()?;
        let checkpoint = ImmutableSourceParseCheckpoint::prepare(
            entry,
            inputs,
            capture,
            &mut timings,
            &mut catalogue,
        )?;
        if supplied_libraries.is_empty() {
            checkpoint
                .assemble(
                    inputs,
                    &execution.interpreter_providers,
                    &mut timings,
                    &mut catalogue,
                )?
                .1
        } else {
            checkpoint.assemble_supplied_build_libraries(
                Arc::clone(&supplied_libraries),
                &execution.interpreter_providers,
                &mut timings,
                &mut catalogue,
            )?
        }
    };
    let interval = resolve(
        &assembled,
        &preparation,
        profile,
        inputs,
        &supplied_libraries,
        execution.dependency_activation,
    )?;
    // A dependency Build checks only what its entry reaches, so typing its
    // file under a check-only owner would produce a subject nothing judges.
    // Likewise, a root Build with no boundary traits, no implementations, and no
    // non-entry machines has nothing beyond the entry machine already checked.
    let check_only_subject = if execution
        .checks_whole_subject_first(inputs, assembled.anonymous_owner.as_ref())
        && requires_provisional_subject_check(
            &interval.program,
            &assembled.build_scope_sources,
            assembled.build_source,
        ) {
        check_only_subject(&assembled, &preparation, profile, &supplied_libraries)?
    } else {
        None
    };
    let evaluation = prepare_build_evaluation(
        BuildEvaluationRequest {
            preceding_entries: preparations.stages[preparations.first_stage_in_package..].iter().map(|(_, stage)| {
                stage.configuration.executed_entry.as_ref().map(|entry| &entry.declaration).ok_or_else(|| {
                    refusal("a preparing Build lost its entered declaration".to_owned())
                })
            }).collect::<Result<_, _>>()?,
            execution_purpose: BuildExecutionPurpose::Source,
            root_path: request.root,
            interval: &interval,
            check_only_subject: check_only_subject.as_ref(),
            build_source: assembled.build_source,
            sources: &assembled.parsed.files,
            package_inputs: inputs,
            anonymous_source: assembled.anonymous_owner.as_ref(),
            generated_source_custody: &assembled.generated_source_custody,
            execution,
            source_acquisition: assembled.build_source_acquisition.as_ref(),
            artifact_only: request.artifact_only,
        },
        &mut timings,
    )
    .map_err(|failure| locate_failure(failure, &assembled.parsed.files, inputs))?;
    let mut file_imports = std::mem::take(&mut preparations.file_imports);
    file_imports.observe(&assembled.parsed, source_provenance::DependencyScope::Build);
    Ok(PreparedPlanningStage {
        preparations,
        evaluation,
        source: PlanningSource {
            file_imports,
            supplied_libraries,
            query_authors: interval.program.symbols.source_map(),
            query_declarations: None,
            deferred: deferred(assembled)?,
            preparation,
            timings,
            original: inputs.cloned(),
            profile,
        },
    })
}

/// Type the complete root Build again with its own files under a check-only
/// owner, so the whole file is checked before Build runs.
fn check_only_subject(
    assembled: &AssembledSyntax,
    preparation: &SourcePreparation,
    profile: TargetProfile,
    libraries: &[SuppliedBuildLibrary],
) -> Result<Option<TypedBuildInterval>, CompileFailure> {
    let Some(stage) = assembled.check_only_build_stage_source()? else {
        return Ok(None);
    };
    let check_owner = assembled
        .anonymous_owner
        .as_ref()
        .filter(|_| !libraries.is_empty())
        .map(|capture| capture.provisional_check_owner())
        .transpose()
        .map_err(refusal)?;
    let scope = SuppliedBuildLibrary::declaration_scope(libraries, check_owner).map(Arc::new);
    stage
        .prepare(preparation, profile.target_name(), scope)
        .map(Some)
        .map_err(|rows| locate_failure(rows.into(), &assembled.parsed.files, None))
}

fn requires_provisional_subject_check(
    typed: &source_compiler::representations::source_state::TypedTrees,
    build_sources: &std::collections::HashSet<source_text::SourceId>,
    build_source: Option<package_compilation::source_assembly::BuildSourceSelection>,
) -> bool {
    let Some(sources) = typed.symbols.source_map() else {
        return true;
    };
    let is_user_build = |span: source_text::SourceSpan| {
        build_sources.contains(&span.source_id)
            && sources
                .get(span.source_id)
                .is_some_and(|file| file.origin == source_admission::SourceOrigin::User)
    };
    if typed.traits().iter().any(|trait_def| {
        trait_def.is_boundary
            && typed
                .symbols
                .symbol_source_span(trait_def.symbol)
                .is_some_and(is_user_build)
    }) {
        return true;
    }
    if typed.conformances().iter().any(|conformance| {
        typed
            .symbols
            .symbol_source_span(conformance.symbol)
            .is_some_and(is_user_build)
    }) {
        return true;
    }
    let entry_span =
        build_source.map(package_compilation::source_assembly::BuildSourceSelection::entry);
    typed.machines().iter().any(|machine| {
        let parent = typed.symbols.get(machine.symbol).parent;
        let is_top_level = !parent.is_valid()
            || !matches!(
                typed.symbols.get(parent).kind,
                symbols::SymbolKind::Machine | symbols::SymbolKind::State
            );
        is_top_level
            && typed
                .symbols
                .symbol_source_span(machine.symbol)
                .is_some_and(|span| is_user_build(span) && Some(span) != entry_span)
    })
}

fn resolve(
    assembled: &AssembledSyntax,
    preparation: &SourcePreparation,
    profile: TargetProfile,
    inputs: Option<&PackageCompilationInputs>,
    libraries: &[SuppliedBuildLibrary],
    entry_only: bool,
) -> Result<TypedBuildInterval, CompileFailure> {
    let scope = inputs
        .map(|inputs| inputs.declaration_scope(build_declarations::DependencyPurpose::Build))
        .or_else(|| SuppliedBuildLibrary::declaration_scope(libraries, assembled.build_owner()))
        .map(Arc::new);
    let stage = assembled.build_stage_source()?;
    let prepared = if entry_only {
        stage.prepare_entry_only(preparation, profile.target_name(), scope)
    } else {
        stage.prepare(preparation, profile.target_name(), scope)
    };
    prepared.map_err(|rows| locate_failure(rows.into(), &assembled.parsed.files, inputs))
}

fn locate_failure(
    mut failure: CompileFailure,
    sources: &SourceMap,
    inputs: Option<&PackageCompilationInputs>,
) -> CompileFailure {
    match &mut failure {
        CompileFailure::Diagnostics(rows)
        | CompileFailure::SourceCompletion {
            diagnostics: rows, ..
        } => {
            package_compilation::attribute_diagnostics(rows, sources, inputs);
        }
        CompileFailure::BuildConsentRequired { .. } => {}
    }
    failure
}

fn deferred(assembled: AssembledSyntax) -> Result<DeferredProductSources, CompileFailure> {
    assembled.product_sources.ok_or_else(|| {
        vec![Diagnostic::error(
            "project Build lost the Product roots deferred by its package capture",
        )]
        .into()
    })
}

impl PlanningSource {
    pub(super) fn retain_build_files(&self, root: &Path, previous: &mut Vec<std::path::PathBuf>) {
        if let Some(authors) = &self.query_authors {
            for file in authors.selected_files().filter(|file| {
                file.package_root == root
                    && file.dependency_scope == source_provenance::DependencyScope::Build
            }) {
                if !previous.contains(&file.path) {
                    previous.push(file.path.clone());
                }
            }
        }
    }

    pub(super) fn provide(
        &self,
        owner: semantic_vocabulary::PackageKeyIdentity,
        declaration: &str,
        services: &mut impl PlanningServices,
    ) -> Result<build_execution::ProducedBuildProvider, CompileFailure> {
        let mut available = self
            .supplied_libraries
            .iter()
            .filter_map(|library| library.provider_handoff(owner))
            .chain(self.original.iter().flat_map(|inputs| {
                inputs
                    .dependency_build_handoff_instances()
                    .filter(move |(scope, handoff)| {
                        *scope == build_declarations::DependencyPurpose::Build
                            && handoff.package() == owner
                    })
                    .map(|(_, handoff)| handoff)
            }));
        let selected = available.next().ok_or_else(|| {
            refusal(
                "Build provider requires an implementation from an invoker-supplied library".into(),
            )
        })?;
        if available.any(|other| other != selected) {
            return Err(refusal(
                "Build libraries disagree on the source of the selected provider".into(),
            ));
        }
        let product = services.provider_product(selected)?;
        if product.build_handoff(self.profile)? != *selected {
            return Err(refusal(
                "Build provider Product differs from the supplied library handoff".into(),
            ));
        }
        product
            .product_stage(&[self.profile])?
            .produce_build_provider(self.profile, declaration)
            .map_err(Into::into)
    }

    fn named_source(
        &self,
        activation: &TerminalBuildActivation,
        services: &mut impl PlanningServices,
    ) -> Result<PackageCompilationInputs, CompileFailure> {
        let graph = match &self.original {
            Some(original) => original.clone(),
            None => services
                .declared_source(activation)?
                .ok_or_else(|| refusal("named Build requires its acquired root".into()))?,
        };
        let declaration = activation.project_declaration().map_err(refusal)?;
        let name = match &declaration {
            build_declarations::BuildDeclaration::Package(package) => &package.name,
            build_declarations::BuildDeclaration::Application(application) => &application.name,
            build_declarations::BuildDeclaration::Workspace(_) => {
                return Err(refusal("workspace has no named Product root".into()));
            }
        };
        if graph.package_name(graph.root()) != Some(name.as_str())
            || graph.root_role() != declaration.kind()
        {
            return Err(refusal(
                "acquired source differs from the executed Build declaration".into(),
            ));
        }
        Ok(graph)
    }

    /// Open the Product and check its returned Build again on the Product's
    /// sources before returning it.
    pub(super) fn open_product(
        self,
        evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PlannedProduct, CompileFailure> {
        let product = self.open_unchecked(evaluated, inputs, false)?;
        match BuildRecheck::owed_by(&product) {
            None => Ok(product),
            Some((recheck, owed)) => {
                let checked = recheck.run();
                product.settle_recheck(Some(owed), Some(checked))
            }
        }
    }

    /// Open the Product and start checking its returned Build again on the
    /// Product's sources on an idle worker. The Product is released only by
    /// [`PendingProduct::settle`], after that check passes.
    pub(super) fn open_pending(
        self,
        evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PendingProduct, CompileFailure> {
        self.open_pending_scoped(evaluated, inputs, false)
    }

    /// Open only the entry, operand, and imported files needed to form a
    /// dependency Build handoff without seeding every unimported module of the
    /// dependency package.
    pub(super) fn open_pending_for_handoff(
        self,
        evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PendingProduct, CompileFailure> {
        self.open_pending_scoped(evaluated, inputs, true)
    }

    fn open_pending_scoped(
        self,
        evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
        exclude_product_subject: bool,
    ) -> Result<PendingProduct, CompileFailure> {
        let product = self.open_unchecked(evaluated, inputs, exclude_product_subject)?;
        let owed = BuildRecheck::owed_by(&product).map(|(recheck, owed)| (owed, recheck.start()));
        Ok(PendingProduct::new(product, owed))
    }

    fn open_unchecked(
        mut self,
        evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
        exclude_product_subject: bool,
    ) -> Result<PlannedProduct, CompileFailure> {
        for library in self.supplied_libraries.iter() {
            let joined = inputs.ok_or_else(|| {
                refusal("a supplied Build library has no joined Product inputs".into())
            })?;
            library.validate_handoff(joined).map_err(refusal)?;
        }
        let source = self
            .deferred
            .assemble(
                inputs,
                ProductEntrySources {
                    bindings: &evaluated.configuration.root_binding_requests,
                    has_registered_tests: evaluated
                        .configuration
                        .product_selection_requests
                        .iter()
                        .any(|event| event.test_group.is_some()),
                    exclude_product_subject,
                },
                &evaluated.configuration.generated_sources,
                &evaluated.configuration.declared_product_selections,
                &evaluated.interpreter_providers,
                &mut self.timings,
                &mut *self.preparation.catalogue()?,
            )
            .map_err(|diagnostics| {
                CompileFailure::source_completion(
                    diagnostics,
                    evaluated.source_completion_accounting.clone(),
                )
            })?
            .1;
        self.file_imports
            .observe(&source.parsed, source_provenance::DependencyScope::Product);
        Ok(PlannedProduct {
            file_imports: self.file_imports,
            supplied_build_libraries: self.supplied_libraries,
            evaluated,
            build_profile: self.profile,
            runtime_consents: None,
            source,
            package_inputs: inputs.cloned(),
            suppliers: Default::default(),
            preparation: self.preparation,
            timings: self.timings,
            dependency_timings: CompileTimings::default(),
        })
    }
}
