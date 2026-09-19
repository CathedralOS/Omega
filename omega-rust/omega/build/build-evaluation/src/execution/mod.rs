//! Executes one admitted build activation and settles its captured outputs.
mod machine_evaluation;
mod observation_projection;
mod output_custody;

use crate::admission::configuration::{BuildConfig, extract_build_config};
use crate::admission::declarations::{
    harvest_behavior_exclusions, harvest_provider_selections, harvest_root_grants,
    harvest_wire_compatibility_demands,
};
use crate::admission::selection::root_bindings::collect_root_bindings;
use crate::admitted_build_program::{AdmittedBuildMachine, SelectedAdmittedBuildMachine};
use crate::evidence::observations::{BUILD_OBSERVATION_SCHEMA_VERSION, BuildObservationSummary};
use crate::{AdmittedBuildProgram, ComputedBuildConfig};
use diagnostics::Diagnostic;

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
        selected_target_profile,
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
        target_vocabulary,
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
    let filesystem_operation_schema_version = measured
        .observations()
        .filesystem_operation_schema_version();
    let filesystem_operation_attempts =
        observation_projection::project_filesystem_operation_attempts(
            measured.observations(),
            &machine_name,
        )?;
    let included_source_handoffs = observation_projection::project_included_source_handoffs(
        measured.observations(),
        filesystem_operation_attempts.len(),
        &machine_name,
    )?;
    let filesystem_host_observed = measured.observations().filesystem_host_observed();
    let build_log = measured.observations().build_log().to_vec();
    let output_obligations = measured.observations().build_output_obligations().to_vec();
    let output_receipts = measured.observations().build_output_receipts().to_vec();
    let root_bindings = collect_root_bindings(typed, measured.executed_root_bindings())?;
    let executed_exclusions = measured.executed_behavior_exclusions().to_vec();

    // Phase 3: read the augmented `Build` back and harvest the declared
    // grants, selections, demands, and exclusions into the configuration.
    let mut arguments = measured.into_value();
    let augmented = arguments.pop().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` returned no argument values (expected the augmented Build)"
        ))]
    })?;

    let (mut config, optimization_report) = extract_build_config(
        &augmented,
        optimization_admission,
        selected_target_profile,
        target_vocabulary.is_some(),
    )
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "`{machine_name}` produced an invalid Build: {reason}"
        ))]
    })?;
    config.grants = harvest_root_grants(typed, machine).map_err(|diagnostic| vec![diagnostic])?;
    config.provider_selections = harvest_provider_selections(typed, machine)?;
    config.opaque_representation_selections =
        representation_planning::harvest_opaque_representation_selections(typed, machine)?;
    config.wire_compatibility_demands = harvest_wire_compatibility_demands(typed, machine)?;
    config.behavior_exclusions = harvest_behavior_exclusions(typed, &executed_exclusions)?;
    config.root_bindings = root_bindings;

    // Capture the actual execution's staged output, then check host reach.
    let staged_output_tree = filesystem_scope.staged_output_tree(filesystem_reachable)?;
    if filesystem_host_observed && !filesystem_reachable {
        return Err(vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` observed filesystem host state without admitted filesystem reach"
        ))]);
    }

    // Phase 4: settle every required output against sealed staged custody
    // before any generated source is selected or the result may publish.
    filesystem_scope.verify_required_outputs(staged_output_tree.as_ref(), &machine_name)?;
    let required_output_settlements = output_custody::settle_build_output_obligations(
        &output_obligations,
        &output_receipts,
        staged_output_tree.as_ref(),
        &machine_name,
    )?;
    if artifact_only {
        if !config.root_bindings.is_empty() || !config.provider_selections.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "artifact-only build `{machine_name}` may not bind executable roots or select boundary providers"
            ))]);
        }
        if required_output_settlements.is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "artifact-only build `{machine_name}` completed no required output"
            ))]);
        }
    }
    let generated_sources = output_custody::select_generated_sources(
        staged_output_tree.as_ref(),
        &included_source_handoffs,
        &machine_name,
    )?;

    // Phase 5: assemble the checked result.
    Ok(ComputedBuildConfig {
        config,
        optimization_report_request: optimization_report,
        evaluation_usage: Some(machine_evaluation::evaluation_usage(
            usage,
            evaluation_sponsor,
        )),
        observation_summary: Some(BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            filesystem_host_observed,
            filesystem_operation_schema_version,
            filesystem_operation_attempts,
            canonical_source_metadata_identity: filesystem_scope
                .canonical_source_metadata_identity(),
            activation: filesystem_scope.activation(selected_target_profile),
            captured_source_inventory: filesystem_scope.captured_source_inventory(),
            included_source_handoffs,
            required_output_settlements,
            staged_output_tree,
            build_log,
        }),
        selected_build_machine_symbol: Some(machine.symbol),
        generated_sources,
    })
}
