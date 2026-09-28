//! Executes one admitted build activation and settles its captured outputs.
mod machine_evaluation;

use crate::admission::configuration::{BuildConfig, extract_build_config};
use crate::admission::selection::root_bindings::collect_root_bindings;
use crate::admitted_build_program::{AdmittedBuildMachine, SelectedAdmittedBuildMachine};
use crate::{AdmittedBuildProgram, ComputedBuildConfig};
use build_time_evaluation::wire_compatibility::harvest_wire_compatibility_demands;
use diagnostics::Diagnostic;
use package_compilation::CapturedBuildObservations;
use trust_model::{
    harvest_behavior_exclusions, harvest_component_assumption_acceptances, harvest_root_grants,
};

/// Consume one admitted build activation and extract its durable configuration
/// and evaluation evidence.
pub fn execute_admitted_build_program(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    // The private captured-source backing is scratch for this occurrence
    // only. Settlement releases it before staged custody is captured; every
    // other exit (an evaluator halt, a rejected Build, a failed settlement)
    // must release it as well, so a failed build leaves no residue outside
    // its build directory.
    let captured_snapshot = admitted.filesystem_scope.captured_snapshot_release();
    let outcome = execute_admitted_build_occurrence(admitted);
    captured_snapshot.release();
    outcome
}

fn execute_admitted_build_occurrence(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    let AdmittedBuildProgram {
        prepared,
        machine,
        operational_plan: _,
        service_reach_plan: _,
        filesystem_scope,
        evaluation_sponsor,
        artifact_only,
    } = admitted;
    let AdmittedBuildMachine::Selected(selected) = machine else {
        if artifact_only {
            return Err(vec![Diagnostic::error(
                "an artifact-only application requires a build machine that completes at least one required output"
                    .to_owned(),
            )]);
        }
        return Ok(ComputedBuildConfig {
            config: BuildConfig::default(),
            optimization_report_request: optimization_core::OptimizationReportRequest::Suppressed,
            evaluation_usage: None,
            observation_summary: None,
            selected_build_machine_symbol: None,
            generated_sources: Vec::new(),
        });
    };
    let SelectedAdmittedBuildMachine {
        entry: machine_entry,
        symbol: machine_symbol,
        name: machine_name,
        normalized_callable_identity: _,
        optimization_admission,
        has_toolchain_build_activation,
        filesystem_reachable,
        execution_mode,
        initial_build,
    } = selected;
    let typed = prepared.typed();
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("admitted build entry remains in its owned prepared program");
    let initial_arguments = vec![initial_build];
    let evaluation_sponsor = evaluation_sponsor.as_ref();

    // Phase 1: run the admitted machine once in its admitted execution mode.
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
    let provider_selections = provider_planning::collect_executed_provider_selections(
        typed,
        measured.executed_provider_selections(),
    )?;

    // Phase 3: join the augmented Build and declared grants/demands with
    // the selections and exclusions recorded by this execution.
    let mut arguments = measured.into_value();
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    let (mut config, optimization_report) = extract_build_config(
        &augmented,
        optimization_admission,
        has_toolchain_build_activation,
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` produced an invalid Build: {reason}"
        ))]
    })?;
    config.grants = harvest_root_grants(typed, machine).map_err(|diagnostic| vec![diagnostic])?;
    config.provider_selections = provider_selections;
    config.opaque_representation_selections =
        representation_planning::harvest_opaque_representation_selections(typed, machine)?;
    config.wire_compatibility_demands = harvest_wire_compatibility_demands(typed, machine)?;
    config.behavior_exclusions = harvest_behavior_exclusions(typed, &executed_exclusions)?;
    config.accepted_component_assumptions =
        harvest_component_assumption_acceptances(typed, machine)?;
    config.root_bindings = root_bindings;

    // Phase 4: settle outputs before artifact admission or source selection.
    let observation_summary = observations.settle(filesystem_reachable, &machine_name)?;
    if artifact_only {
        if !config.root_bindings.is_empty() || !config.provider_selections.is_empty() {
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

    // Phase 5: assemble the checked result.
    Ok(ComputedBuildConfig {
        config,
        optimization_report_request: optimization_report,
        evaluation_usage: Some(checked_interpreter::BuildEvaluationUsage::capture(
            usage,
            evaluation_sponsor,
        )),
        observation_summary: Some(observation_summary),
        selected_build_machine_symbol: Some(machine.symbol),
        generated_sources,
    })
}
