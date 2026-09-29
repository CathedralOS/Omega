//! Check the program that a Build activation will actually consume.
//!
//! Semantic admission is required with or without package inputs. Package
//! authority is an additional obligation, not the switch that enables checking.
//! Transfer the checker-transformed typed program: checking one clone and
//! interpreting a separately specialized clone would not bind execution to
//! the admitted bodies. Provider planning uses the Build declaration context
//! and admitted execution profile, never the current product's unfinished
//! provider choices. Only comparison execution facts enter the interpreter;
//! runtime adapter dispatch does not grant Build host-service authority.
//!
//! This still checks the whole provisional program. Removing that work requires
//! a sound Build-closure projection and a separately frozen authored-selection
//! frontier; selection visibility alone cannot discharge helper contracts.

use build_time_evaluation::PreparedBuildMachineProgram;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;

pub(crate) fn prepare_checked_build(
    typed: &typed_trees::TypedTrees,
    target_machine_declarations: &provider_planning::target_machines::SelectedTargetMachineDeclarations,
    packages: Option<&PackageCompilationInputs>,
    execution_profile: Option<target::TargetProfile>,
    timings: &mut artifacts::compile_timings::CompileTimings,
) -> Result<PreparedBuildMachineProgram, Vec<Diagnostic>> {
    // Package build execution can carry filesystem and other boundary
    // authority. Check the frozen ordinary source graph first; the ordinary
    // final checked pass repeats this gate after any explicit generated-source
    // handoff.
    // Other targets' sibling bodies remain for the settled product check;
    // the Build activation uses its already-selected execution-profile body.
    // Its result feeds Build execution, not the later settled check: the settled
    // program differs (the build may append generated source, target
    // selection swaps canonical bodies, provider settlement records erasure
    // authorizations, and const folds land the endpoints only the
    // preliminary mode tolerates pending), and the settled request closes
    // those tolerances and carries the specialization rosters this request
    // leaves empty. Only the exact-input check memo deduplicates it — for a
    // later pass that arrives at this same frozen graph under this same
    // request.
    let mut provisional = typed.clone();
    typed_trees_to_checked_trees::prune_target_siblings_typed(&mut provisional);
    let mut checked = timings.record(
        artifacts::compile_timings::TYPED_TREES_TO_PRELIMINARY_CHECKED_TREES,
        || {
            let started = std::time::Instant::now();
            let result = typed_trees_to_checked_trees::lower_typed_trees(
                provisional,
                &typed_trees_to_checked_trees::CheckingRequest::preliminary()
                    .with_declaration_authority(
                        packages
                            .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
                    ),
            );
            artifacts::compile_timings::counters::record_preliminary_check(started.elapsed());
            result
        },
    )?;
    let context = provider_planning::ProviderPlanningContext::new(
        &checked.typed,
        source::DependencyScope::Build,
    );
    let declarations = target_machine_declarations.settle_provider_defaults(context)?;
    let bindings = provider_planning::evaluated_via_bindings::evaluate_via_bindings(
        context,
        execution_profile,
        packages,
    )?;
    let derivation = provider_planning::ProviderPlanDerivation::evaluated(
        context,
        execution_profile.map(target::TargetProfile::target_name),
        &bindings,
        &declarations.origins,
    )?;
    let candidates = provider_planning::derive_satisfies_plans(derivation);
    let diagnostics = provider_planning::validate_derived_provider_plan_candidates(
        context,
        &bindings,
        &candidates,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let provenance = if candidates.is_empty() && declarations.provider_defaults.is_empty() {
        Vec::new()
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
        let (_, provenance) =
            provider_planning::selected_provider_plan_facts(context, &bindings, selected)?;
        provenance
    };
    selected_dispatch::settle_selected_comparison_execution(
        &mut checked,
        source::DependencyScope::Build,
        &provenance,
    )?;
    Ok(PreparedBuildMachineProgram::from_checked(checked))
}
