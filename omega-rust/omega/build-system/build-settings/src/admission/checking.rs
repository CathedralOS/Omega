/*
Check the `Build` program that a `TerminalBuildActivation` will actually consume
(`prepare_checked_build`).

Semantic admission is mandatory with or without package inputs; package authority
adds declaration-selection restrictions on top of the semantic floor rather than
toggling whether checking runs.

How we project, fold, and check the `Build` slice without corrupting the Product
continuation:
1. Rejoin completed toolchain provider defaults (`toolchain_defaults.rejoin_build(typed)`)
   against the canonical `TypedTrees` arena before projecting, and collect only
   the `SymbolHandle`s of actually selected providers and boundary requirements
   (describing an unselected implementation never imports its body).
2. Project the `Build` slice (`project_build_program`) and verify that source
   code cannot manufacture a counterfeit `Build` value
   (`admit_toolchain_build_activation`) before ordinary borrow checking runs.
3. Evaluate or defer pending type-dependent `PreCheckEvaluation` continuations
   against the canonical `TypedTrees` arena (never a throwaway clone, which
   would leave `Build` types unresolved in the continuation), then settle
   `Build`-scope provider plans (`derive_satisfies_plans`,
   `select_derived_provider_plans`) and fold any remaining deferred constant
   evaluations (`SelectedConstantEvaluation::evaluate_pending`).
4. Run `psi_compiler::CheckRequest` on the settled `Build` projection, reusing
   `priors` only when the projected `Build` program matches, install
   `Build`-specific comparison dispatch (`settle_selected_comparison_execution`)
   on an owned copy of `CheckedTrees`, and discharge the completed `Build`
   pre-checks from `pre_checks` so the subsequent Product compilation only
   evaluates its complement.
*/

use artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES;
use build_time_evaluation::{
    PreparedBuildMachineProgram, provider_declarations::ProviderSelectionSubject,
};
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::{ProviderPlanDerivation, ProviderPlanningContext};
use psi_compiler::RetainedCheck;
use source::DependencyScope;
use target::TargetProfile;

pub(crate) fn prepare_checked_build(
    typed: &mut typed_trees::TypedTrees,
    pre_checks: &mut Vec<build_time_evaluation::PreCheckEvaluation>,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    dependency_activation: bool,
    toolchain_defaults: Option<&package_compilation::CompletedProviderDefaults>,
    execution_profile: Option<target::TargetProfile>,
    timings: &mut artifacts::compile_timings::CompileTimings,
    priors: &[&RetainedCheck],
    retain: bool,
) -> Result<
    (
        PreparedBuildMachineProgram,
        super::provider_execution::BuildProviderPlans,
        Option<RetainedCheck>,
    ),
    Vec<Diagnostic>,
> {
    let completed_defaults = toolchain_defaults
        .map(|defaults| defaults.rejoin_build(typed))
        .transpose()?
        .unwrap_or_default();
    // The target realization table supplies canonical float operations, so
    // the activation keeps the sealed catalog map and its public requirements
    // in its projection whether or not any Build selection names them.
    let mut selected_declarations =
        typed_trees::float_providers::catalog_realization_declarations(typed);
    for selection in &completed_defaults {
        selected_declarations.push(selection.provider_type.symbol);
        match &selection.subject {
            ProviderSelectionSubject::BoundaryTrait(identity)
            | ProviderSelectionSubject::BoundaryRequirement(identity) => {
                selected_declarations.push(identity.symbol);
            }
            ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                selected_declarations.extend(family.coordinates().iter().map(|row| row.symbol));
            }
        }
    }
    // Validation subjects are the request's root package, as on the product
    // side: the core Build that settles provider defaults, or a dependency's
    // own Build executed for its consumer, checks only what its entry and
    // selections reach.
    let is_subject = |file: &source::SourceFile| {
        !dependency_activation && PackageCompilationInputs::is_validation_subject(packages, file)
    };
    let projection = validation::execution_projection::project_build_program(
        typed,
        entry,
        &selected_declarations,
        is_subject,
    )?;
    build_time_evaluation::admit_toolchain_build_activation(&projection)?;
    let mut pending = Vec::new();
    for continuation in pre_checks.iter() {
        let continuation = continuation.project(typed, &projection)?;
        if let Some(continuation) = continuation.evaluate_or_defer(typed)? {
            pending.push(continuation);
        }
    }
    let provisional = if pre_checks.is_empty() {
        projection
    } else {
        validation::execution_projection::project_build_program(
            typed,
            entry,
            &selected_declarations,
            is_subject,
        )?
    };
    let context = ProviderPlanningContext::new(&provisional, DependencyScope::Build);
    let mut defaults =
        build_time_evaluation::provider_declarations::retain_provider_defaults(context)?;
    defaults.extend(completed_defaults.into_iter().map(|selection| {
        build_time_evaluation::provider_declarations::TargetProviderDefaults {
            target: selection
                .target
                .map(|target| target.canonical_name().to_owned()),
            machines: Vec::new(),
            selections: vec![selection],
        }
    }));
    let declarations = provider_planning::target_machines::settle_provider_defaults(
        &typed_trees::provider_declarations::provider_target_declarations(
            context.typed,
            context.scope,
        ),
        &defaults,
        execution_profile.unwrap_or_else(TargetProfile::host),
    );
    let bindings =
        crate::via_binding_evaluation::evaluate_via_bindings(context, execution_profile, packages)?;
    let body_declarations = typed_trees::direct_calls::collect(context.typed, context.scope);
    let service_catalog = std::sync::Arc::new(
        typed_trees::service_declarations::PreparedServiceCatalog::prepare(context, &[]),
    );
    let services = service_catalog.bind(context).map_err(|error| vec![error])?;
    let candidates = provider_planning::derive_satisfies_plans(ProviderPlanDerivation::evaluated(
        services,
        &body_declarations,
        &[],
        execution_profile.map(TargetProfile::target_name),
        &bindings,
        &declarations.origins,
        &[],
    ))?;
    let (selected_provider_plan_facts, provenance) =
        if candidates.is_empty() && declarations.provider_defaults.is_empty() {
            (effects::SelectedProviderPlanFacts::default(), Vec::new())
        } else {
            let profile = execution_profile.ok_or_else(|| {
                vec![Diagnostic::error(
                    "Build provider planning requires an admitted execution profile",
                )]
            })?;
            let selected = provider_planning::select_derived_provider_plans(
                context.typed,
                &candidates,
                profile,
                &declarations.provider_defaults,
                &[],
            )?;
            provider_planning::selected_provider_plan_facts(context, selected)?
        };
    let had_pending = !pending.is_empty();
    if had_pending {
        let operator_selections = validation::derive_pre_flow_operator_selections(typed);
        super::selected_constants::SelectedConstantEvaluation::prepare(
            services,
            execution_profile.unwrap_or_else(TargetProfile::host),
            &[],
            &operator_selections,
            &selected_provider_plan_facts,
        )?
        .evaluate_pending(typed, pending)?;
    }
    let outcome = timings.record_result(TYPED_TREES_TO_CHECKED_TREES, || {
        let started = std::time::Instant::now();
        let program = if had_pending {
            validation::execution_projection::project_build_program(
                typed,
                entry,
                &selected_declarations,
                is_subject,
            )?
        } else {
            provisional
        };
        let result = psi_compiler::CheckRequest {
            program,
            admission: psi_compiler::SourceAdmission {
                declarations: packages
                    .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
                ..Default::default()
            },
            previous: priors,
            retain,
            library_slice: psi_compiler::LibrarySliceRequest {
                root_package: packages.map(|inputs| inputs.root()),
            },
        }
        .check();
        artifacts::compile_timings::counters::record_preliminary_check(started.elapsed());
        if !result.as_ref().is_ok_and(|outcome| outcome.reused) {
            artifacts::compile_timings::counters::record_settled_check(started.elapsed());
        }
        result
    })?;
    let psi_compiler::CheckedSource {
        program: checked,
        retained,
        ..
    } = outcome;
    let mut checked =
        std::sync::Arc::try_unwrap(checked).unwrap_or_else(|shared| (*shared).clone());
    selected_dispatch::settle_selected_comparison_execution(
        &mut checked,
        DependencyScope::Build,
        &provenance,
    )?;
    *pre_checks = pre_checks
        .iter()
        .map(|continuation| continuation.exclude_completed_work(typed, &checked))
        .collect::<Result<_, _>>()?;
    let providers = super::provider_execution::BuildProviderPlans::capture(
        &checked,
        selected_provider_plan_facts,
        &provenance,
    )?;
    Ok((
        PreparedBuildMachineProgram::from_checked(checked),
        providers,
        retained,
    ))
}
