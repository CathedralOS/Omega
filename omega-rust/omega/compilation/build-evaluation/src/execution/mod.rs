//! Executes one admitted build activation and settles its captured outputs.
mod machine_evaluation;

use crate::{AdmittedBuildProgram, BuildConfig, ComputedBuildConfig};
use build_time_evaluation::harvest_wire_compatibility_demands;
use diagnostics::Diagnostic;
use package_compilation::{CapturedBuildObservations, SelectedBuildMachine};
use provider_planning::program_entry::collect_root_bindings;
use trust_model::{
    harvest_behavior_exclusions, harvest_component_assumption_acceptances, harvest_root_grants,
};

/// Consume one admitted build activation and extract its durable configuration
/// and evaluation evidence.
pub(crate) fn execute_admitted_build_program(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    let AdmittedBuildProgram {
        prepared,
        machine,
        filesystem_scope,
        evaluation_sponsor,
        artifact_only,
    } = admitted;
    let SelectedBuildMachine {
        entry: machine_entry,
        symbol: machine_symbol,
        name: machine_name,
        normalized_callable_identity: _,
        value_admission,
        filesystem_reachable,
        execution_mode,
        initial_build,
    } = machine;
    let typed = prepared.typed();
    let machine = typed
        .machine(machine_symbol)
        .expect("admitted build entry remains in its owned prepared program");
    let initial_arguments = vec![initial_build];
    let evaluation_sponsor = evaluation_sponsor.as_ref();

    // Phase 1: run the admitted machine once in its admitted execution mode.
    artifacts::compile_timings::counters::record_build_execution();
    let measured = machine_evaluation::evaluate_admitted_machine(
        &prepared,
        &machine_entry,
        initial_arguments,
        execution_mode,
        evaluation_sponsor,
        &machine_name,
    )?;
    let usage = measured.usage();

    // Phase 2: project the retained observations into their durable rows.
    let observations = CapturedBuildObservations::capture(
        measured.observations(),
        &filesystem_scope,
        &machine_name,
    )?;
    let root_bindings = collect_root_bindings(typed, measured.executed_root_bindings())?;
    let executed_exclusions = measured.executed_behavior_exclusions().to_vec();
    let wire_compatibility_demands =
        harvest_wire_compatibility_demands(typed, measured.executed_wire_demands())?;
    let provider_selections =
        build_time_evaluation::provider_declarations::collect_executed_provider_selections(
            provider_planning::ProviderPlanningContext::new(
                typed,
                source::DependencyScope::Product,
            ),
            measured.executed_build_selections(),
        )?;

    let assumed_bindings =
        build_time_evaluation::assumed_bindings::collect_executed_assumed_bindings(
            typed,
            machine_symbol,
            measured.executed_build_selections(),
        )?;

    // Phase 3: join the augmented Build and declared grants/demands with
    // the selections and exclusions recorded by this execution.
    let mut arguments = measured.into_value();
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    let (settings, optimization_report) =
        value_admission.extract(&augmented).map_err(|reason| {
            vec![Diagnostic::error(format!(
                "`{machine_name}` produced an invalid Build: {reason}"
            ))]
        })?;
    let mut config = BuildConfig {
        settings,
        ..Default::default()
    };
    config.grants = harvest_root_grants(typed, machine).map_err(|diagnostic| vec![diagnostic])?;
    config.provider_selections = provider_selections;
    config.assumed_bindings = assumed_bindings;
    config.opaque_representation_selections =
        representation_planning::harvest_opaque_representation_selections(typed, machine)?;
    config.wire_compatibility_demands = wire_compatibility_demands;
    config.behavior_exclusions = harvest_behavior_exclusions(typed, &executed_exclusions)?;
    config.accepted_component_assumptions =
        harvest_component_assumption_acceptances(typed, machine)?;
    config.root_bindings = root_bindings;

    // Phase 4: settle outputs before artifact admission or source selection.
    let observation_summary = observations.settle(filesystem_reachable, &machine_name)?;
    if artifact_only {
        if !config.root_bindings.is_empty()
            || !config.provider_selections.is_empty()
            || !config.assumed_bindings.is_empty()
        {
            return Err(vec![Diagnostic::error(format!(
                "artifact-only build `{machine_name}` may not bind executable roots or select boundary providers"
            ))]);
        }
        if observation_summary.required_output_settlements().is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "artifact-only build `{machine_name}` completed no required output"
            ))]);
        }
    }
    let generated_sources = observation_summary.select_generated_sources(&machine_name)?;

    // Phase 5: retain only the actual engine's measured usage. A sponsor for
    // another measurement family cannot give this legacy tree run its units.
    let evaluation_usage =
        checked_interpreter::capture_checked_build_evaluation_usage(usage, evaluation_sponsor)
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "build `{machine_name}` evaluation accounting is inconsistent: {error:?}"
                ))]
            })?;
    Ok(ComputedBuildConfig {
        config,
        optimization_report_request: optimization_report,
        evaluation_usage: Some(evaluation_usage),
        observation_summary: Some(observation_summary),
        selected_build_machine_symbol: Some(machine.symbol),
        generated_sources,
    })
}
