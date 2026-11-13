/*
Check the `Build` program that a `TerminalBuildActivation` will actually consume
(`prepare_checked_build`).

Semantic admission is mandatory with or without package inputs; package authority
adds declaration-selection restrictions on top of the semantic floor rather than
toggling whether checking runs.

How we project, fold, and check the `Build` slice without corrupting the Product
continuation:
1. Retain the canonical float catalog and public requirements supplied by the
   target realization table when projecting the Build slice. Ordinary provider
   defaults remain declared by their supplying package.
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

Steps 1-3 are `settle_build_slice` and the discharge is `discharge_build_slice`,
because the Product is not always completed from the frontier Build ran on.
When Product sources were deferred until Build finished, they are typed as a
new frontier that holds the Build source again with its constants unsettled.
`settle_rejoined_build_slice` runs the same settlement and discharge there, so
a Build constant that needs Build's providers is folded under them and never
left for the Product's targets to evaluate. The caller asks for it when
admission reports such a fold (`provider_folded_constants`) or when package
inputs joined after Build ran; a constant that needs no provider folds the
same way on either frontier.
*/

use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use provider_planning::{ProviderPlanDerivation, ProviderPlanningContext};
use psi_compiler::RetainedCheck;
use source::DependencyScope;
use target::TargetProfile;

/*
The `Build` slice of one typed frontier with its constants folded under the
admitted `Build` provider plans. The plans and their review provenance belong
to the same settlement and must not be re-derived separately.
*/
pub(crate) struct SettledBuildSlice {
    // For admission, the slice as it stands after settlement, ready to check.
    // For a rejoined slice, the slice its pre-checks were evaluated over,
    // which is exactly the work to discharge: nothing checks a rejoined slice
    // again, so a declaration settlement did not evaluate must stay with the
    // Product's pre-checks rather than be discharged unevaluated.
    program: typed_trees::TypedTrees,
    plans: Option<SettledBuildPlans>,
    // Whether a constant waited for the provider plans. Only such a constant
    // needs `Build`'s authority; the rest fold the same way wherever the
    // source is typed.
    folded_under_plans: bool,
}

struct SettledBuildPlans {
    facts: effects::SelectedProviderPlanFacts,
    provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
}

/*
Who reads the slice's provider plans. Admission executes `Build` under them, so
it always needs them. A rejoined slice was already admitted and executed; it
needs them only to fold a constant that is still pending, and deriving them for
every deferred Product would repeat provider planning on each compilation.
*/
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildPlanDemand {
    Admission,
    PendingConstants,
}

pub(crate) fn settle_build_slice(
    typed: &mut typed_trees::TypedTrees,
    pre_checks: &[build_time_evaluation::PreCheckEvaluation],
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    dependency_activation: bool,
    execution_profile: Option<target::TargetProfile>,
    demand: BuildPlanDemand,
) -> Result<SettledBuildSlice, Vec<Diagnostic>> {
    // The target realization table supplies canonical float operations, so
    // the activation keeps the sealed catalog map and its public requirements
    // in its projection whether or not any Build selection names them.
    let selected_declarations =
        typed_trees::float_providers::catalog_realization_declarations(typed);
    // Validation subjects are the request's root package, as on the product
    // side. A dependency's Build executed for its consumer checks only what
    // its entry and selections reach.
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
    if pending.is_empty() && demand == BuildPlanDemand::PendingConstants {
        return Ok(SettledBuildSlice {
            program: projection,
            plans: None,
            folded_under_plans: false,
        });
    }
    // Evaluation can materialize declarations, so provider planning reads the
    // slice as it stands now; `projection` stays the slice that was evaluated.
    let reprojected = if pre_checks.is_empty() {
        None
    } else {
        Some(validation::execution_projection::project_build_program(
            typed,
            entry,
            &selected_declarations,
            is_subject,
        )?)
    };
    let provisional = reprojected.as_ref().unwrap_or(&projection);
    let context = ProviderPlanningContext::new(provisional, DependencyScope::Build);
    let defaults = build_time_evaluation::provider_declarations::retain_provider_defaults(context)?;
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
    let program = match demand {
        BuildPlanDemand::PendingConstants => projection,
        BuildPlanDemand::Admission if had_pending => {
            validation::execution_projection::project_build_program(
                typed,
                entry,
                &selected_declarations,
                is_subject,
            )?
        }
        BuildPlanDemand::Admission => reprojected.unwrap_or(projection),
    };
    Ok(SettledBuildSlice {
        program,
        plans: Some(SettledBuildPlans {
            facts: selected_provider_plan_facts,
            provenance,
        }),
        folded_under_plans: had_pending,
    })
}

/*
Remove the settled `Build` slice from the pre-checks that remain for the
Product, so Product completion evaluates only what `Build` did not.
*/
fn discharge_build_slice(
    typed: &typed_trees::TypedTrees,
    pre_checks: &mut Vec<build_time_evaluation::PreCheckEvaluation>,
    completed: &typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    *pre_checks = pre_checks
        .iter()
        .map(|continuation| continuation.exclude_completed_work(typed, completed))
        .collect::<Result<_, _>>()?;
    Ok(())
}

/*
Settle and discharge the `Build` slice of a frontier that replaced the one
`Build` was admitted and executed on. `entry` is the executed build machine
rejoined onto this frontier. The slice was already checked on its original
frontier, so this folds its constants and discharges it without a second check.
Any failure rejects the compilation: the caller never continues with the
slice's constants still pending.
*/
pub fn settle_rejoined_build_slice(
    input: psi_compiler::BuildSourceInput<'_>,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    dependency_activation: bool,
    execution_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    let psi_compiler::BuildSourceInput { typed, pre_checks } = input;
    let settled = settle_build_slice(
        typed,
        pre_checks,
        entry,
        packages,
        dependency_activation,
        execution_profile,
        BuildPlanDemand::PendingConstants,
    )?;
    discharge_build_slice(typed, pre_checks, &settled.program)
}

pub(crate) fn prepare_admitted_build(
    typed: &mut typed_trees::TypedTrees,
    pre_checks: &mut Vec<build_time_evaluation::PreCheckEvaluation>,
    entry: symbols::SymbolHandle,
    packages: Option<&PackageCompilationInputs>,
    dependency_activation: bool,
    execution_profile: Option<target::TargetProfile>,
    _timings: &mut artifacts::compile_timings::CompileTimings,
    _priors: &[&RetainedCheck],
    _retain: bool,
    _checked_slices: Option<&dyn psi_compiler::CheckedSliceStore>,
) -> Result<
    (
        std::sync::Arc<psi_compiler::AcceptedTerminalSource>,
        super::provider_execution::BuildProviderPlans,
        Option<RetainedCheck>,
        bool, // A constant was folded under the provider plans.
    ),
    Vec<Diagnostic>,
> {
    let SettledBuildSlice {
        program,
        plans,
        folded_under_plans,
    } = settle_build_slice(
        typed,
        pre_checks,
        entry,
        packages,
        dependency_activation,
        execution_profile,
        BuildPlanDemand::Admission,
    )?;
    let SettledBuildPlans {
        facts: selected_provider_plan_facts,
        provenance,
    } = plans.ok_or_else(|| {
        vec![Diagnostic::error(
            "Build admission settled its slice without provider plans",
        )]
    })?;
    // Capture the source owner before the route judges it. Neither a prior
    // checked tree nor a later source clone can substitute for this owner.
    let program = std::sync::Arc::new(typed_trees::FrozenTypedTrees::from(program));
    let providers = super::provider_execution::BuildProviderPlans::capture(
        &program,
        selected_provider_plan_facts,
        &provenance,
    )?;
    let accepted = psi_compiler::AcceptedTerminalSource::judge(
        program,
        psi_compiler::TerminalSourceCompilation {
            entries: &[entry],
            subject: packages.map(|inputs| inputs.root()),
            selected: &[],
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )?;
    discharge_build_slice(typed, pre_checks, accepted.source())?;
    Ok((
        std::sync::Arc::new(accepted),
        providers,
        None,
        folded_under_plans,
    ))
}
