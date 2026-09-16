//! Projecting the retained filesystem attempts and generated-source handoffs
//! of a successful run into the durable observation rows.

use crate::evidence::filesystem_scope::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY};
use crate::evidence::observations::{
    BuildFilesystemAuthorizedPath, BuildFilesystemByteOperand, BuildFilesystemGrantAccess,
    BuildFilesystemGrantRefusal, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleOutput,
    BuildFilesystemMetadataObservation, BuildFilesystemMetadataObservationKind,
    BuildFilesystemMutableByteOperand, BuildFilesystemMutableByteOperandResolution,
    BuildFilesystemMutableI64Operand, BuildFilesystemMutableI64OperandResolution,
    BuildFilesystemObservedByteRegion, BuildFilesystemObservedByteRegionKind,
    BuildFilesystemOperationAttempt, BuildFilesystemOperationObservationClass,
    BuildFilesystemPathLikeOperand, BuildFilesystemProvider, BuildFilesystemReturnedPath,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildIncludedSourceHandoff, project_logical_handle_identity,
    project_logical_handle_input_resolution, project_logical_handle_kind,
    project_logical_handle_output_source, project_operation_result, project_scalar_operand_value,
};
use checked_interpreter::EvaluationObservations;
use diagnostics::Diagnostic;

/// Every filesystem attempt of the run as an observation row, with grant roots
/// resolved to the compiler-issued Source and Output roots.
pub(super) fn project_filesystem_operation_attempts(
    observations: &EvaluationObservations,
    machine_name: &str,
    source_inputs_replayed: bool,
) -> Result<Vec<BuildFilesystemOperationAttempt>, Vec<Diagnostic>> {
    observations
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
        .map_err(|diagnostic| vec![diagnostic])
}

/// Every generated source the run handed off, bound to the attempt that
/// sealed it inside this occurrence's filesystem custody.
pub(super) fn project_included_source_handoffs(
    observations: &EvaluationObservations,
    attempt_count: usize,
    machine_name: &str,
) -> Result<Vec<BuildIncludedSourceHandoff>, Vec<Diagnostic>> {
    observations
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
            if filesystem_attempt_ordinal > attempt_count as u64 {
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
        .map_err(|diagnostic| vec![diagnostic])
}
