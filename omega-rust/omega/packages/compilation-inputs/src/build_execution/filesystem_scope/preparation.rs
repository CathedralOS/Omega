//! Bind the request's package and root staging scope before build admission.

use crate::BuildMachineFilesystemScope;
use crate::PackageCompilationInputs;
use build_time_evaluation::BuildMachineFilesystemSponsor;
use diagnostics::Diagnostic;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique private output roots; captured inputs require no host directories.
static NEXT_PRIVATE_OUTPUT: AtomicU64 = AtomicU64::new(0);

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
    let source_root = package_inputs
        .map(|inputs| {
            inputs
                .package_root(inputs.root())
                .expect("validated package inputs retain their root")
                .to_path_buf()
        })
        .or_else(|| {
            required_sources
                .retained_source_root()
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| {
            root_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        });
    let build_dir = build_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| source_root.join("build"));
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
        crate::BuildMachineFilesystemScope::for_source_root(
            source_root.clone(),
            build_dir,
            filesystem_sponsor,
        )
    }
    .with_execution_profile(build_execution_profile);
    // The loader already captured this namespace before parsing. Deriving
    // Build's metadata from those original trees performs no host read and
    // cannot pick up a later import, deleted member or changed source byte.
    let package_input =
        package_inputs.and_then(|inputs| inputs.captured_source_input(inputs.root()));
    let loader_input = if package_input.is_none() {
        required_sources
            .retained_source_filesystem()
            .map(|filesystem| filesystem.captured_tree_at(&source_root))
            .transpose()
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "could not select the retained Build source input: {error}"
                ))]
            })?
            .flatten()
            .map(|tree| crate::source_snapshot::retained_source_input(&tree))
            .transpose()
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "could not derive retained Build source metadata: {reason}"
                ))]
            })?
    } else {
        None
    };
    let retained_input = package_input.or(loader_input.as_ref());
    // Retained custody always supplies the Source view, including callers
    // that did not select an explicit snapshot request. It must never become
    // a real-host grant merely because this activation has no output roster.
    let automatic_snapshot =
        retained_input.map(|_| crate::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
    let build_snapshot = build_snapshot.or(automatic_snapshot.as_ref());
    if let Some(build_snapshot) = build_snapshot {
        // Named immutable inputs are assignment-bound to exact dependency
        // occurrences before any filesystem work: an input keyed to an edge
        // the reconciled graph does not contain is extra and rejects rather
        // than attaching to the nearest edge.
        for (occurrence, _) in build_snapshot.dependency_inputs() {
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
        }
        if build_snapshot.dependency_inputs().next().is_some() {
            return Err(vec![Diagnostic::error(
                "named dependency inputs must be routed by package-closure preparation before build activation",
            )]);
        }
        // Review supplies a session sponsor; ordinary compilation does not.
        // Both must use the same captured-output custody. Provision only a
        // fresh private write root, never adopt earlier publication contents.
        if build_machine_filesystem_scope.sponsor.is_none() {
            let sponsor = loop {
                let root = std::env::temp_dir().join(format!(
                    "omega-build-output-{}-{}",
                    std::process::id(),
                    NEXT_PRIVATE_OUTPUT.fetch_add(1, Ordering::Relaxed),
                ));
                match BuildMachineFilesystemSponsor::create_private(
                    &root,
                    build_time_evaluation::BuildMachineFilesystemSponsorLimits::COMPILER_DEFAULT,
                ) {
                    Ok(sponsor) => break sponsor,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => {
                        return Err(vec![Diagnostic::error(format!(
                            "could not create private build output staging: {error}"
                        ))]);
                    }
                }
            };
            let canonical_root = sponsor.session_root().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "could not bind build output staging: {error}"
                ))]
            })?;
            build_machine_filesystem_scope.build_dir = canonical_root.join("output");
            // The admitted key must follow the root it describes. Admission
            // resolved the CALLER's requested spelling, and this provisioning
            // has just replaced it with a root created privately here, so the
            // baseline is re-bound to the new root -- its admission IS that
            // private creation. Establishment's comparison keeps its meaning
            // for every caller-supplied spelling, which is the case the
            // baseline exists to protect: an ancestor swapped to a host alias
            // after admission still changes the resolution and still rejects.
            build_machine_filesystem_scope.admitted_build_dir_key =
                super::overlap_key(&build_machine_filesystem_scope.build_dir);
            build_machine_filesystem_scope.sponsor = Some(sponsor);
        }
        // One capture authority produces the immutable input inventory and
        // its canonical metadata index together; the scope rejects a captured
        // inventory that disagrees with the binding's validated index.
        let captured_input = match build_snapshot.capture() {
            crate::BuildSnapshotCapture::SourceInventory => match retained_input {
                Some(input) => input.clone(),
                None => {
                    let Some(inputs) = package_inputs else {
                        return Err(vec![Diagnostic::error(
                            "a complete Build source inventory requires retained loader capture or package source custody",
                        )]);
                    };
                    if inputs.canonical_source_metadata(inputs.root()).is_none() {
                        return Err(vec![Diagnostic::error(
                            "a complete Build source inventory requires compiler-validated canonical Source metadata",
                        )]);
                    }
                    crate::capture_package_source_input(&source_root).map_err(|reason| {
                        vec![Diagnostic::error(format!(
                            "could not capture the build source snapshot: {reason}"
                        ))]
                    })?
                }
            },
            crate::BuildSnapshotCapture::Scoped(request) => {
                let captured = if let Some(complete) = retained_input {
                    let filesystem = required_sources
                        .retained_source_filesystem()
                        .map(std::sync::Arc::as_ref)
                        .or_else(|| package_inputs.map(PackageCompilationInputs::source_filesystem))
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "retained source capture has no original compilation filesystem",
                            )]
                        })?;
                    let mut required_members = Vec::new();
                    for file in required_sources.files() {
                        // Toolchain and injected sources are not members of
                        // this package's inventory; virtual names need no
                        // physical or captured path resolution here.
                        if !file.path.starts_with(&source_root) {
                            continue;
                        }
                        let resolved = filesystem.canonicalize(&file.path).map_err(|error| {
                            vec![Diagnostic::error(format!(
                                "could not resolve required source member {} in the invocation namespace: {error}",
                                file.path.display()
                            ))]
                        })?;
                        if let Ok(relative) = resolved.strip_prefix(&source_root) {
                            required_members.push(relative.to_path_buf());
                        }
                    }
                    crate::source_snapshot::retained_scoped_source_input(
                        complete,
                        request,
                        required_members,
                    )
                } else {
                    crate::capture_scoped_source_input(
                        &source_root,
                        request,
                        required_sources.files().map(|file| file.path.clone()),
                    )
                };
                captured.map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "could not capture the build source snapshot: {reason}"
                    ))]
                })?
            }
        };
        build_machine_filesystem_scope = if package_inputs.is_some()
            && matches!(
                build_snapshot.capture(),
                crate::BuildSnapshotCapture::Scoped(_)
            ) {
            // A narrowed inventory has a different commitment from the full
            // package. Compare exact captured entries against that package's
            // authenticated full capture rather than dropping provenance or
            // treating matching path kinds/lengths as matching source bytes.
            let physical_complete;
            let complete = match retained_input {
                Some(complete) => complete,
                None => {
                    physical_complete =
                        crate::capture_package_source_input(&source_root).map_err(|reason| {
                            vec![Diagnostic::error(format!(
                                "could not validate the scoped package source snapshot: {reason}"
                            ))]
                        })?;
                    &physical_complete
                }
            };
            build_machine_filesystem_scope
                .with_scoped_package_source_input(captured_input, complete)?
        } else {
            build_machine_filesystem_scope.with_captured_source_input(captured_input)?
        }
        .with_named_inputs(build_snapshot.inputs())
        .with_required_outputs(build_snapshot.required_outputs().iter().cloned())?;
    }
    Ok(build_machine_filesystem_scope)
}
