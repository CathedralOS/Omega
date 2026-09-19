//! Projecting the retained filesystem attempts and generated-source handoffs
//! of a successful run into the durable observation rows.

use crate::evidence::filesystem_scope::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY};
use crate::evidence::observations::{
    BuildFilesystemAuthorizedPath, BuildFilesystemGrantAccess, BuildFilesystemGrantRefusal,
    BuildFilesystemGrantRefusalReason, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemOperationAttempt, BuildFilesystemProvider,
    BuildFilesystemRoot, BuildIncludedSourceHandoff, project_logical_handle_identity,
    project_logical_handle_input_resolution, project_logical_handle_kind,
    project_logical_handle_output_source, project_operation_result,
};
use checked_interpreter::EvaluationObservations;
use diagnostics::Diagnostic;

/// Every filesystem attempt of the run as an observation row, with grant roots
/// resolved to the compiler-issued Source and Output roots.
pub(super) fn project_filesystem_operation_attempts(
    observations: &EvaluationObservations,
    machine_name: &str,
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
                result: project_operation_result(
                    attempt
                        .result()
                        .expect("successful build evaluation cannot retain a halted filesystem call"),
                ),
                post_error: attempt
                    .post_error()
                    .expect("successful build evaluation cannot retain a halted filesystem call"),
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
