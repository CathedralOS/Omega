//! Projecting the retained filesystem attempts and generated-source handoffs
//! of a successful run into the durable observation rows.

use super::filesystem_scope::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY};
use super::observations::{
    BuildFilesystemAuthorizedPath, BuildFilesystemOperationAttempt, BuildFilesystemRoot,
    BuildIncludedSourceHandoff,
};
use checked_interpreter::EvaluationObservations;
use diagnostics::Diagnostic;

/// Every filesystem attempt of the run as an observation row, with grant roots
/// joined to the compiler-issued Source, Output, and named input roots.
pub(super) fn project_filesystem_operation_attempts(
    observations: &EvaluationObservations,
    filesystem_scope: &crate::BuildMachineFilesystemScope,
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
                    } else if filesystem_scope.named_input_inventory(path.root()).is_some() {
                        BuildFilesystemRoot::NamedInput(path.root().get())
                    } else {
                        return Err(Diagnostic::error(format!(
                            "build-time evaluation of `{machine_name}` returned unknown filesystem grant-root identity `{}`",
                            path.root().get()
                        )));
                    };
                    Ok(BuildFilesystemAuthorizedPath {
                        operand_ordinal: path.operand_ordinal(),
                        access: path.access(),
                        root,
                        relative_path: path.relative_path().to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(BuildFilesystemOperationAttempt {
                operation_tag: attempt.operation_tag(),
                provider: attempt.provider(),
                result: attempt
                        .result()
                        .expect("successful build evaluation cannot retain a halted filesystem call"),
                post_error: attempt
                    .post_error()
                    .expect("successful build evaluation cannot retain a halted filesystem call"),
                authorized_paths,
                logical_handle_inputs: attempt.logical_handle_inputs().to_vec(),
                logical_handle_output: attempt.logical_handle_output(),
                retired_logical_handles: attempt.retired_logical_handles().to_vec(),
                grant_refusals: attempt.grant_refusals().to_vec(),
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
