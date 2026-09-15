//! Execution: running one admitted build program, checking replay and
//! output custody, and assembling the checked result.

use crate::admitted_build_program::{AdmittedBuildMachine, SelectedAdmittedBuildMachine};
use crate::configuration::{BuildConfig, extract_build_config};
use crate::declarations::{
    harvest_behavior_exclusions, harvest_provider_selections, harvest_root_grants,
    harvest_wire_compatibility_demands,
};
use crate::filesystem_scope::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY};
use crate::observations::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildEvaluationUsage, BuildFilesystemAuthorizedPath,
    BuildFilesystemByteOperand, BuildFilesystemGrantAccess, BuildFilesystemGrantRefusal,
    BuildFilesystemGrantRefusalReason, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemMetadataObservation,
    BuildFilesystemMetadataObservationKind, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemMutableI64Operand,
    BuildFilesystemMutableI64OperandResolution, BuildFilesystemObservedByteRegion,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationAttempt,
    BuildFilesystemOperationObservationClass, BuildFilesystemPathLikeOperand,
    BuildFilesystemProvider, BuildFilesystemReplayDisposition, BuildFilesystemReplayVerdict,
    BuildFilesystemReturnedPath, BuildFilesystemReturnedPathCompleteness,
    BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildIncludedSourceHandoff, BuildObservationClass, BuildObservationSummary,
    BuildRequiredOutputSettlement, project_logical_handle_identity,
    project_logical_handle_input_resolution, project_logical_handle_kind,
    project_logical_handle_output_source, project_operation_result, project_scalar_operand_value,
};
use crate::replay_eligibility::{
    ReceiptedOutputEntry, ReceiptedOutputFile, complete_no_output_failure_suffix_is_recognized,
    errno_tag, exact_source_write_refusal, get_last_error_tag, is_source_input_replay_record,
    operand_free_unknown_descriptor_operation_tag, receipted_output_entries,
    source_input_replay_prefix_end, unknown_descriptor_bad_descriptor_failure_tag,
    unknown_descriptor_get_osfhandle_tag, unknown_descriptor_open_at_tag,
    unknown_descriptor_read_dir_tag, unknown_descriptor_read_file_metadata_tag,
    unknown_descriptor_read_operation_tag, unknown_descriptor_set_file_times_tag,
    unknown_descriptor_unlink_at_tag, unknown_descriptor_write_operation_tag,
    unknown_descriptor_write_payload_operation_tag, unknown_native_handle_close_tag,
    unknown_native_handle_final_path_tag, unknown_native_handle_mutation_tag,
};
use crate::selection::root_bindings::collect_root_bindings;
use crate::{AdmittedBuildProgram, ComputedBuildConfig};
use build_output::{
    BuildStagedOutputEntryKind, BuildStagedOutputTree, ReplayedBuildOutputEntry, empty,
    replayed_output_tree, select_included_sources,
};
use build_time_evaluation::{
    BuildEvaluationSponsor, BuildMachineExecutionMode, BuildMachineFilesystemAccess,
    BuildMachineFilesystemMetadataLayout,
};
use diagnostics::Diagnostic;

/// Consume one admitted build activation and extract its durable configuration
/// and evaluation evidence.
pub fn execute_admitted_build_program(
    admitted: AdmittedBuildProgram,
) -> Result<ComputedBuildConfig, Vec<Diagnostic>> {
    let AdmittedBuildProgram {
        prepared,
        machine,
        operational_plan,
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
    let measured = match evaluation_sponsor {
        Some(sponsor) => {
            build_time_evaluation::evaluate_build_machine_entry_arguments_measured_with_sponsor(
                &prepared,
                &machine_entry,
                initial_arguments.clone(),
                execution_mode.clone(),
                sponsor,
            )
        }
        None => build_time_evaluation::evaluate_build_machine_entry_arguments_measured(
            &prepared,
            &machine_entry,
            initial_arguments.clone(),
            execution_mode,
        ),
    }
    .map_err(|reason| {
        let partial_evidence = reason
            .observations()
            .filter(|observations| !observations.filesystem_operation_attempts().is_empty())
            .map(|observations| {
                let attempts = observations.filesystem_operation_attempts();
                let halted = attempts
                    .iter()
                    .filter(|attempt| {
                        matches!(
                            attempt.outcome(),
                            Some(checked_interpreter::FilesystemOperationAttemptOutcome::EvaluationHalted(_))
                        )
                    })
                    .count();
                let grant_refusals = attempts
                    .iter()
                    .map(|attempt| attempt.grant_refusals().len())
                    .sum::<usize>();
                let scalar_operands = attempts
                    .iter()
                    .map(|attempt| attempt.scalar_operands().len())
                    .sum::<usize>();
                let byte_operands = attempts
                    .iter()
                    .map(|attempt| attempt.byte_operands().len())
                    .sum::<usize>();
                let path_like_operands = attempts
                    .iter()
                    .map(|attempt| attempt.path_like_operands().len())
                    .sum::<usize>();
                let logical_handle_operands = attempts
                    .iter()
                    .map(|attempt| attempt.logical_handle_inputs().len())
                    .sum::<usize>();
                let mutable_carrier_operands = attempts
                    .iter()
                    .map(|attempt| {
                        attempt.mutable_byte_operand_resolutions().len()
                            + attempt.mutable_i64_operand_resolutions().len()
                    })
                    .sum::<usize>();
                let rooted_path_operands = attempts
                    .iter()
                    .map(|attempt| attempt.rooted_path_operand_resolutions().len())
                    .sum::<usize>();
                format!(
                    "; partial non-admission filesystem evidence: {} call(s), {halted} evaluator-halted, {grant_refusals} grant refusal(s), {scalar_operands} scalar operand(s), {byte_operands} immutable byte operand(s), {path_like_operands} path-like operand(s), {rooted_path_operands} rooted-path operand(s), {logical_handle_operands} logical-handle operand(s), {mutable_carrier_operands} mutable-carrier operand(s)",
                    attempts.len()
                )
            })
            .unwrap_or_default();
        vec![Diagnostic::error(format!(
            "build-time evaluation of `{machine_name}` failed: {reason}{partial_evidence}"
        ))]
    })?;
    let usage = measured.usage();
    let replay = if filesystem_reachable {
        let attempts = measured.observations().filesystem_operation_attempts();
        if let Some(operation_suffix_start) =
            source_input_replay_prefix_end(attempts).filter(|end| *end < attempts.len())
        {
            if operation_suffix_start == 0
                && attempts.len() == 1
                && exact_source_write_refusal(&attempts[0])
            {
                checked_interpreter::FilesystemReplay::from_source_write_refusal_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 2
                && unknown_descriptor_bad_descriptor_failure_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
                && errno_tag(attempts[operation_suffix_start + 1].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_failure_with_errno_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && operand_free_unknown_descriptor_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_operation_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && attempts[operation_suffix_start].operation_tag() == 10
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_seek_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_open_at_tag(attempts[operation_suffix_start].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_open_at_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_unlink_at_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_read_dir_tag(attempts[operation_suffix_start].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_write_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_set_file_times_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_read_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_write_payload_operation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_read_file_metadata_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_descriptor_get_osfhandle_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_native_handle_close_tag(attempts[operation_suffix_start].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_native_handle_final_path_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 1
                && unknown_native_handle_mutation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_mutation_observations(
                    measured.observations(),
                )
                .ok()
            } else if attempts.len() - operation_suffix_start == 2
                && (unknown_native_handle_close_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ) || unknown_native_handle_final_path_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ) || unknown_native_handle_mutation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ))
                && get_last_error_tag(attempts[operation_suffix_start + 1].operation_tag())
            {
                checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_failure_with_last_error_observations(
                    measured.observations(),
                )
                .ok()
            } else {
                checked_interpreter::FilesystemReplay::from_input_output_observations(
                    measured.observations(),
                )
                .ok()
            }
        } else {
            is_source_input_replay_record(measured.observations())
                .then(|| {
                    checked_interpreter::FilesystemReplay::from_source_input_observations(
                        measured.observations(),
                    )
                })
                .and_then(Result::ok)
        }
    } else {
        None
    };
    let replay_has_no_output_attempts = replay
        .as_ref()
        .is_some_and(|replay| !replay.has_output_attempts());
    let replay_includes_complete_no_output_failure = replay.is_some()
        && source_input_replay_prefix_end(measured.observations().filesystem_operation_attempts())
            .is_some_and(|suffix_start| {
                complete_no_output_failure_suffix_is_recognized(
                    measured.observations().filesystem_operation_attempts(),
                    suffix_start,
                )
            });
    let receipted_output_entries = replay.as_ref().and_then(receipted_output_entries);
    let replay_usage = if let Some(replay) = replay {
        let replay_mode = BuildMachineExecutionMode::Granted {
            filesystem: BuildMachineFilesystemAccess::ReplayFilesystem(replay),
            filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout::default(),
        };
        let replayed = match evaluation_sponsor {
            Some(sponsor) => {
                build_time_evaluation::evaluate_build_machine_entry_arguments_measured_with_sponsor(
                    &prepared,
                    &machine_entry,
                    initial_arguments,
                    replay_mode,
                    sponsor,
                )
            }
            None => build_time_evaluation::evaluate_build_machine_entry_arguments_measured(
                &prepared,
                &machine_entry,
                initial_arguments,
                replay_mode,
            ),
        }
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "build-time replay of `{machine_name}` failed: {reason}"
            ))]
        })?;
        if replayed.value() != measured.value()
            || replayed.observations() != measured.observations()
            || replayed.executed_root_bindings() != measured.executed_root_bindings()
        {
            return Err(vec![Diagnostic::error(format!(
                "build-time replay of `{machine_name}` changed its result or operation record"
            ))]);
        }
        Some(replayed.usage())
    } else {
        None
    };
    let source_inputs_replayed = replay_usage.is_some();
    let replayed_output_tree = if replay_has_no_output_attempts {
        Some(empty())
    } else {
        receipted_output_entries
            .as_ref()
            .map(|entries| {
                let mut regular_files = std::collections::BTreeMap::new();
                let mut normalized_entries = Vec::with_capacity(entries.len());
                for entry in entries {
                    let normalized = match entry {
                        ReceiptedOutputEntry::Directory { .. }
                        | ReceiptedOutputEntry::Symlink { .. } => entry.clone(),
                        ReceiptedOutputEntry::File(file) => {
                            regular_files.insert(
                                file.relative_path.clone(),
                                (file.bytes.clone(), file.executable),
                            );
                            entry.clone()
                        }
                        ReceiptedOutputEntry::HardLink {
                            existing_relative_path,
                            relative_path,
                        } => {
                            let (bytes, executable) = regular_files
                                .get(existing_relative_path)
                                .expect("validated hard link follows a regular-file name")
                                .clone();
                            regular_files
                                .insert(relative_path.clone(), (bytes.clone(), executable));
                            ReceiptedOutputEntry::File(ReceiptedOutputFile {
                                relative_path: relative_path.clone(),
                                bytes,
                                executable,
                            })
                        }
                    };
                    normalized_entries.push(normalized);
                }
                let replayed_entries = normalized_entries
                    .iter()
                    .map(|entry| match entry {
                        ReceiptedOutputEntry::Directory { relative_path } => {
                            ReplayedBuildOutputEntry::directory(relative_path)
                        }
                        ReceiptedOutputEntry::File(file) => ReplayedBuildOutputEntry::regular_file(
                            &file.relative_path,
                            &file.bytes,
                            file.executable,
                        ),
                        ReceiptedOutputEntry::Symlink {
                            relative_path,
                            target_spelling,
                        } => {
                            ReplayedBuildOutputEntry::symbolic_link(relative_path, target_spelling)
                        }
                        ReceiptedOutputEntry::HardLink { .. } => {
                            unreachable!("hard links are normalized before staged commitment")
                        }
                    })
                    .collect::<Vec<_>>();
                replayed_output_tree(&replayed_entries)
            })
            .transpose()?
    };
    let observation_ceiling = if filesystem_reachable {
        BuildObservationClass::Volatile
    } else {
        BuildObservationClass::Hermetic
    };
    let filesystem_operation_schema_version = measured
        .observations()
        .filesystem_operation_schema_version();
    let filesystem_operation_attempts = measured
        .observations()
        .filesystem_operation_attempts()
        .iter()
        .map(|attempt| {
            let authorized_paths = attempt
                .authorized_paths()
                .iter()
                .map(|path| {
                    let root = if path.root() == BUILD_SOURCE_ROOT_IDENTITY {
                        BuildFilesystemRoot::Source
                    } else if path.root() == BUILD_OUTPUT_ROOT_IDENTITY {
                        BuildFilesystemRoot::Output
                    } else {
                        return Err(Diagnostic::error(format!(
                            "build-time evaluation of `{machine_name}` returned unknown filesystem grant-root identity `{}`",
                            path.root().get()
                        )));
                    };
                    Ok(BuildFilesystemAuthorizedPath {
                        operand_ordinal: path.operand_ordinal(),
                        access: match path.access() {
                            checked_interpreter::FilesystemGrantAccess::Read => {
                                BuildFilesystemGrantAccess::Read
                            }
                            checked_interpreter::FilesystemGrantAccess::Write => {
                                BuildFilesystemGrantAccess::Write
                            }
                        },
                        root,
                        relative_path: path.relative_path().to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let logical_handle_inputs = attempt
                .logical_handle_inputs()
                .iter()
                .map(|input| BuildFilesystemLogicalHandleInput {
                    operand_ordinal: input.operand_ordinal(),
                    kind: project_logical_handle_kind(input.kind()),
                    resolution: project_logical_handle_input_resolution(input.resolution()),
                })
                .collect();
            let scalar_operands = attempt
                .scalar_operands()
                .iter()
                .map(|operand| BuildFilesystemScalarOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    value: project_scalar_operand_value(operand.value()),
                })
                .collect();
            let byte_operands = attempt
                .byte_operands()
                .iter()
                .map(|operand| BuildFilesystemByteOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    bytes: operand.bytes().to_vec(),
                })
                .collect();
            let path_like_operands = attempt
                .path_like_operands()
                .iter()
                .map(|operand| BuildFilesystemPathLikeOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    bytes: operand.bytes().to_vec(),
                })
                .collect();
            let rooted_path_operand_resolutions = attempt
                .rooted_path_operand_resolutions()
                .iter()
                .map(|operand| {
                    let root = if operand.root() == BUILD_SOURCE_ROOT_IDENTITY {
                        BuildFilesystemRoot::Source
                    } else if operand.root() == BUILD_OUTPUT_ROOT_IDENTITY {
                        BuildFilesystemRoot::Output
                    } else {
                        return Err(Diagnostic::error(format!(
                            "build-time evaluation of `{machine_name}` returned unknown rooted-path operand identity `{}`",
                            operand.root().get()
                        )));
                    };
                    Ok(BuildFilesystemRootedPathOperandResolution {
                        operand_ordinal: operand.operand_ordinal(),
                        root,
                        relative_path: operand.relative_path().to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let returned_paths = attempt
                .returned_paths()
                .iter()
                .map(|returned| BuildFilesystemReturnedPath {
                    operand_ordinal: returned.operand_ordinal(),
                    kind: match returned.kind() {
                        checked_interpreter::FilesystemReturnedPathKind::ReadLinkPayload => {
                            BuildFilesystemReturnedPathKind::ReadLinkPayload
                        }
                        checked_interpreter::FilesystemReturnedPathKind::CanonicalPath => {
                            BuildFilesystemReturnedPathKind::CanonicalPath
                        }
                        checked_interpreter::FilesystemReturnedPathKind::FinalPath => {
                            BuildFilesystemReturnedPathKind::FinalPath
                        }
                    },
                    completeness: match returned.completeness() {
                        checked_interpreter::FilesystemReturnedPathCompleteness::Complete => {
                            BuildFilesystemReturnedPathCompleteness::Complete
                        }
                        checked_interpreter::FilesystemReturnedPathCompleteness::LimitReached => {
                            BuildFilesystemReturnedPathCompleteness::LimitReached
                        }
                    },
                    bytes: returned.bytes().to_vec(),
                })
                .collect();
            let observed_byte_regions = attempt
                .observed_byte_regions()
                .iter()
                .map(|region| {
                    Ok(BuildFilesystemObservedByteRegion {
                        output_operand_ordinal: region.output_operand_ordinal(),
                        kind: match region.kind() {
                            checked_interpreter::FilesystemObservedByteRegionKind::SequentialFileRead => {
                                BuildFilesystemObservedByteRegionKind::SequentialFileRead
                            }
                            checked_interpreter::FilesystemObservedByteRegionKind::PositionedFileRead => {
                                BuildFilesystemObservedByteRegionKind::PositionedFileRead
                            }
                            checked_interpreter::FilesystemObservedByteRegionKind::DirectoryRecords => {
                                BuildFilesystemObservedByteRegionKind::DirectoryRecords
                            }
                            checked_interpreter::FilesystemObservedByteRegionKind::FindEntry => {
                                BuildFilesystemObservedByteRegionKind::FindEntry
                            }
                        },
                        offset: u64::try_from(region.offset()).map_err(|_| {
                            Diagnostic::error(
                                "build observation byte-region offset is not canonically representable",
                            )
                        })?,
                        length: u64::try_from(region.length()).map_err(|_| {
                            Diagnostic::error(
                                "build observation byte-region length is not canonically representable",
                            )
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let metadata_observations = attempt
                .metadata_observations()
                .iter()
                .map(|observation| BuildFilesystemMetadataObservation {
                    output_operand_ordinal: observation.output_operand_ordinal(),
                    kind: match observation.kind() {
                        checked_interpreter::FilesystemMetadataObservationKind::FollowedPath => {
                            BuildFilesystemMetadataObservationKind::FollowedPath
                        }
                        checked_interpreter::FilesystemMetadataObservationKind::OpenDescriptor => {
                            BuildFilesystemMetadataObservationKind::OpenDescriptor
                        }
                        checked_interpreter::FilesystemMetadataObservationKind::UnfollowedFinalPath => {
                            BuildFilesystemMetadataObservationKind::UnfollowedFinalPath
                        }
                    },
                    device: observation.device(),
                    mode: observation.mode(),
                    link_count: observation.link_count(),
                    inode: observation.inode(),
                    user: observation.user(),
                    group: observation.group(),
                    referenced_device: observation.referenced_device(),
                    access_time: observation.access_time(),
                    modification_time: observation.modification_time(),
                    change_time: observation.change_time(),
                    birth_time: observation.birth_time(),
                    size: observation.size(),
                    blocks_512: observation.blocks_512(),
                    preferred_block_size: observation.preferred_block_size(),
                })
                .collect();
            let mutable_byte_operand_resolutions = attempt
                .mutable_byte_operand_resolutions()
                .iter()
                .map(|operand| BuildFilesystemMutableByteOperandResolution {
                    operand_ordinal: operand.operand_ordinal(),
                    bytes: operand.bytes().to_vec(),
                })
                .collect();
            let mutable_i64_operand_resolutions = attempt
                .mutable_i64_operand_resolutions()
                .iter()
                .map(|operand| BuildFilesystemMutableI64OperandResolution {
                    operand_ordinal: operand.operand_ordinal(),
                    value: operand.value(),
                })
                .collect();
            let mutable_byte_operands = attempt
                .mutable_byte_operands()
                .iter()
                .map(|operand| BuildFilesystemMutableByteOperand {
                    operand_ordinal: operand.operand_ordinal(),
                    pre_bytes: operand.pre_bytes().to_vec(),
                    post_bytes: operand.post_bytes().to_vec(),
                })
                .collect();
            let mutable_i64_operands = attempt
                .mutable_i64_operands()
                .iter()
                .map(|operand| BuildFilesystemMutableI64Operand {
                    operand_ordinal: operand.operand_ordinal(),
                    pre_value: operand.pre_value(),
                    post_value: operand.post_value(),
                })
                .collect();
            let logical_handle_output = attempt.logical_handle_output().map(|output| {
                BuildFilesystemLogicalHandleOutput {
                    kind: project_logical_handle_kind(output.kind()),
                    identity: project_logical_handle_identity(output.identity()),
                    source: project_logical_handle_output_source(output.source()),
                }
            });
            let retired_logical_handles = attempt
                .retired_logical_handles()
                .iter()
                .copied()
                .map(project_logical_handle_identity)
                .collect();
            Ok(BuildFilesystemOperationAttempt {
                operation_tag: attempt.operation_tag(),
                provider: match attempt.provider() {
                checked_interpreter::FilesystemObservationProvider::Virtual => {
                    BuildFilesystemProvider::Virtual
                }
                checked_interpreter::FilesystemObservationProvider::RealUnscoped => {
                    BuildFilesystemProvider::RealUnscoped
                }
                checked_interpreter::FilesystemObservationProvider::RealScoped => {
                    BuildFilesystemProvider::RealScoped
                }
            },
                observation_class: if source_inputs_replayed {
                    BuildFilesystemOperationObservationClass::Receipted
                } else {
                    BuildFilesystemOperationObservationClass::Volatile
                },
                result: project_operation_result(
                    attempt
                        .result()
                        .expect("successful build evaluation cannot retain a halted filesystem call"),
                ),
                post_error: attempt
                    .post_error()
                    .expect("successful build evaluation cannot retain a halted filesystem call"),
                scalar_operands,
                byte_operands,
                path_like_operands,
                rooted_path_operand_resolutions,
                returned_paths,
                observed_byte_regions,
                metadata_observations,
                mutable_byte_operand_resolutions,
                mutable_i64_operand_resolutions,
                mutable_byte_operands,
                mutable_i64_operands,
                authorized_paths,
                logical_handle_inputs,
                logical_handle_output,
                retired_logical_handles,
                grant_refusals: attempt
                    .grant_refusals()
                    .iter()
                    .map(|refusal| BuildFilesystemGrantRefusal {
                        operand_ordinal: refusal.operand_ordinal(),
                        access: match refusal.access() {
                            checked_interpreter::FilesystemGrantAccess::Read => {
                                BuildFilesystemGrantAccess::Read
                            }
                            checked_interpreter::FilesystemGrantAccess::Write => {
                                BuildFilesystemGrantAccess::Write
                            }
                        },
                        reason: match refusal.reason() {
                            checked_interpreter::FilesystemGrantRefusalReason::Unresolvable => {
                                BuildFilesystemGrantRefusalReason::Unresolvable
                            }
                            checked_interpreter::FilesystemGrantRefusalReason::OutsideGrantedRoots => {
                                BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
                            }
                            checked_interpreter::FilesystemGrantRefusalReason::UnrepresentableRootedPath => {
                                BuildFilesystemGrantRefusalReason::UnrepresentableRootedPath
                            }
                            checked_interpreter::FilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => {
                                BuildFilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded
                            }
                        },
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    let included_source_handoffs = measured
        .observations()
        .build_included_sources()
        .iter()
        .map(|source| {
            if source.root() != BUILD_OUTPUT_ROOT_IDENTITY {
                return Err(Diagnostic::error(format!(
                    "build-time evaluation of `{machine_name}` handed off a generated source outside the compiler-issued Output root"
                )));
            }
            let filesystem_attempt_ordinal = u64::try_from(source.filesystem_attempt_ordinal())
                .map_err(|_| {
                    Diagnostic::error(format!(
                        "build-time evaluation of `{machine_name}` produced an included-source ordinal that exceeds canonical u64"
                    ))
                })?;
            if filesystem_attempt_ordinal > filesystem_operation_attempts.len() as u64 {
                return Err(Diagnostic::error(format!(
                    "build-time evaluation of `{machine_name}` handed off generated source `{}` at an ordinal outside this occurrence's filesystem attempt custody",
                    String::from_utf8_lossy(source.relative_path())
                )));
            }
            Ok(BuildIncludedSourceHandoff {
                relative_path: source.relative_path().to_vec(),
                filesystem_attempt_ordinal,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    let filesystem_host_observed = measured.observations().filesystem_host_observed();
    let build_log = measured.observations().build_log().to_vec();
    let output_obligations = measured.observations().build_output_obligations().to_vec();
    let output_receipts = measured.observations().build_output_receipts().to_vec();
    let root_bindings = collect_root_bindings(typed, measured.executed_root_bindings())?;
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
    config.behavior_exclusions = harvest_behavior_exclusions(typed, &operational_plan, machine)?;
    config.root_bindings = root_bindings;
    let captured_output_tree = filesystem_scope.staged_output_tree(filesystem_reachable)?;
    let (staged_output_tree, complete_replay_verified) = match (
        replayed_output_tree,
        captured_output_tree,
        filesystem_scope.is_replay(),
    ) {
        (Some(replayed), Some(captured), false) => {
            if replayed != captured {
                return Err(vec![Diagnostic::error(format!(
                    "build-time replay of `{machine_name}` reproduced an Output tree that differs from sponsored staged-output custody"
                ))]);
            }
            (Some(captured), true)
        }
        (Some(replayed), None, true) => (Some(replayed), true),
        (Some(replayed), None, false) if replay_includes_complete_no_output_failure => {
            (Some(replayed), true)
        }
        (Some(_), None, false) => (None, false),
        (Some(_), Some(_), true) => {
            unreachable!("replay scope cannot capture a physical staged-output tree")
        }
        (None, captured, _) => (captured, false),
    };
    if complete_replay_verified && !source_inputs_replayed {
        return Err(vec![Diagnostic::error(format!(
            "build-time replay of `{machine_name}` completed without exact source-input replay"
        ))]);
    }
    if complete_replay_verified && staged_output_tree.is_none() {
        return Err(vec![Diagnostic::error(format!(
            "build-time replay of `{machine_name}` completed without staged-output custody"
        ))]);
    }
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
    // Linear required-output custody: every declared member must be completed
    // by exactly one sealed regular file in retained staged-output custody
    // before any generated source is selected or the result may publish.
    filesystem_scope.verify_required_outputs(staged_output_tree.as_ref(), &machine_name)?;
    // Compiler-owned output obligations issued by `builder.output.require`
    // settle against the same staged custody: every obligation must be
    // `Completed` exactly once by the sealed regular file its receipt names,
    // and no completed output may have been mutated afterward.
    let required_output_settlements = settle_build_output_obligations(
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
    let generated_sources = match staged_output_tree.as_ref() {
        Some(tree) => {
            let included_source_paths = included_source_handoffs
                .iter()
                .map(|handoff| handoff.relative_path.clone())
                .collect::<Vec<_>>();
            select_included_sources(tree, &included_source_paths)?
        }
        None if included_source_handoffs.is_empty() => Vec::new(),
        None => {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` handed off generated source without sponsored staged-output custody"
            ))]);
        }
    };
    Ok(ComputedBuildConfig {
        config,
        optimization_report_request: optimization_report,
        evaluation_usage: Some(BuildEvaluationUsage {
            usage_schema_version: usage.schema().schema_version(),
            step_schedule_marker: usage.schedule().marker(),
            invocation_fuel_ceiling: usage.fuel_ceiling(),
            sponsor_schema_version: evaluation_sponsor
                .map(|sponsor| sponsor.limits().schema_version()),
            session_fuel_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_fuel_units()),
            session_build_log_byte_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_build_log_bytes()),
            session_filesystem_attempt_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_filesystem_operation_attempts()),
            session_live_filesystem_handle_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_live_filesystem_handles()),
            session_live_cell_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_live_cells()),
            session_live_text_byte_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_live_text_bytes()),
            session_result_cell_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_result_cells()),
            session_result_text_byte_ceiling: evaluation_sponsor
                .map(|sponsor| sponsor.limits().maximum_result_text_bytes()),
            session_peak_live_filesystem_handles: evaluation_sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_filesystem_handles),
            session_peak_live_cells: evaluation_sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_cells),
            session_peak_live_text_bytes: evaluation_sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_text_bytes),
            fuel_units: usage.fuel_units(),
            replay_fuel_units: replay_usage.map_or(0, |usage| usage.fuel_units()),
            build_log_bytes: usage.build_log_bytes(),
            replay_build_log_bytes: replay_usage.map_or(0, |usage| usage.build_log_bytes()),
            filesystem_operation_attempts: usage.filesystem_operation_attempts(),
            replay_filesystem_operation_attempts: replay_usage
                .map_or(0, |usage| usage.filesystem_operation_attempts()),
            peak_live_cells: usage.peak_live_cells(),
            replay_peak_live_cells: replay_usage.map_or(0, |usage| usage.peak_live_cells()),
            peak_live_text_bytes: usage.peak_live_text_bytes(),
            replay_peak_live_text_bytes: replay_usage
                .map_or(0, |usage| usage.peak_live_text_bytes()),
            result_cells: usage.result_cells(),
            replay_result_cells: replay_usage.map_or(0, |usage| usage.result_cells()),
            result_text_bytes: usage.result_text_bytes(),
            replay_result_text_bytes: replay_usage.map_or(0, |usage| usage.result_text_bytes()),
        }),
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

/// Settle the compiler-owned required-output obligations recorded by a
/// successful build evaluation against retained staged-output custody.
///
/// Settlement is linear and compiler-checked: every issued obligation must
/// have been completed exactly once by the sealed regular file its receipt
/// names. A pending obligation, a sticky `fail`, a receipt rejoined to a
/// different output, a completed output mutated afterward, or a completion
/// whose file never reached sealed staged custody each reject the activation
/// before any product may publish. The returned rows are the durable
/// observation evidence; `build.rs` publication still re-derives the file
/// from staged custody.
fn settle_build_output_obligations(
    obligations: &[checked_interpreter::BuildOutputObligation],
    receipts: &[checked_interpreter::BuildOutputReceipt],
    staged_output_tree: Option<&BuildStagedOutputTree>,
    machine_name: &str,
) -> Result<Vec<BuildRequiredOutputSettlement>, Vec<Diagnostic>> {
    let mut settlements = Vec::with_capacity(obligations.len());
    for (index, obligation) in obligations.iter().enumerate() {
        let name = String::from_utf8_lossy(obligation.relative_path());
        if obligation.root() != BUILD_OUTPUT_ROOT_IDENTITY {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` issued required output `{name}` outside the compiler-issued Output root"
            ))]);
        }
        let checked_interpreter::BuildOutputObligationState::Completed {
            receipt: receipt_index,
            ..
        } = obligation.state()
        else {
            let message = match obligation.state() {
                checked_interpreter::BuildOutputObligationState::Pending => format!(
                    "required output `{name}` of `{machine_name}` was declared but never completed"
                ),
                checked_interpreter::BuildOutputObligationState::Failed { diagnostic, .. } => {
                    format!(
                        "required output `{name}` of `{machine_name}` failed: {}",
                        String::from_utf8_lossy(diagnostic)
                    )
                }
                checked_interpreter::BuildOutputObligationState::Completed { .. } => {
                    unreachable!("completed obligations settle")
                }
            };
            return Err(vec![Diagnostic::error(message)]);
        };
        let Some(receipt) = receipts.get(*receipt_index) else {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` issued no completion receipt for required output `{name}`"
            ))]);
        };
        if receipt.obligation() != index
            || receipt.root() != obligation.root()
            || receipt.relative_path() != obligation.relative_path()
        {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` completed required output `{name}` against a different output than it declared"
            ))]);
        }
        if !obligation.post_completion_mutations().is_empty() {
            return Err(vec![Diagnostic::error(format!(
                "required output `{name}` of `{machine_name}` was mutated after its completion was accepted"
            ))]);
        }
        match staged_output_tree.and_then(|tree| tree.sealed_entry(obligation.relative_path())) {
            Some(entry) if matches!(entry.kind(), BuildStagedOutputEntryKind::File { .. }) => {}
            Some(_) => {
                return Err(vec![Diagnostic::error(format!(
                    "required output `{name}` of `{machine_name}` is not a sealed regular file in staged-output custody"
                ))]);
            }
            None => {
                return Err(vec![Diagnostic::error(format!(
                    "required output `{name}` of `{machine_name}` was completed without sealed staged-output custody"
                ))]);
            }
        }
        settlements.push(BuildRequiredOutputSettlement {
            relative_path: obligation.relative_path().to_vec(),
            sealed_attempt_ordinal: u64::try_from(receipt.sealed_at()).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "build-time evaluation of `{machine_name}` produced a sealed-output ordinal outside canonical u64"
                ))]
            })?,
        });
    }
    Ok(settlements)
}
