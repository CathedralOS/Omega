//! Bind a request's package and root staging scope, reopening matching
//! review evidence, before build admission.

use crate::BuildMachineFilesystemScope;
use crate::ReviewOnlyBuildFilesystemReplayRecord;
use build_time_evaluation::BuildMachineFilesystemSponsor;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process counter giving every captured-source snapshot a unique fresh
/// backing directory. The backing must sit outside the source root (so its
/// residue can never enter a later canonical capture) and outside the build
/// write root (so the machine cannot reach it through its Output grant).
static NEXT_CAPTURED_SOURCE_SNAPSHOT: AtomicU64 = AtomicU64::new(0);

/// Reopen matching review evidence and bind the request's package/root staging
/// scope before build admission. Replay remains review-only filesystem custody.
/// `build_execution_profile` is the request's admitted profile for build-scope
/// sources and the build machine; it joins the activation every replay record
/// is bound to.
pub fn prepare_filesystem_scope(
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    build_execution_profile: target::TargetProfile,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<BuildMachineFilesystemSponsor>,
    replay_record: Option<&ReviewOnlyBuildFilesystemReplayRecord>,
    build_snapshot: Option<&crate::BuildSnapshotRequest>,
) -> Result<BuildMachineFilesystemScope, Vec<Diagnostic>> {
    if replay_record.is_some() && build_snapshot.is_some() {
        return Err(vec![Diagnostic::error(
            "a build snapshot request applies only to a primary execution; filesystem replay evidence already fixes this build occurrence",
        )]);
    }
    if let Some(replay_record) = replay_record {
        let expected_source_metadata = package_inputs
            .map(|inputs| {
                inputs
                    .canonical_source_metadata(inputs.root())
                    .map(|metadata| {
                        crate::BuildCanonicalSourceMetadataIdentity::new(
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
            crate::rehydrate_review_only_build_filesystem_replay_record(
                record,
                crate::BuildFilesystemReplayRecordLimits::new(
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
        crate::BuildMachineFilesystemScope::for_package_root(
            inputs
                .package_root(inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf(),
            build_dir,
            filesystem_sponsor,
            inputs.canonical_source_metadata(inputs.root()).cloned(),
        )
        .with_package_activation(inputs.root(), inputs.root_role())
    } else {
        crate::BuildMachineFilesystemScope::for_root(root_path, build_dir, filesystem_sponsor)
    }
    .with_execution_profile(build_execution_profile);
    if let Some(build_snapshot) = build_snapshot {
        // One capture authority produces the immutable input inventory and
        // its canonical metadata index together; the scope rejects a captured
        // inventory that disagrees with the binding's validated index.
        let source_root = package_inputs
            .map(|inputs| {
                inputs
                    .package_root(inputs.root())
                    .expect("validated package inputs retain their root")
                    .to_path_buf()
            })
            .unwrap_or_else(|| {
                root_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
            });
        let captured_input = package_compilation::capture_package_source_input(&source_root)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "could not capture the build source snapshot: {reason}"
                ))]
            })?;
        let snapshot_dir = std::env::temp_dir().join(format!(
            "omega-captured-source-{}-{}",
            std::process::id(),
            NEXT_CAPTURED_SOURCE_SNAPSHOT.fetch_add(1, Ordering::Relaxed)
        ));
        build_machine_filesystem_scope = build_machine_filesystem_scope
            .with_captured_source_input(captured_input, snapshot_dir)?
            .with_required_outputs(build_snapshot.required_outputs().iter().cloned())?;
    }
    if let Some(filesystem_replay) = filesystem_replay {
        build_machine_filesystem_scope = build_machine_filesystem_scope.with_replay(
            filesystem_replay,
            replay_record
                .expect("a rehydrated filesystem replay always accompanies its record")
                .replay_activation(),
        );
    }
    Ok(build_machine_filesystem_scope)
}
