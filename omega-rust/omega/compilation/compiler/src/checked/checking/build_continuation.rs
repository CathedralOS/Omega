/*
We admit and prepare Build invocations while retaining pending Product
implementations (`BuildSourceInvocation`) and completing the Product frontend
once Build settles (`complete_product_frontend`).

Dependency preparation needs Build's result and declaration descriptions, not an
executable whole-package Product. When an actual Product demand arrives, we
consume its retained Build once, complete the authored module closure, and parse
and type any generated source extension without replaying Build effects.
*/

use artifacts::compile_timings::CompileTimings;
use diagnostics::Diagnostic;
use package_compilation::BuildSourceCustody;
use package_compilation::PackageCompilationInputs;
use products::{
    BuildAdmission, BuiltTypedProgram, CheckedExecutionRequest, CompileFailure,
    TypedCompilationFrontier,
};
use std::path::Path;
use std::sync::Arc;

pub(super) struct BuildSourceInvocation {
    /// The build dependencies the stages of this Build have prepared, as its
    /// current stage has them mounted.
    prepared: Vec<package_compilation::source_imports::ProductQueryDependency>,
    execution_contract: package_compilation::build_execution::BuildExecutionContract,
    parsed: products::PreparedCheckedSource,
    frontier: TypedCompilationFrontier,
    evaluation: build_settings::PreparedBuildEvaluation,
    package_inputs: Option<PackageCompilationInputs>,
    selected_target_profile: Option<target::TargetProfile>,
    build_execution_profile: Option<target::TargetProfile>,
    dependency_activation: bool,
    timings: CompileTimings,
    started: std::time::Instant,
}

pub(super) fn prepare_build_invocation(
    root_path: &Path,
    child: CheckedExecutionRequest<'_>,
    parsed: products::PreparedCheckedSource,
    frontier: TypedCompilationFrontier,
    mut timings: CompileTimings,
    pure_build_compilation: Option<&build_settings::PureBuildCompilation>,
) -> Result<BuildSourceInvocation, CompileFailure> {
    let started = std::time::Instant::now();
    let CheckedExecutionRequest {
        selected_target_profile,
        package_inputs,
        build,
        optimization_rollback: _,
    } = child;
    let build_execution_profile = build.profile;
    let dependency_activation = build.dependency_activation;
    let execution_contract = build.contract();
    let TypedCompilationFrontier {
        preparation,
        build_stage,
        mut source,
        anonymous_source,
        product_sources,
        build_source,
        source_file_count,
        generated_source_custody,
        base_sources,
    } = frontier;
    let build_input = source.build_input();
    let evaluation = build_settings::prepare_build_evaluation(
        build_settings::BuildEvaluationRequest {
            execution_purpose: artifacts::compile_timings::counters::BuildExecutionPurpose::Source,
            pure_build_compilation,
            root_path,
            typed: build_input.typed,
            pre_checks: build_input.pre_checks,
            build_source,
            sources: &base_sources,
            package_inputs,
            anonymous_source: anonymous_source.as_ref(),
            generated_source_custody: &generated_source_custody,
            execution: build,
            artifact_only: false,
        },
        &mut timings,
    )?;
    Ok(BuildSourceInvocation {
        prepared: Vec::new(),
        execution_contract,
        parsed,
        frontier: TypedCompilationFrontier {
            preparation,
            build_stage,
            source,
            anonymous_source,
            product_sources,
            build_source,
            source_file_count,
            generated_source_custody,
            base_sources,
        },
        evaluation,
        package_inputs: package_inputs.cloned(),
        selected_target_profile,
        build_execution_profile,
        dependency_activation,
        timings,
        started,
    })
}

impl BuildSourceInvocation {
    pub(super) fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        self.evaluation.source_completion_accounting()
    }

    pub(super) fn product_queries(&self) -> super::ProductQueries {
        super::ProductQueries::new(
            &self.frontier,
            self.package_inputs.clone(),
            self.build_execution_profile,
            self.prepared.clone(),
        )
    }
    pub(super) fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut build_settings::ReturnedBuild>,
    ) -> Result<build_settings::BuildSourceResult<'result>, CompileFailure> {
        self.evaluation.source_result(returned)
    }
    pub(super) fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.evaluation.execution()
    }

    /// Whether the stage this invocation holds ends at a dependency
    /// preparation with statements of the Build entry still to check.
    pub(super) fn has_later_stage(&self) -> bool {
        !self.frontier.build_stage.is_last()
    }

    /// Resolves, types, judges and admits the next stage of this Build
    /// (`wiki/spec/build/scoped_execution.md`, "Checked Build stages"), with
    /// the build dependencies its earlier stages prepared selectable under
    /// their aliases. Nothing of the stage runs here, and nothing of an
    /// earlier stage is checked or run again: "Preparation extends the
    /// authorized environment of the subsequent continuation, not the
    /// meaning of earlier code."
    ///
    /// The stage takes the ordinary route: the Build sources are assembled
    /// again with the prepared packages mounted, the entry is cut to the
    /// stage's statements, and the result is typed and handed to the same
    /// Build admission the first stage went through. This invocation then
    /// holds that stage: its frontier, its deferred Product continuation and
    /// the execution that runs next.
    pub(super) fn prepare_next_stage(
        &mut self,
        prepared: Vec<package_compilation::source_imports::ProductQueryDependency>,
    ) -> Result<(), CompileFailure> {
        let spent = self.evaluation.source_completion_accounting().clone();
        self.prepare_stage(prepared)
            .map_err(|failure| failure.with_source_completion_accounting(spent))
    }

    fn prepare_stage(
        &mut self,
        prepared: Vec<package_compilation::source_imports::ProductQueryDependency>,
    ) -> Result<(), CompileFailure> {
        let stage = self.frontier.build_stage;
        if stage.is_last() {
            return Err(vec![Diagnostic::error(
                "Build entry has no stage after the one that returned",
            )]
            .into());
        }
        let pending = self.frontier.product_sources.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "a staged Build requires its deferred Product context",
            )]
        })?;
        let inputs = package_compilation::source_imports::ProductQueryInputs::for_build_stage(
            self.package_inputs.clone(),
            prepared.clone(),
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let preparation = self.frontier.preparation.clone();
        let (count, syntax) = pending.sources().assemble_build_stage(
            &inputs,
            &mut self.timings,
            &mut *preparation.catalogue()?,
        )?;
        let diagnostic_sources = syntax.sources.clone();
        let package_inputs = self.package_inputs.as_ref();
        let frontier = lower_typed_frontier(
            preparation,
            count,
            syntax,
            self.build_execution_profile,
            package_inputs,
            &mut self.timings,
            Some(stage.index + 1),
        )
        .map_err(|mut diagnostics| {
            super::attribute_diagnostics(&mut diagnostics, &diagnostic_sources, package_inputs);
            diagnostics
        })?;
        let TypedCompilationFrontier {
            preparation,
            build_stage,
            mut source,
            anonymous_source,
            product_sources,
            build_source,
            source_file_count,
            generated_source_custody,
            base_sources,
        } = frontier;
        let build_input = source.build_input();
        self.evaluation
            .continue_stage(
                build_settings::BuildStageRequest {
                    typed: build_input.typed,
                    build_source,
                    package_inputs,
                    anonymous_source: anonymous_source.as_ref(),
                },
                &mut self.timings,
            )
            .map_err(|failure| super::attribute_failure(failure, &base_sources, package_inputs))?;
        self.prepared = prepared;
        self.frontier = TypedCompilationFrontier {
            preparation,
            build_stage,
            source,
            anonymous_source,
            product_sources,
            build_source,
            source_file_count,
            generated_source_custody,
            base_sources,
        };
        Ok(())
    }

    pub(super) fn complete(
        self,
        returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<products::PreparedBuild, CompileFailure> {
        let settled = self.settle(returned, joined_inputs, product_target)?;
        let inputs = settled.package_inputs.clone();
        prepare_settled_product(settled, inputs.as_ref())
    }

    pub(super) fn settle(
        self,
        returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<products::SettledBuildSource, CompileFailure> {
        let spent = self.evaluation.source_completion_accounting().clone();
        self.settle_source(returned, joined_inputs, product_target)
            .map_err(|failure| failure.with_source_completion_accounting(spent))
    }

    fn settle_source(
        self,
        mut returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<products::SettledBuildSource, CompileFailure> {
        let Self {
            prepared: _,
            execution_contract,
            parsed,
            frontier,
            evaluation,
            package_inputs,
            selected_target_profile,
            build_execution_profile,
            dependency_activation,
            timings,
            started,
        } = self;
        let original_assembly = package_compilation::SourceAssemblyInputs::new(
            package_inputs.as_ref(),
            build_execution_profile,
        );
        if let (Some(original), Some(joined)) = (package_inputs.as_ref(), joined_inputs)
            && original != joined
        {
            return Err(vec![Diagnostic::error(
                "Build completion cannot replace its original package inputs",
            )]
            .into());
        }
        let initially_unbound = package_inputs.is_none();
        if let (Some(inputs), Some(returned)) = (package_inputs.as_ref(), returned.as_mut()) {
            returned
                .join_preacquired_sources(inputs)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
        let selected_target_profile = if initially_unbound {
            product_target.or(selected_target_profile)
        } else {
            selected_target_profile
        };
        let package_inputs = joined_inputs.or(package_inputs.as_ref());
        if initially_unbound && let Some(inputs) = package_inputs {
            inputs.validate_execution_context(selected_target_profile, build_execution_profile)?;
        }
        let initial_sources = frontier.base_sources.clone();
        evaluation.validate_source_join(returned.as_ref(), package_inputs, initially_unbound)?;
        let evaluated = evaluation.complete(returned).map_err(|failure| {
            super::attribute_failure(failure, &initial_sources, package_inputs)
        })?;
        if initially_unbound && let Some(inputs) = package_inputs {
            if frontier.product_sources.is_none() {
                return Err(vec![Diagnostic::error(
                    "unbound package completion requires a deferred Product context",
                )]
                .into());
            }
            let declaration = evaluated.configuration.project_declaration.as_ref();
            let name = match declaration {
                Some(build_declarations::BuildDeclaration::Package(package)) => {
                    Some(package.name.as_str())
                }
                Some(build_declarations::BuildDeclaration::Application(application)) => {
                    Some(application.name.as_str())
                }
                _ => None,
            };
            if name.is_none()
                || name != inputs.package_name(inputs.root())
                || declaration.map(build_declarations::BuildDeclaration::kind)
                    != Some(inputs.root_role())
            {
                return Err(vec![Diagnostic::error(
                    "joined package identity does not match the executed Build declaration",
                )]
                .into());
            }
        }
        Ok(products::SettledBuildSource {
            execution_contract,
            parsed,
            frontier,
            evaluated,
            package_inputs: package_inputs.cloned(),
            original_assembly,
            build_execution_profile,
            dependency_activation,
            timings,
            build_elapsed_micros: started.elapsed().as_micros(),
        })
    }
}

pub(super) fn prepare_settled_product(
    settled: products::SettledBuildSource,
    inputs: Option<&PackageCompilationInputs>,
) -> Result<products::PreparedBuild, CompileFailure> {
    let spent = settled.evaluated.source_completion_accounting.clone();
    prepare_settled_source_product(settled, inputs)
        .map_err(|failure| failure.with_source_completion_accounting(spent))
}

fn prepare_settled_source_product(
    settled: products::SettledBuildSource,
    inputs: Option<&PackageCompilationInputs>,
) -> Result<products::PreparedBuild, CompileFailure> {
    if settled
        .package_inputs
        .as_ref()
        .map(PackageCompilationInputs::source_inputs)
        != inputs.map(PackageCompilationInputs::source_inputs)
    {
        return Err(vec![Diagnostic::error(
            "Product preparation cannot replace its settled Build source graph",
        )]
        .into());
    }
    let started = std::time::Instant::now();
    let mut timings = settled.timings;
    let inputs_joined_after_build = settled.original_assembly
        != package_compilation::SourceAssemblyInputs::new(inputs, settled.build_execution_profile);
    let frontier = if settled.frontier.product_sources.is_some() || !inputs_joined_after_build {
        settled.frontier
    } else {
        let (frontier, prepared_timings) =
            super::prepare_typed_frontier(settled.parsed, inputs, settled.build_execution_profile)?;
        timings = prepared_timings;
        frontier
    };
    let initial_sources = frontier.base_sources.clone();
    let completed = complete_build_source(
        frontier,
        settled.evaluated,
        inputs,
        settled.build_execution_profile,
        settled.dependency_activation,
        inputs_joined_after_build,
        &mut timings,
    )
    .map_err(|failure| super::attribute_failure(failure, &initial_sources, inputs))?;
    timings.add_completed(
        artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
        settled
            .build_elapsed_micros
            .saturating_add(started.elapsed().as_micros()),
    );
    Ok(products::PreparedBuild {
        built: completed.0,
        build_sources: completed.1,
        timings,
    })
}

pub(super) fn complete_build_source(
    frontier: TypedCompilationFrontier,
    evaluated: build_settings::EvaluatedBuild,
    package_inputs: Option<&PackageCompilationInputs>,
    build_execution_profile: Option<target::TargetProfile>,
    dependency_activation: bool,
    inputs_joined_after_build: bool,
    timings: &mut CompileTimings,
) -> Result<(BuiltTypedProgram, BuildSourceCustody), CompileFailure> {
    let mut spent = evaluated.source_completion_accounting.clone();
    let result = (|| {
        let TypedCompilationFrontier {
            preparation,
            build_stage: _,
            mut source,
            mut anonymous_source,
            product_sources,
            build_source: _,
            mut source_file_count,
            mut generated_source_custody,
            mut base_sources,
        } = frontier;
        let build_settings::EvaluatedBuild {
            configuration: mut computed_build_config,
            restricted_build_requests,
            base_source_consumption_commitment,
            constants: build_constants,
            source_completion_accounting: mut build_accounting,
        } = evaluated;
        if let Some(pending) = product_sources {
            let inputs = package_compilation::ProductSourceInputs::new(
                package_inputs,
                build_execution_profile,
            );
            let product = match pending.retained(&inputs, package_inputs)? {
                Some(product) => product,
                None => {
                    let (count, syntax) = pending.sources().assemble(
                        package_inputs,
                        timings,
                        &mut *preparation.catalogue()?,
                    )?;
                    let diagnostic_sources = syntax.sources.clone();
                    let product = lower_typed_frontier(
                        preparation.clone(),
                        count,
                        syntax,
                        build_execution_profile,
                        package_inputs,
                        timings,
                        None,
                    )
                    .map_err(|mut diagnostics| {
                        super::attribute_diagnostics(
                            &mut diagnostics,
                            &diagnostic_sources,
                            package_inputs,
                        );
                        diagnostics
                    })?;
                    pending.retain(inputs, &product)?;
                    product
                }
            };
            let selected = match computed_build_config.selected_build_machine_symbol {
                Some(selected) => Some(product.source.rejoin_machine(&source, selected)?),
                None => None,
            };
            source = product.source;
            anonymous_source = product.anonymous_source;
            base_sources = product.base_sources;
            source_file_count = product.source_file_count;
            generated_source_custody = product.generated_source_custody;
            Arc::make_mut(&mut computed_build_config).selected_build_machine_symbol = selected;
            // The fresh Build slice owns a separate actual request and receipt.
            // Equality of bytes or values cannot transfer Build evidence into
            // Product; only the additional work joins its cost history.
            if !build_constants.read.is_empty()
                || build_constants.folded_under_providers
                || inputs_joined_after_build
            {
                let entry = selected.ok_or_else(|| {
                    vec![Diagnostic::error(
                        "deferred Product lost the Build entry whose constants it must settle",
                    )]
                })?;
                let rejoined_accounting = build_settings::settle_rejoined_build_slice(
                    &build_constants,
                    source.build_input(),
                    entry,
                    package_inputs,
                    anonymous_source.as_ref(),
                    dependency_activation,
                    timings,
                )?;
                spent.append(rejoined_accounting.clone());
                build_accounting.append(rejoined_accounting);
            }
        }
        if computed_build_config
            .declared_product_selections
            .iter()
            .any(|request| {
                request
                    .operands
                    .iter()
                    .any(|operand| operand.application.is_some())
            })
        {
            source = source.materialize_authored_bodies(
                &mut crate::observations::PsiStageTimings::new(timings),
            )?;
        }
        super::product_selections::admit(
            &mut source,
            Arc::make_mut(&mut computed_build_config),
            package_inputs,
        )?;
        let sources = BuildSourceCustody {
            source_file_count,
            build_execution_profile,
            generated_source_custody,
            base_source_consumption_commitment,
        };
        let admission = BuildAdmission {
            application: match &computed_build_config.project_declaration {
                Some(build_declarations::BuildDeclaration::Application(application)) => {
                    Some(application.clone())
                }
                _ => None,
            },
            selected_build_machine_identity: computed_build_config
                .selected_machine_identity(source.typed())?,
            restricted_build_requests,
        };
        Ok((
            BuiltTypedProgram {
                build_accounting,
                anonymous_source,
                preparation,
                source,
                base_sources,
                computed_build_config,
                admission,
            },
            sources,
        ))
    })();
    result.map_err(|failure: CompileFailure| failure.with_source_completion_accounting(spent))
}

pub(super) fn lower_typed_frontier(
    preparation: products::SourcePreparation,
    source_file_count: usize,
    mut syntax: package_compilation::source_assembly::AssembledSyntax,
    execution_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
    build_stage: Option<usize>,
) -> Result<TypedCompilationFrontier, Vec<Diagnostic>> {
    let generated_source_custody = std::mem::take(&mut syntax.generated_source_custody);
    let base_sources = syntax.sources.clone();
    let build_scope_sources = std::mem::take(&mut syntax.build_scope_sources);
    let execution = execution_profile
        .or_else(target::TargetProfile::host_if_supported)
        .ok_or_else(|| vec![Diagnostic::error(
            "no target profile named and this host has no catalogued Omega deployment profile; name an exact target profile explicitly",
        )])?;
    if execution.native_realization().is_none() {
        return Err(vec![Diagnostic::error(format!(
            "native realization for target profile `{}` is not implemented",
            execution.target_name(),
        ))]);
    }
    build_settings::select_build_helpers(&mut syntax.syntax_trees, execution, &build_scope_sources);
    let build_source = syntax.build_source;
    // A frontier a Build stage is checked and run on holds that stage's
    // statements only (`build_settings::select_build_stage`). A frontier
    // nothing executes the Build entry of keeps the entry whole: the Product
    // frontier, and the one a paused Build's Product query is answered on.
    // There a name of a later stage is recorded on the entry and raised by
    // nobody, because nothing demands the entry.
    let build_stage = match (build_stage, build_source) {
        (Some(stage), Some(build)) => {
            build_settings::select_build_stage(&mut syntax.syntax_trees, build.source_id(), stage)?
        }
        _ => build_settings::BuildStage::WHOLE,
    };
    let request = source_compiler::SourceCompilationRequest {
        source_items: syntax.source_items,
        root_sources: syntax.root_sources,
        frontends: preparation.frontends.clone(),
        build_helper_identity: execution.target_name(),
        syntax: syntax.syntax_trees,
        sources: syntax.sources,
        top_level_bindings: syntax.source_scoped_top_level_bindings,
        selection_authority: package_inputs.map(|inputs| {
            Arc::new(inputs.clone()) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
    };
    let source = request.prepare(&mut crate::observations::PsiStageTimings::new(timings))?;
    Ok(TypedCompilationFrontier {
        preparation,
        build_stage,
        source,
        anonymous_source: syntax.anonymous_owner,
        product_sources: syntax
            .product_sources
            .map(products::DeferredProductCompilation::new),
        build_source,
        source_file_count,
        generated_source_custody,
        base_sources,
    })
}

pub(super) fn complete_product_frontend(
    built: BuiltTypedProgram,
    sources: &mut BuildSourceCustody,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
) -> Result<
    (
        source_compiler::PreparedSource,
        build_time_evaluation::SourceCompletionAccounting,
        Arc<products::ComputedBuildConfig>,
        BuildAdmission,
    ),
    CompileFailure,
> {
    let spent = built.build_accounting.clone();
    let result = (|| -> Result<_, Vec<Diagnostic>> {
        artifacts::compile_timings::counters::record_product_source_completion();
        let BuiltTypedProgram {
            build_accounting,
            anonymous_source: _,
            preparation,
            source,
            base_sources,
            mut computed_build_config,
            admission,
        } = built;
        let source =
            source.complete_authored(&mut crate::observations::PsiStageTimings::new(timings))?;
        let prepared = if let Some((extension, package_inputs)) = sources
            .admit_generated_extension(
                &base_sources,
                package_inputs,
                &computed_build_config.generated_sources,
                &mut *preparation.catalogue()?,
            )? {
            let first_source = extension
                .custody()
                .first()
                .map(|(source, _)| *source)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "admitted generated extension has no source coordinate",
                    )]
                })?;
            let (sources, units) = extension.parse(&base_sources, timings)?;
            let diagnostic_sources = Arc::clone(&sources);
            source
                .extend(
                    source_compiler::SourceExtensionRequest {
                        units,
                        sources,
                        first_source,
                        selection_authority: Arc::new(package_inputs.clone()),
                    },
                    &mut crate::observations::PsiStageTimings::new(timings),
                )
                .map_err(|mut diagnostics| {
                    super::attribute_diagnostics(
                        &mut diagnostics,
                        &diagnostic_sources,
                        Some(package_inputs),
                    );
                    diagnostics
                })?
        } else {
            source.finish()
        };
        let bindings =
            prepared.admit_root_bindings(&computed_build_config.root_binding_requests)?;
        Arc::make_mut(&mut computed_build_config)
            .config
            .root_bindings = bindings;
        Ok((prepared, build_accounting, computed_build_config, admission))
    })();
    result.map_err(|diagnostics| CompileFailure::source_completion(diagnostics, spent))
}
