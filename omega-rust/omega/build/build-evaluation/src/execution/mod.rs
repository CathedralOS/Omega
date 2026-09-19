//! Execution: running one admitted build program, checking replay and
//! output custody, and assembling the checked result.
//!
//! `execute_admitted_build_program` is the route. It runs the admitted machine
//! once, replays it when the observed filesystem shape is replayable, projects
//! the retained observations, extracts the augmented `Build`, reconciles
//! staged-output custody, settles the required outputs, and assembles the
//! [`ComputedBuildConfig`]. Each phase has one owner beside this file:
//! `machine_evaluation.rs` runs and replays the machine,
//! `filesystem_replay.rs` recognizes the replayable observation shape,
//! `observation_projection.rs` projects attempts and handoffs into observation
//! rows, and `output_custody.rs` reconciles custody, settles obligations, and
//! selects generated sources.

mod filesystem_replay;
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
use crate::evidence::observations::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemReplayDisposition,
    BuildFilesystemReplayVerdict, BuildObservationClass, BuildObservationSummary,
};
use crate::evidence::replay_eligibility::receipted_output_entries;
use crate::{AdmittedBuildProgram, ComputedBuildConfig};
use diagnostics::Diagnostic;

/// Consume one admitted build activation and extract its durable configuration
/// and evaluation evidence.
pub fn execute_admitted_build_program(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    // The private captured-source backing is scratch for this occurrence
    // only. Settlement releases it before staged custody is captured; every
    // other exit (an evaluator halt, a rejected Build, a failed replay or
    // settlement) must release it as well, so a failed build leaves no
    // residue outside its build directory.
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
        initial_arguments.clone(),
        execution_mode,
        evaluation_sponsor,
        &machine_name,
    )?;
    let usage = measured.usage();

    // Phase 2: replay it exactly when the observed filesystem shape is
    // replayable, keeping the receipts the replay leaves for output custody.
    let replay = if filesystem_reachable {
        filesystem_replay::recognize_filesystem_replay(measured.observations())
    } else {
        None
    };
    let replay_has_no_output_attempts = replay
        .as_ref()
        .is_some_and(|replay| !replay.has_output_attempts());
    let replay_includes_complete_no_output_failure = replay.is_some()
        && filesystem_replay::includes_complete_no_output_failure(measured.observations());
    let receipted_output_entries = replay.as_ref().and_then(receipted_output_entries);
    let replay_usage = replay
        .map(|replay| {
            machine_evaluation::replay_admitted_machine(
                &prepared,
                &machine_entry,
                initial_arguments,
                replay,
                evaluation_sponsor,
                &machine_name,
                &measured,
            )
        })
        .transpose()?;
    let source_inputs_replayed = replay_usage.is_some();
    let replayed_output_tree = filesystem_replay::replayed_output_tree_from_receipts(
        replay_has_no_output_attempts,
        &receipted_output_entries,
    )?;
    let observation_ceiling = if filesystem_reachable {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };

    // Phase 3: project the retained observations into their durable rows.
    let filesystem_operation_schema_version = measured
        .observations()
        .filesystem_operation_schema_version();
    let filesystem_operation_attempts =
        observation_projection::project_filesystem_operation_attempts(
            measured.observations(),
            &machine_name,
            source_inputs_replayed,
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

    // Phase 4: read the augmented `Build` back and harvest the declared
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

    // Phase 5: reconcile staged-output custody with the replay and check the
    // realized observation against the static ceiling.
    let captured_output_tree = filesystem_scope.staged_output_tree(filesystem_reachable)?;
    let output_custody::StagedOutputCustody {
        tree: staged_output_tree,
        complete_replay_verified,
    } = output_custody::reconcile_staged_output(
        replayed_output_tree,
        captured_output_tree,
        filesystem_scope.is_replay(),
        replay_includes_complete_no_output_failure,
        source_inputs_replayed,
        &machine_name,
    )?;
    let filesystem_replay_verdict =
        BuildFilesystemReplayVerdict::new(if complete_replay_verified {
            BuildFilesystemReplayDisposition::Complete
        } else if source_inputs_replayed {
            BuildFilesystemReplayDisposition::SourceInputsOnly
        } else {
            BuildFilesystemReplayDisposition::NotReplayed
        });
    let realized_observation = if filesystem_replay_verdict.is_complete() {
        BuildObservationClass::Receipted
    } else if filesystem_host_observed {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };
    if realized_observation > observation_ceiling {
        return Err(vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` observed filesystem host state outside its static observation ceiling"
        ))]);
    }

    // Phase 6: settle every required output against sealed staged custody
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

    // Phase 7: assemble the checked result.
    Ok(ComputedBuildConfig {
        config,
        optimization_report_request: optimization_report,
        evaluation_usage: Some(machine_evaluation::evaluation_usage(
            usage,
            replay_usage,
            evaluation_sponsor,
        )),
        observation_summary: Some(BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            ceiling: observation_ceiling,
            realized: realized_observation,
            filesystem_operation_schema_version,
            filesystem_operation_attempts,
            canonical_source_metadata_identity: filesystem_scope
                .canonical_source_metadata_identity(),
            replay_activation: filesystem_scope.activation(selected_target_profile),
            captured_source_inventory: filesystem_scope.captured_source_inventory(),
            filesystem_replay_verdict,
            included_source_handoffs,
            required_output_settlements,
            staged_output_tree,
            build_log,
        }),
        selected_build_machine_symbol: Some(machine.symbol),
        generated_sources,
    })
}
