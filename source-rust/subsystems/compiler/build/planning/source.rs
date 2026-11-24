//! Prepare the complete Build from its captured source and open its Product after settlement.

use super::{PlanningServices, refusal};

#[path = "product_queries.rs"]
mod product_queries;
use artifacts::compile_timings::{CompileTimings, counters::BuildExecutionPurpose};
use build_execution::{
    BuildEvaluationRequest, BuildSubjectValidation, CompileFailure, EvaluatedBuild,
    PreparedBuildEvaluation, TerminalBuildActivation, prepare_build_evaluation,
    settle_rejoined_build_slice,
};
use diagnostics::Diagnostic;
use package_compilation::source_assembly::{
    AssembledSyntax, DeferredProductSources, ImmutableSourceParseCheckpoint,
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
    pub(super) evaluation: PreparedBuildEvaluation,
    pub(super) source: PlanningSource,
}

#[derive(Clone)]
pub(super) struct PlanningSource {
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
    pub(crate) evaluated: EvaluatedBuild,
    pub(crate) build_profile: TargetProfile,
    pub(super) runtime_consents: Option<Vec<TerminalPermissionAcceptance>>,
    pub(crate) source: AssembledSyntax,
    pub(super) generated: Option<source_compiler::SourceExtensionRequest>,
    pub(crate) package_inputs: Option<PackageCompilationInputs>,
    pub(super) suppliers:
        std::collections::BTreeMap<semantic_vocabulary::PackageKeyIdentity, PlannedProduct>,
    pub(crate) preparation: SourcePreparation,
    pub(crate) timings: CompileTimings,
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
        );
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
        let sources = self
            .generated
            .as_ref()
            .map_or(&self.source.parsed.files, |extension| &extension.sources);
        package_compilation::attribute_diagnostics(rows, sources, self.package_inputs());
    }

    pub fn preparation(&self) -> &SourcePreparation {
        &self.preparation
    }
    pub fn timings(&self) -> &CompileTimings {
        &self.timings
    }
}

pub fn prepare(
    request: PlanningRequest<'_>,
    preparation: SourcePreparation,
    mut timings: CompileTimings,
) -> Result<PreparedPlanningStage, CompileFailure> {
    let (inputs, capture, supplied_libraries) = match request.sources {
        PlanningSources::Captured(capture) => (None, Some(capture), Vec::new()),
        PlanningSources::CapturedWithLibraries { source, libraries } => {
            (None, Some(source), libraries)
        }
        PlanningSources::Package(inputs) => (Some(inputs), None, Vec::new()),
    };
    let supplied_libraries: Arc<[_]> = supplied_libraries.into();
    let execution = request.execution.resolve_compilation_defaults(inputs);
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
    let assembled = {
        let mut catalogue = preparation.catalogue()?;
        let checkpoint = ImmutableSourceParseCheckpoint::prepare(
            request.root,
            inputs,
            capture,
            &mut timings,
            &mut catalogue,
        )?;
        if supplied_libraries.is_empty() {
            checkpoint.assemble(inputs, &mut timings, &mut catalogue)?.1
        } else {
            checkpoint.assemble_supplied_build_libraries(
                Arc::clone(&supplied_libraries),
                &mut timings,
                &mut catalogue,
            )?
        }
    };
    let interval = resolve(&assembled, &preparation, profile, inputs)?;
    let check_only_subject = check_only_subject(&assembled, &preparation, profile)?;
    let evaluation = prepare_build_evaluation(
        BuildEvaluationRequest {
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
    .map_err(|failure| locate_failure(failure, &assembled, inputs))?;
    Ok(PreparedPlanningStage {
        evaluation,
        source: PlanningSource {
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
) -> Result<Option<TypedBuildInterval>, CompileFailure> {
    let Some(stage) = assembled.check_only_build_stage_source()? else {
        return Ok(None);
    };
    stage
        .prepare(preparation, profile.target_name(), None)
        .map(Some)
        .map_err(|rows| locate_failure(rows.into(), assembled, None))
}

fn resolve(
    assembled: &AssembledSyntax,
    preparation: &SourcePreparation,
    profile: TargetProfile,
    inputs: Option<&PackageCompilationInputs>,
) -> Result<TypedBuildInterval, CompileFailure> {
    assembled
        .build_stage_source()?
        .prepare(
            preparation,
            profile.target_name(),
            inputs.map(|inputs| {
                Arc::new(inputs.declaration_scope(build_declarations::DependencyPurpose::Build))
            }),
        )
        .map_err(|rows| locate_failure(rows.into(), assembled, inputs))
}

fn locate_failure(
    mut failure: CompileFailure,
    source: &AssembledSyntax,
    inputs: Option<&PackageCompilationInputs>,
) -> CompileFailure {
    match &mut failure {
        CompileFailure::Diagnostics(rows)
        | CompileFailure::SourceCompletion {
            diagnostics: rows, ..
        } => {
            package_compilation::attribute_diagnostics(rows, &source.parsed.files, inputs);
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

    pub(super) fn open_product(
        mut self,
        mut evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
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
                &evaluated.configuration.root_binding_requests,
                &evaluated.configuration.declared_product_selections,
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
        let owes_real_owner_check =
            evaluated.subject_validation == BuildSubjectValidation::CheckedUnderProvisionalOwner;
        if owes_real_owner_check
            || !evaluated.constants.read.is_empty()
            || evaluated.constants.folded_under_providers
        {
            let authority = inputs.map(|inputs| {
                Arc::new(inputs.declaration_scope(build_declarations::DependencyPurpose::Build))
            });
            let declaration = evaluated
                .configuration
                .selected_build_declaration()
                .ok_or_else(|| refusal("rejoined Build source lost its issued entry".into()))?;
            // Checking the rejoined subject can refuse any declaration in the
            // Product's files, so its refusals name their file, line and column.
            let checked = (|| {
                let (typed, entry) = source.build_stage_source()?.rejoin_build_entry(
                    &self.preparation,
                    self.profile.target_name(),
                    authority,
                    inputs.map(|inputs| inputs.root()),
                    declaration,
                )?;
                settle_rejoined_build_slice(
                    &evaluated.constants,
                    &typed,
                    entry,
                    inputs,
                    source.anonymous_owner.as_ref(),
                    evaluated.subject_validation,
                    &mut self.timings,
                )
            })();
            let spent = checked.map_err(|failure| locate_failure(failure, &source, inputs))?;
            evaluated.source_completion_accounting.append(spent);
            if owes_real_owner_check {
                evaluated.subject_validation = BuildSubjectValidation::Checked;
            }
        }
        let generated = if evaluated.configuration.generated_sources.is_empty() {
            None
        } else {
            let inputs = inputs.ok_or_else(|| {
                refusal(
                    "generated-source final compilation requires package-aware source custody"
                        .to_owned(),
                )
            })?;
            let root = inputs.package_root(inputs.root()).ok_or_else(|| {
                refusal("generated-source handoff has no acquired package root".to_owned())
            })?;
            let extension = package_compilation::source_imports::admit_generated_extension(
                &source.parsed.files,
                root,
                Some(inputs.root()),
                &evaluated.configuration.generated_sources,
                &mut *self.preparation.catalogue()?,
            )?;
            Some(extension.prepare_source_request(&source.parsed.files, inputs, &mut self.timings)?)
        };
        Ok(PlannedProduct {
            generated,
            evaluated,
            build_profile: self.profile,
            runtime_consents: None,
            source,
            package_inputs: inputs.cloned(),
            suppliers: Default::default(),
            preparation: self.preparation,
            timings: self.timings,
        })
    }
}
