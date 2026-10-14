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
    packages: Option<&PackageCompilationInputs>,
    execution_profile: Option<target::TargetProfile>,
    timings: &mut artifacts::compile_timings::CompileTimings,
    priors: &[&psi_compiler::RetainedCheck],
    retain: bool,
) -> Result<
    (
        PreparedBuildMachineProgram,
        Option<psi_compiler::RetainedCheck>,
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
    let projection = validation::execution_projection::project_build_program(typed, entry)?;
    let mut pending = Vec::new();
    for continuation in pre_checks.iter() {
        let continuation = continuation.project(typed, &projection)?;
        if let Some(continuation) = continuation.evaluate_or_defer(typed)? {
            pending.push(continuation);
        }
    }
    let provisional = validation::execution_projection::project_build_program(typed, entry)?;
    let context = provider_planning::ProviderPlanningContext::new(
        &provisional,
        source::DependencyScope::Build,
    );
    let defaults = build_time_evaluation::provider_declarations::retain_provider_defaults(context)?;
    let declarations = provider_planning::target_machines::settle_provider_defaults(
        context,
        &defaults,
        execution_profile.unwrap_or_else(target::TargetProfile::host),
    );
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
    )?;
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
        let operator_selections = validation::derive_pre_flow_operator_selections(typed);
        selected_dispatch::SelectedConstantEvaluation::prepare(
            context,
            execution_profile.unwrap_or_else(target::TargetProfile::host),
            &[],
            &operator_selections,
            &selected_provider_plan_facts,
        )?
        .evaluate_pending(typed, pending)?;
    }
    // Carry the invocation's earlier language judgments only for the exact
    // settled Build program this activation projects: its own slot's chain
    // plus sibling rows whose projections arrived equal. Psi still admits
    // this invocation's declaration authority; provider/comparison settlement
    // and executor grants remain fresh below.
    let outcome = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let program = validation::execution_projection::project_build_program(typed, entry)?;
            let result = psi_compiler::CheckRequest {
                program,
                admission: psi_compiler::SourceAdmission {
                    declarations: packages
                        .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
                    ..Default::default()
                },
                previous: priors,
                retain,
            }
            .check();
            artifacts::compile_timings::counters::record_preliminary_check(started.elapsed());
            if result.as_ref().is_ok_and(|outcome| outcome.reused) {
                artifacts::compile_timings::counters::record_settled_check_reuse();
            } else {
                artifacts::compile_timings::counters::record_settled_check(started.elapsed());
            }
            result
        },
    )?;
    let psi_compiler::CheckedSource {
        program: checked,
        retained,
        ..
    } = outcome;
    // Build installs execution-specific comparison facts in its own view;
    // the shared language judgment stays unchanged for other activations.
    let mut checked =
        std::sync::Arc::try_unwrap(checked).unwrap_or_else(|shared| (*shared).clone());
    selected_dispatch::settle_selected_comparison_execution(
        &mut checked,
        source::DependencyScope::Build,
        &provenance,
    )?;
    // Only successful Build admission discharges this continuation's Build
    // work. Product keeps its exact complement, not a second full evaluation.
    *pre_checks = pre_checks
        .iter()
        .map(|continuation| continuation.exclude_projection(typed, &checked))
        .collect::<Result<_, _>>()?;
    Ok((PreparedBuildMachineProgram::from_checked(checked), retained))
}
