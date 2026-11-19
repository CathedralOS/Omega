//! Prepare and resume Build against one package capture. Its Product roots stay deferred until settlement.

use super::{PlanningServices, refusal};

#[path = "product_queries.rs"]
mod product_queries;
use artifacts::compile_timings::{CompileTimings, counters::BuildExecutionPurpose};
use build_execution::{
    BuildEvaluationRequest, BuildStageRequest, CompileFailure, EvaluatedBuild,
    PreparedBuildEvaluation, TerminalBuildActivation, prepare_build_evaluation,
};
use diagnostics::Diagnostic;
use package_compilation::source_assembly::{
    AssembledSyntax, DeferredProductSources, ImmutableSourceParseCheckpoint,
};
use package_compilation::source_imports::{ProductQueryDependency, ProductQueryInputs};
use package_compilation::{BuildExecutionInputs, CapturedLocalSource, PackageCompilationInputs};
use review_evidence::permission_acceptance::TerminalPermissionAcceptance;
use source_compiler::{BuildStageSource, SourceContinuation, SourcePreparation};
use source_declarations::build_stage::BuildStage;
use std::{path::Path, sync::Arc};
use target::TargetProfile;

pub enum PlanningSources<'a> {
    Captured(&'a CapturedLocalSource),
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
    initial: SourceContinuation,
    query_frontier: Option<(ProductQueryInputs, SourceContinuation)>,
    pub(super) stage: BuildStage,
    pub(super) deferred: DeferredProductSources,
    pub(super) preparation: SourcePreparation,
    pub(super) timings: CompileTimings,
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
    pub(crate) preparation: SourcePreparation,
    pub(crate) timings: CompileTimings,
}

impl PlannedProduct {
    /// Restrict native and runtime requests to the reviewed Product occurrences of this lock.
    pub fn with_runtime_consents(mut self, accepted: Vec<TerminalPermissionAcceptance>) -> Self {
        self.runtime_consents = Some(accepted);
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
            source.sources.selected_files(),
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
    let (inputs, capture) = match request.sources {
        PlanningSources::Captured(capture) => (None, Some(capture)),
        PlanningSources::Package(inputs) => (Some(inputs), None),
    };
    let execution = request.execution.resolve_compilation_defaults(inputs);
    let profile = execution.profile.ok_or_else(|| {
        vec![Diagnostic::error(
            "Build execution requires an explicit profile on an uncatalogued host",
        )]
    })?;
    let mut assembled = {
        let mut catalogue = preparation.catalogue()?;
        let checkpoint = ImmutableSourceParseCheckpoint::prepare(
            request.root,
            inputs,
            capture,
            &mut timings,
            &mut catalogue,
        )?;
        checkpoint.assemble(inputs, &mut timings, &mut catalogue)?.1
    };
    let (source, stage) = resolve(&mut assembled, 0, &preparation, profile, inputs)?;
    let evaluation = prepare_build_evaluation(
        BuildEvaluationRequest {
            execution_purpose: BuildExecutionPurpose::Source,
            root_path: request.root,
            typed: source.typed(),
            build_source: assembled.build_source,
            sources: &assembled.sources,
            package_inputs: inputs,
            anonymous_source: assembled.anonymous_owner.as_ref(),
            generated_source_custody: &assembled.generated_source_custody,
            execution,
            artifact_only: request.artifact_only,
        },
        &mut timings,
    )?;
    Ok(PreparedPlanningStage {
        evaluation,
        source: PlanningSource {
            stage,
            initial: source,
            query_frontier: None,
            deferred: deferred(assembled)?,
            preparation,
            timings,
            original: inputs.cloned(),
            profile,
        },
    })
}

fn resolve(
    assembled: &mut AssembledSyntax,
    index: usize,
    preparation: &SourcePreparation,
    profile: TargetProfile,
    inputs: Option<&PackageCompilationInputs>,
) -> Result<(SourceContinuation, BuildStage), CompileFailure> {
    Ok(stage_source(assembled)?.prepare(
        index,
        preparation,
        profile.target_name(),
        inputs.map(|inputs| Arc::new(inputs.clone()) as Arc<_>),
    )?)
}

fn stage_source(assembled: &mut AssembledSyntax) -> Result<BuildStageSource, CompileFailure> {
    let entry = assembled
        .build_source
        .ok_or_else(|| vec![Diagnostic::error("project Build has no source-owned entry")])?;
    Ok(BuildStageSource {
        syntax: std::mem::take(&mut assembled.syntax_trees),
        source_items: std::mem::take(&mut assembled.source_items),
        roots: std::mem::take(&mut assembled.root_sources),
        build_sources: std::mem::take(&mut assembled.build_scope_sources),
        entry: entry.source_id(),
        sources: Arc::clone(&assembled.sources),
        imports: std::mem::take(&mut assembled.source_scoped_top_level_bindings),
    })
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
    pub(super) fn resume(
        &mut self,
        evaluation: &mut PreparedBuildEvaluation,
        previous: TerminalBuildActivation,
        prepared: &[ProductQueryDependency],
        services: &mut impl PlanningServices,
    ) -> Result<TerminalBuildActivation, CompileFailure> {
        let named = if self.original.is_none()
            && previous.requests().iter().any(|issued| {
                matches!(
                    issued.request(),
                    build_declarations::BuildRequest::Package(_)
                        | build_declarations::BuildRequest::Application(_)
                )
            }) {
            Some(self.named_source(&previous, services)?)
        } else {
            None
        };
        let inputs = ProductQueryInputs::for_build_stage(self.original.clone(), prepared.to_vec())
            .and_then(|inputs| inputs.with_named_source(named))
            .map_err(refusal)?;
        let authority = inputs.build_stage_authority_inputs().map_err(refusal)?;
        let mut assembled = self
            .deferred
            .assemble_build_stage(
                &inputs,
                &mut self.timings,
                &mut *self.preparation.catalogue()?,
            )?
            .1;
        let (source, stage) = resolve(
            &mut assembled,
            self.stage.index + 1,
            &self.preparation,
            self.profile,
            authority.as_ref(),
        )?;
        if stage.count != self.stage.count {
            return Err(vec![Diagnostic::error(
                "the captured Build changed its number of stages",
            )]
            .into());
        }
        evaluation.continue_stage(
            BuildStageRequest {
                source_acquisition: assembled.build_source_acquisition.as_ref(),
                typed: source.typed(),
                build_source: assembled.build_source,
                package_inputs: authority.as_ref(),
                anonymous_source: assembled.anonymous_owner.as_ref(),
            },
            &mut self.timings,
        )?;
        self.deferred = deferred(assembled)?;
        self.stage = stage;
        self.initial = source;
        self.query_frontier = None;
        evaluation
            .execution()
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "the next Build stage has no admitted execution",
                )]
            })?
            .resume(previous)
            .map_err(Into::into)
    }

    fn named_source(
        &self,
        activation: &TerminalBuildActivation,
        services: &mut impl PlanningServices,
    ) -> Result<PackageCompilationInputs, CompileFailure> {
        let graph = match &self.original {
            Some(original) => original.clone(),
            None => services.declared_source(activation)?.ok_or_else(|| {
                refusal("named Build requires its acquired root".into())
            })?,
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
        evaluated: EvaluatedBuild,
        inputs: Option<&PackageCompilationInputs>,
    ) -> Result<PlannedProduct, CompileFailure> {
        let source = self
            .deferred
            .assemble(
                inputs,
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
                &source.sources,
                root,
                Some(inputs.root()),
                &evaluated.configuration.generated_sources,
                &mut *self.preparation.catalogue()?,
            )?;
            let first_source = extension
                .custody()
                .first()
                .map(|(source, _)| *source)
                .ok_or_else(|| {
                    refusal("generated-source handoff has no selected source".to_owned())
                })?;
            let (sources, units) = extension.source_input(&source.sources)?;
            Some(source_compiler::SourceExtensionRequest {
                sources,
                units,
                first_source,
                selection_authority: Arc::new(inputs.clone()),
            })
        };
        Ok(PlannedProduct {
            generated,
            evaluated,
            build_profile: self.profile,
            runtime_consents: None,
            source,
            package_inputs: inputs.cloned(),
            preparation: self.preparation,
            timings: self.timings,
        })
    }
}
