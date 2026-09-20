//! Bind the request's package and root staging scope before build admission.

use crate::BuildMachineFilesystemScope;
use build_time_evaluation::BuildMachineFilesystemSponsor;
use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process counter giving every captured-source snapshot a unique fresh
/// backing directory. The backing must sit outside the source root (so its
/// residue can never enter a later canonical capture) and outside the build
/// write root (so the machine cannot reach it through its Output grant).
static NEXT_CAPTURED_SOURCE_SNAPSHOT: AtomicU64 = AtomicU64::new(0);

/// Private staging is never a successful output directory. Retained bytes
/// outlive it; failure at any admission/evaluation step releases it as well.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct BuildOutputScratch(std::path::PathBuf);

impl Drop for BuildOutputScratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl BuildOutputScratch {
    fn create() -> Result<Self, Vec<Diagnostic>> {
        loop {
            let path = std::env::temp_dir().join(format!(
                "omega-build-output-{}-{}",
                std::process::id(),
                NEXT_CAPTURED_SOURCE_SNAPSHOT.fetch_add(1, Ordering::Relaxed),
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(vec![Diagnostic::error(format!(
                        "could not create private build output staging: {error}"
                    ))]);
                }
            }
        }
    }
}

/// Bind the request's package/root staging scope before build admission.
/// The execution profile joins the retained activation identity.
/// `None` records an admitted host no catalogued profile
/// describes rather than naming one. `required_sources` are the source
/// files this compilation already assembled; a scoped capture request must
/// cover every member located under the captured root.
pub fn prepare_filesystem_scope(
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    required_sources: &source::SourceMap,
    build_execution_profile: Option<target::TargetProfile>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<BuildMachineFilesystemSponsor>,
    build_snapshot: Option<&crate::BuildSnapshotRequest>,
) -> Result<BuildMachineFilesystemScope, Vec<Diagnostic>> {
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
        // Named immutable inputs are assignment-bound to exact dependency
        // occurrences before any filesystem work: an input keyed to an edge
        // the reconciled graph does not contain is extra and rejects rather
        // than attaching to the nearest edge.
        let mut dependency_inputs = BTreeMap::new();
        for (occurrence, slots) in build_snapshot.dependency_inputs() {
            let admitted =
                package_inputs.is_some_and(|inputs| inputs.has_dependency_occurrence(occurrence));
            if !admitted {
                return Err(vec![Diagnostic::error(format!(
                    "named build inputs are assigned to a dependency occurrence that does not exist: requester {:?}, purpose {:?}, alias `{}`, target {:?}",
                    occurrence.requester(),
                    occurrence.purpose(),
                    occurrence.alias(),
                    occurrence.target(),
                ))]);
            }
            dependency_inputs.insert(occurrence.clone(), slots.clone());
        }
        // Review supplies a session sponsor; ordinary compilation does not.
        // Both must use the same captured-output custody. Provision only a
        // fresh private write root, never adopt earlier publication contents.
        if build_machine_filesystem_scope.sponsor.is_none() {
            let scratch = std::sync::Arc::new(BuildOutputScratch::create()?);
            let canonical_root = std::fs::canonicalize(&scratch.0).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "could not bind build output staging: {error}"
                ))]
            })?;
            let sponsor = BuildMachineFilesystemSponsor::new(&canonical_root).map_err(|error| {
                vec![Diagnostic::error(format!(
                    "could not sponsor build output staging: {error}"
                ))]
            })?;
            build_machine_filesystem_scope.build_dir = canonical_root.join("output");
            build_machine_filesystem_scope.sponsor = Some(sponsor);
            build_machine_filesystem_scope.output_scratch = Some(scratch);
        }
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
        let captured_input = match build_snapshot.capture() {
            crate::BuildSnapshotCapture::PackageInventory => {
                // The package-inventory form is the package custody route:
                // a standalone request must name its members explicitly
                // rather than silently admitting the whole source root.
                let Some(inputs) = package_inputs else {
                    return Err(vec![Diagnostic::error(
                        "a package-inventory build snapshot requires package source custody; a standalone build must declare an explicit source capture request",
                    )]);
                };
                if inputs.canonical_source_metadata(inputs.root()).is_none() {
                    return Err(vec![Diagnostic::error(
                        "a package-inventory build snapshot requires compiler-validated canonical Source metadata",
                    )]);
                }
                package_compilation::capture_package_source_input(&source_root).map_err(
                    |reason| {
                        vec![Diagnostic::error(format!(
                            "could not capture the build source snapshot: {reason}"
                        ))]
                    },
                )?
            }
            crate::BuildSnapshotCapture::Scoped(request) => {
                package_compilation::capture_scoped_source_input(
                    &source_root,
                    request,
                    required_sources.files().map(|file| file.path.clone()),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "could not capture the build source snapshot: {reason}"
                    ))]
                })?
            }
        };
        let snapshot_dir = std::env::temp_dir().join(format!(
            "omega-captured-source-{}-{}",
            std::process::id(),
            NEXT_CAPTURED_SOURCE_SNAPSHOT.fetch_add(1, Ordering::Relaxed)
        ));
        build_machine_filesystem_scope = if package_inputs.is_some()
            && matches!(
                build_snapshot.capture(),
                crate::BuildSnapshotCapture::Scoped(_)
            ) {
            // A narrowed inventory has a different commitment from the full
            // package. Compare exact captured entries against that package's
            // authenticated full capture rather than dropping provenance or
            // treating matching path kinds/lengths as matching source bytes.
            let complete = package_compilation::capture_package_source_input(&source_root)
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "could not validate the scoped package source snapshot: {reason}"
                    ))]
                })?;
            build_machine_filesystem_scope.with_scoped_package_source_input(
                captured_input,
                &complete,
                snapshot_dir,
            )?
        } else {
            build_machine_filesystem_scope
                .with_captured_source_input(captured_input, snapshot_dir)?
        }
        .with_dependency_inputs(dependency_inputs)
        .with_required_outputs(build_snapshot.required_outputs().iter().cloned())?;
    }
    Ok(build_machine_filesystem_scope)
}
