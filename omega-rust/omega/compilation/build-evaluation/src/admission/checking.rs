//! Check the program that a Build activation will actually consume.
//!
//! Semantic admission is required with or without package inputs. Package
//! authority is an additional obligation, not the switch that enables checking.
//! Transfer the prepared and checked typed program: checking one clone and
//! interpreting a separately specialized clone would not bind execution to
//! the admitted bodies. Provider planning uses the Build declaration context
//! and admitted execution profile, never the current product's unfinished
//! provider choices. Only comparison execution facts enter the interpreter;
//! runtime adapter dispatch does not grant Build host-service authority.
//!
//! Product declarations remain a frozen selection view. The complete Build
//! dependency context goes through ordinary checking, including helper bodies;
//! a declaration's visibility never substitutes for its contract proof.

use build_time_evaluation::PreparedBuildMachineProgram;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

pub(crate) fn prepare_checked_build(
    typed: &mut typed_trees::TypedTrees,
    pre_checks: &mut Vec<build_time_evaluation::PreCheckEvaluation>,
    entry: symbols::SymbolHandle,
    target_machine_declarations: &provider_planning::target_machines::SelectedTargetMachineDeclarations,
    packages: Option<&PackageCompilationInputs>,
    execution_profile: Option<target::TargetProfile>,
    timings: &mut artifacts::compile_timings::CompileTimings,
    prior: Option<&typed_trees_to_checked_trees::RetainedCheck>,
    retain: bool,
) -> Result<
    (
        PreparedBuildMachineProgram,
        Option<typed_trees_to_checked_trees::RetainedCheck>,
    ),
    Vec<Diagnostic>,
> {
    // Pure compiler evaluation settles type-dependent storage before ordinary
    // ownership and contract checking. Its existing admission floor forbids
    // service effects and retains exact declaration-selection authority; it
    // does not grant execution of the Build activation. Every retained helper
    // body still owes the settled check below before filesystem or other
    // sponsored Build execution can consume the prepared program.
    // Settle the owned work in the canonical arenas. Evaluating only a clone
    // would leave the continuation's Build types unresolved and make Product
    // repeat their evaluation under unrelated provider selections. Projection
    // selects work and later the executable judgment; it is not a copyback API.
    let projection = typed_trees_to_checked_trees::project_build_program(typed, entry)?;
    let mut pending = Vec::new();
    for continuation in pre_checks.iter() {
        let continuation = continuation.project(typed, &projection)?;
        if let Some(continuation) = continuation.evaluate_or_defer(typed)? {
            pending.push(continuation);
        }
    }
    let provisional = typed_trees_to_checked_trees::project_build_program(typed, entry)?;
    let context = provider_planning::ProviderPlanningContext::new(
        &provisional,
        source::DependencyScope::Build,
    );
    let defaults = target_machine_declarations.retain_provider_defaults(context)?;
    let declarations = target_machine_declarations.settle_provider_defaults(
        context,
        &defaults,
        execution_profile.unwrap_or_else(target::TargetProfile::host),
    )?;
    let bindings = provider_planning::evaluated_via_bindings::evaluate_via_bindings(
        context,
        execution_profile,
        packages,
    )?;
    let candidates = provider_planning::derive_satisfies_plans(
        provider_planning::ProviderPlanDerivation::evaluated(
            context,
            &[],
            execution_profile.map(target::TargetProfile::target_name),
            &bindings,
            &declarations.origins,
        ),
    );
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
                &candidates,
                profile.native_target(),
                &declarations.provider_defaults,
                &[],
            )?;
            provider_planning::selected_provider_plan_facts(context, selected)?
        };
    if !pending.is_empty() {
        let operator_selections =
            typed_trees_to_checked_trees::derive_pre_flow_operator_selections(typed);
        selected_dispatch::SelectedConstantEvaluation::prepare(
            context,
            &operator_selections,
            &selected_provider_plan_facts,
        )?
        .evaluate_pending(typed, pending)?;
    }
    // Carry the discovery pass's language judgment only for the exact settled
    // Build program. Psi still admits this invocation's declaration authority;
    // provider/comparison settlement and executor grants remain fresh below.
    let outcome = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let program = typed_trees_to_checked_trees::project_build_program(typed, entry)?;
            let preparation_request = typed_trees_to_typed_trees::PreparationRequest::settled();
            let checking = typed_trees_to_checked_trees::CheckingRequest::new()
                .with_declaration_authority(
                    packages.map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
                );
            let result =
                typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation_request)
                    .and_then(|(program, preparation)| {
                        typed_trees_to_checked_trees::lower_typed_trees_reusing(
                            program,
                            preparation,
                            &checking,
                            prior,
                            retain,
                        )
                    });
            if !result.as_ref().is_ok_and(|outcome| outcome.took_prior) {
                artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            }
            result
        },
    )?;
    let typed_trees_to_checked_trees::CheckingOutcome {
        mut checked,
        retained,
        ..
    } = outcome;
    selected_dispatch::settle_selected_comparison_execution(
        &mut checked,
        source::DependencyScope::Build,
        &provenance,
    )?;
    // Only successful Build admission discharges this continuation's Build
    // work. Product keeps its exact complement, not a second full evaluation.
    *pre_checks = pre_checks
        .iter()
        .map(|continuation| continuation.exclude_projection(typed, &checked.typed))
        .collect::<Result<_, _>>()?;
    Ok((PreparedBuildMachineProgram::from_checked(checked), retained))
}
