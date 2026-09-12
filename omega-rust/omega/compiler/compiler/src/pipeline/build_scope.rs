use super::PackageCompilationInputs;
use build_evaluation::BuildMachineFilesystemScope;
use build_evaluation::ReviewOnlyBuildFilesystemReplayRecord;
use build_time_evaluation::BuildMachineFilesystemSponsor;
use diagnostics::Diagnostic;
use std::path::Path;

/// Reopen matching review evidence and bind the request's package/root staging
/// scope before build admission. Replay remains review-only filesystem custody.
pub(super) fn prepare_filesystem_scope(
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<BuildMachineFilesystemSponsor>,
    replay_record: Option<&ReviewOnlyBuildFilesystemReplayRecord>,
) -> Result<BuildMachineFilesystemScope, Vec<Diagnostic>> {
    if let Some(replay_record) = replay_record {
        let expected_source_metadata = package_inputs
            .map(|inputs| {
                inputs
                    .canonical_source_metadata(inputs.root())
                    .map(|metadata| {
                        crate::pipeline::build_config::BuildCanonicalSourceMetadataIdentity::new(
                            metadata.policy_version(),
                            *metadata.source_content_commitment(),
                        )
                    })
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "package-aware filesystem replay requires canonical Source metadata",
                        )]
                    })
            })
            .transpose()?;
        if replay_record.canonical_source_metadata_identity() != expected_source_metadata {
            return Err(vec![Diagnostic::error(
                "build filesystem replay record does not match the current canonical Source metadata identity",
            )]);
        }
    }
    let filesystem_replay = replay_record
        .map(|record| {
            super::build_replay_record::rehydrate_review_only_build_filesystem_replay_record(
                record,
                super::BuildFilesystemReplayRecordLimits::new(
                    record.canonical_bytes().len(),
                    4_096,
                ),
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "could not reopen build filesystem replay record: {error}"
                ))]
            })
        })
        .transpose()?;
    let build_dir = build_dir.map(Path::to_path_buf).unwrap_or_else(|| {
        root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.join("build"))
            .unwrap_or_else(|| std::path::PathBuf::from("build"))
    });
    let mut build_machine_filesystem_scope = if let Some(inputs) = package_inputs {
        crate::pipeline::build_config::BuildMachineFilesystemScope::for_package_root(
            inputs
                .package_root(inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf(),
            build_dir,
            filesystem_sponsor,
            inputs.canonical_source_metadata(inputs.root()).cloned(),
        )
    } else {
        crate::pipeline::build_config::BuildMachineFilesystemScope::for_root(
            root_path,
            build_dir,
            filesystem_sponsor,
        )
    };
    if let Some(filesystem_replay) = filesystem_replay {
        build_machine_filesystem_scope =
            build_machine_filesystem_scope.with_replay(filesystem_replay);
    }
    Ok(build_machine_filesystem_scope)
}
