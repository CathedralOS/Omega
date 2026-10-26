//! Admit and execute Build while retaining pending Product implementations.
//!
//! Dependency preparation needs Build's result and declaration descriptions,
//! not an executable whole-package Product. An actual Product request completes
//! its loaded module bodies, then types generated sources without replaying Build.

use artifacts::compile_timings::CompileTimings;
use compilation_products::{
    BuildAdmission, BuiltTypedProgram, CheckedExecutionRequest, CompileFailure,
    TypedCompilationFrontier,
};
use diagnostics::Diagnostic;
use package_compilation::BuildSourceCustody;
use package_compilation::PackageCompilationInputs;
use std::path::Path;
use std::sync::Arc;

pub(super) struct BuildSourceInvocation {
    frontier: TypedCompilationFrontier,
    evaluation: build_evaluation::PreparedBuildEvaluation,
    package_inputs: Option<PackageCompilationInputs>,
    selected_target_profile: Option<target::TargetProfile>,
    build_execution_profile: Option<target::TargetProfile>,
    timings: CompileTimings,
    started: std::time::Instant,
}

/// Consume a prepared typed program. Source preparation owns any reuse;
/// declaration authority and Build effects are admitted for this activation.
pub(super) fn prepare_build_invocation(
    root_path: &Path,
    child: CheckedExecutionRequest<'_>,
    frontier: TypedCompilationFrontier,
    mut timings: CompileTimings,
    pure_build_compilation: Option<&build_evaluation::PureBuildCompilation>,
) -> Result<BuildSourceInvocation, CompileFailure> {
    let started = std::time::Instant::now();
    let CheckedExecutionRequest {
        selected_target_profile,
        package_inputs,
        build,
        optimization_rollback: _,
        permit_unsettled_fused_service_fields: _,
        settled_check_reuse,
    } = child;
    let build_execution_profile = build.profile;
    let TypedCompilationFrontier {
        mut source,
        anonymous_source,
        product_sources,
        build_source_id,
        source_file_count,
        generated_source_custody,
        base_sources,
    } = frontier;
    let build_input = source.build_input();
    let evaluation = build_evaluation::prepare_build_evaluation(
        build_evaluation::BuildEvaluationRequest {
            pure_build_compilation,
            root_path,
            typed: build_input.typed,
            pre_checks: build_input.pre_checks,
            build_source_id,
            sources: &base_sources,
            package_inputs,
            anonymous_source: anonymous_source.as_ref(),
            generated_source_custody: &generated_source_custody,
            execution: build,
            artifact_only: false,
            prior_checks: settled_check_reuse
                .prior
                .build
                .as_ref()
                .into_iter()
                .chain(settled_check_reuse.prior.sibling_build_checks.iter())
                .collect(),
            retain_check: settled_check_reuse.retain,
        },
        &mut timings,
    )?;
    Ok(BuildSourceInvocation {
        frontier: TypedCompilationFrontier {
            source,
            anonymous_source,
            product_sources,
            build_source_id,
            source_file_count,
            generated_source_custody,
            base_sources,
        },
        evaluation,
        package_inputs: package_inputs.cloned(),
        selected_target_profile,
        build_execution_profile,
        timings,
        started,
    })
}

impl BuildSourceInvocation {
    pub(super) fn execution(&mut self) -> Option<&mut build_evaluation::PreparedBuildExecution> {
        self.evaluation.execution()
    }

    pub(super) fn complete(
        self,
        returned: Option<build_evaluation::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
        reuse: &compilation_products::SettledCheckReuse,
        catalogue: &mut source::SourceCatalogue,
    ) -> Result<compilation_products::PreparedBuild, CompileFailure> {
        let Self {
            frontier,
            evaluation,
            package_inputs,
            selected_target_profile,
            build_execution_profile,
            mut timings,
            started,
        } = self;
        if let (Some(original), Some(joined)) = (package_inputs.as_ref(), joined_inputs)
            && original != joined
        {
            return Err(vec![Diagnostic::error(
                "Build completion cannot replace its original package inputs",
            )]
            .into());
        }
        let initially_unbound = package_inputs.is_none();
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
        if let Some(returned) = &returned {
            returned
                .validate_source_join(package_inputs, initially_unbound)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
        }
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
        let completed = complete_build_source(
            frontier,
            evaluated,
            package_inputs,
            build_execution_profile,
            &mut timings,
            reuse,
            catalogue,
        )
        .map_err(|failure| super::attribute_failure(failure, &initial_sources, package_inputs))?;
        timings.add_completed(
            artifacts::compile_timings::BUILD_AND_CHECKED_CONTINUATION,
            started.elapsed().as_micros(),
        );
        Ok(compilation_products::PreparedBuild {
            built: completed.0,
            build_sources: completed.1,
            timings,
        })
    }
}

fn complete_build_source(
    frontier: TypedCompilationFrontier,
    evaluated: build_evaluation::EvaluatedBuild,
    package_inputs: Option<&PackageCompilationInputs>,
    build_execution_profile: Option<target::TargetProfile>,
    timings: &mut CompileTimings,
    reuse: &compilation_products::SettledCheckReuse,
    catalogue: &mut source::SourceCatalogue,
) -> Result<(BuiltTypedProgram, BuildSourceCustody), CompileFailure> {
    let TypedCompilationFrontier {
        mut source,
        anonymous_source: _,
        product_sources,
        build_source_id: _,
        mut source_file_count,
        mut generated_source_custody,
        mut base_sources,
    } = frontier;
    let build_evaluation::EvaluatedBuild {
        configuration: mut computed_build_config,
        restricted_build_requests,
        base_source_consumption_commitment,
        retained_check: retained_build_check,
    } = evaluated;
    if let Some(pending) = product_sources {
        // Successful Build return currently implies its dependency requests
        // have all joined (the executor refuses any unresolved request). Final
        // ownership precedes Product typing; never retag the private judgment.
        let (count, syntax) = pending.assemble(package_inputs, timings, catalogue)?;
        let diagnostic_sources = syntax.sources.clone();
        let product = lower_typed_frontier(
            count,
            syntax,
            build_execution_profile,
            package_inputs,
            reuse,
            timings,
        )
        .map_err(|mut diagnostics| {
            super::attribute_diagnostics(&mut diagnostics, &diagnostic_sources, package_inputs);
            diagnostics
        })?;
        // The retained parsed input preserves declaration source coordinates.
        // No display name, allocation order or old typed handle selects a body
        // in this distinct Product context.
        let selected = match computed_build_config.selected_build_machine_symbol {
            Some(selected) => Some(product.source.rejoin_machine(&source, selected)?),
            None => None,
        };
        source = product.source;
        base_sources = product.base_sources;
        source_file_count = product.source_file_count;
        generated_source_custody = product.generated_source_custody;
        Arc::make_mut(&mut computed_build_config).selected_build_machine_symbol = selected;
    }
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
            source,
            base_sources,
            computed_build_config,
            admission,
            retained_build_check,
        },
        sources,
    ))
}

pub(super) fn lower_typed_frontier(
    source_file_count: usize,
    mut syntax: package_compilation::source_assembly::AssembledSyntax,
    execution_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    previous: &compilation_products::SettledCheckReuse,
    timings: &mut CompileTimings,
) -> Result<TypedCompilationFrontier, Vec<Diagnostic>> {
    if syntax.product_sources.is_none()
        && let Some(ownership) = syntax.anonymous_owner.take()
    {
        syntax.sources = ownership.complete_without_dependencies()?;
    }
    let generated_source_custody = std::mem::take(&mut syntax.generated_source_custody);
    let base_sources = syntax.sources.clone();
    // Only build helpers select an execution target here. Product families
    // retain a canonical frontend shared by every requested product target.
    let build_scope_sources = std::mem::take(&mut syntax.build_scope_sources);
    let execution = execution_profile
        .or_else(target::TargetProfile::host_if_supported)
        .ok_or_else(|| vec![Diagnostic::error(
            "no target profile named and this host has no catalogued Omega deployment profile; name an exact target profile explicitly",
        )])?;
    // Build execution still requires an implemented host shape, but that
    // projection is not the identity used to choose source declarations.
    if execution.native_realization().is_none() {
        return Err(vec![Diagnostic::error(format!(
            "native realization for target profile `{}` is not implemented",
            execution.target_name(),
        ))]);
    }
    build_evaluation::select_build_helpers(
        &mut syntax.syntax_trees,
        execution,
        &build_scope_sources,
    );
    let build_source_id = syntax.build_source_id;
    let request = psi_compiler::SourceCompilationRequest {
        syntax: syntax.syntax_trees,
        source_items: syntax.source_items,
        sources: syntax.sources,
        top_level_bindings: syntax.source_scoped_top_level_bindings,
        selection_authority: package_inputs.map(|inputs| {
            Arc::new(inputs.clone()) as Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>
        }),
        previous: previous
            .prior
            .product
            .iter()
            .chain(&previous.prior.sibling_product_checks)
            .cloned()
            .collect(),
        pending_sources: previous.pending_sources.clone(),
        root_package: package_inputs.map(PackageCompilationInputs::root),
        build_helper_identity: execution.identity().as_str(),
    };
    let source = request.prepare(&mut terminal_production::PsiStageTimings::new(timings))?;
    Ok(TypedCompilationFrontier {
        source,
        anonymous_source: syntax.anonymous_owner,
        product_sources: syntax.product_sources,
        build_source_id,
        source_file_count,
        generated_source_custody,
        base_sources,
    })
}

/// An actual Product demand consumes its retained Build exactly once and
/// completes the implementation sources already selected by its module closure.
/// Generated source is a later stratum, never a reason to rerun authored Build.
pub(super) fn complete_product_frontend(
    built: BuiltTypedProgram,
    sources: &mut BuildSourceCustody,
    package_inputs: Option<&PackageCompilationInputs>,
    timings: &mut CompileTimings,
    catalogue: &mut source::SourceCatalogue,
) -> Result<
    (
        Arc<psi_compiler::PreparedSource>,
        Arc<compilation_products::ComputedBuildConfig>,
        BuildAdmission,
    ),
    Vec<Diagnostic>,
> {
    artifacts::compile_timings::counters::record_product_source_completion();
    let BuiltTypedProgram {
        source,
        base_sources,
        computed_build_config,
        admission,
        retained_build_check: _,
    } = built;
    let source =
        source.complete_authored(&mut terminal_production::PsiStageTimings::new(timings))?;
    // Package custody, generated-byte admission and parsing stay outside Psi's
    // resolver/typing owner. No authored Build effects repeat on this path.
    let prepared = if let Some((extension, package_inputs)) = sources.admit_generated_extension(
        &base_sources,
        package_inputs,
        &computed_build_config.generated_sources,
        catalogue,
    )? {
        let (sources, units) = extension.parse(&base_sources, timings)?;
        // Extension failures can name generated units absent from base_sources.
        // Resolve their spans before consuming the only map that owns those bytes;
        // the outer authored-source handler preserves resolved locations.
        let diagnostic_sources = Arc::clone(&sources);
        source
            .extend(
                psi_compiler::SourceExtensionRequest {
                    units,
                    sources,
                    first_source: source::SourceId(base_sources.len()),
                    selection_authority: Arc::new(package_inputs.clone()),
                },
                &mut terminal_production::PsiStageTimings::new(timings),
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
    Ok((prepared, computed_build_config, admission))
}
