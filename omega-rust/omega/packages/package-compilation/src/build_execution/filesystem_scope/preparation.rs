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
    anonymous_source: Option<&crate::AnonymousSourceCapture>,
    required_sources: &source::SourceMap,
    build_execution_profile: Option<target::TargetProfile>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<BuildMachineFilesystemSponsor>,
    build_snapshot: Option<&crate::BuildSnapshotRequest>,
    prepared_source: Option<PreparedBuildSourceInput>,
) -> Result<BuildMachineFilesystemScope, Vec<Diagnostic>> {
    if package_inputs.is_some() && anonymous_source.is_some() {
        return Err(vec![Diagnostic::error(
            "Build scope cannot combine anonymous source custody with supplied package inputs",
        )]);
    }
    if prepared_source.is_some()
        && !matches!(
            build_snapshot.map(crate::BuildSnapshotRequest::capture),
            Some(crate::BuildSnapshotCapture::Scoped(_))
        )
    {
        return Err(vec![Diagnostic::error(
            "prepared Build source inventory requires its scoped snapshot request",
        )]);
    }
    let anonymous_input = anonymous_source
        .map(|source| source.build_input())
        .transpose()
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "could not bind acquired Build input: {reason}"
            ))]
        })?;
    let retained_input = package_inputs
        .and_then(|inputs| inputs.captured_source_input(inputs.root()))
        .or(anonymous_input.as_ref());
    let source_root = package_inputs
        .and_then(|inputs| inputs.package_root(inputs.root()))
        .or_else(|| anonymous_source.map(|source| source.root()))
        .or_else(|| {
            root_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
        })
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let build_dir = build_dir.map(Path::to_path_buf).unwrap_or_else(|| {
        root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| parent.join("build"))
            .unwrap_or_else(|| std::path::PathBuf::from("build"))
    });
    let mut build_machine_filesystem_scope =
        if package_inputs.is_some() || anonymous_source.is_some() {
            crate::BuildMachineFilesystemScope::for_admitted_root(
                source_root.clone(),
                build_dir,
                filesystem_sponsor,
                package_inputs
                    .and_then(|inputs| inputs.canonical_source_metadata(inputs.root()))
                    .cloned()
                    .or_else(|| {
                        anonymous_input
                            .as_ref()
                            .map(|input| input.canonical_source_metadata().clone())
                    }),
            )
        } else {
            crate::BuildMachineFilesystemScope::for_root(root_path, build_dir, filesystem_sponsor)
        }
        .with_execution_profile(build_execution_profile);
    if let Some(inputs) = package_inputs {
        build_machine_filesystem_scope = build_machine_filesystem_scope
            .with_package_activation(inputs.root(), inputs.root_role());
    }
    // Retained custody always supplies the Source view, including callers
    // that did not select an explicit snapshot request. It must never become
    // a real-host grant merely because this activation has no output roster.
    let automatic_snapshot =
        retained_input.map(|_| crate::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>()));
    let build_snapshot = build_snapshot.or(automatic_snapshot.as_ref());
    if let Some(build_snapshot) = build_snapshot {
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
            crate::BuildSnapshotCapture::PackageInventory => {
                // An acquired inventory is already closed before it has a
                // package name. Only the uncaptured physical adapter needs
                // package custody or an explicit caller-selected inventory.
                match retained_input {
                    Some(input) => input.clone(),
                    None => {
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
                        crate::capture_package_source_input(&source_root).map_err(|reason| {
                            vec![Diagnostic::error(format!(
                                "could not capture the build source snapshot: {reason}"
                            ))]
                        })?
                    }
                }
            }
            crate::BuildSnapshotCapture::Scoped(request) => {
                let captured = match prepared_source {
                    Some(prepared) => prepared.admit(
                        root_path,
                        &source_root,
                        package_inputs
                            .map(|inputs| inputs.source_filesystem())
                            .or_else(|| anonymous_source.map(|source| source.filesystem())),
                        required_sources,
                        request,
                    ),
                    None => prepare_scoped_source_input(
                        root_path,
                        &source_root,
                        package_inputs
                            .map(|inputs| inputs.source_filesystem())
                            .or_else(|| anonymous_source.map(|source| source.filesystem())),
                        retained_input,
                        required_sources,
                        request,
                    ),
                };
                captured.map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "could not capture the build source snapshot: {reason}"
                    ))]
                })?
            }
        };
        build_machine_filesystem_scope =
            if (package_inputs.is_some() || anonymous_source.is_some())
                && matches!(
                    build_snapshot.capture(),
                    crate::BuildSnapshotCapture::Scoped(_)
                )
            {
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

/// An admitted selection from an immutable source inventory. It carries no
/// filesystem grant or executable authority; the activation binds those later.
pub struct PreparedBuildSourceInput {
    source_root: std::path::PathBuf,
    request: crate::BuildSourceCaptureRequest,
    input: build_output::CapturedBuildSourceInput,
}

impl PreparedBuildSourceInput {
    pub(crate) fn prepare(
        root_path: &Path,
        source_root: &Path,
        filesystem: &source::SourceFileSystem,
        retained_input: Option<&build_output::CapturedBuildSourceInput>,
        required_sources: &source::SourceMap,
        request: &crate::BuildSourceCaptureRequest,
    ) -> Result<Self, Vec<Diagnostic>> {
        let input = prepare_scoped_source_input(
            root_path,
            source_root,
            Some(filesystem),
            retained_input,
            required_sources,
            request,
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "could not capture the build source snapshot: {reason}"
            ))]
        })?;
        Ok(Self {
            source_root: source_root.to_path_buf(),
            request: request.clone(),
            input,
        })
    }

    fn admit(
        self,
        root_path: &Path,
        source_root: &Path,
        filesystem: Option<&source::SourceFileSystem>,
        required_sources: &source::SourceMap,
        request: &crate::BuildSourceCaptureRequest,
    ) -> Result<build_output::CapturedBuildSourceInput, String> {
        if self.source_root != source_root || self.request != *request {
            return Err("prepared Build source inventory belongs to another request".to_owned());
        }
        // Supplier declarations may extend the Build prefix. Check the final
        // required set against the retained selection without recapturing it.
        let required =
            required_source_members(root_path, source_root, filesystem, required_sources)?;
        crate::source::source_snapshot::require_source_members(
            |path| self.input.retained_entry(path).is_some(),
            required,
        )?;
        Ok(self.input)
    }
}

fn prepare_scoped_source_input(
    root_path: &Path,
    source_root: &Path,
    filesystem: Option<&source::SourceFileSystem>,
    retained_input: Option<&build_output::CapturedBuildSourceInput>,
    required_sources: &source::SourceMap,
    request: &crate::BuildSourceCaptureRequest,
) -> Result<build_output::CapturedBuildSourceInput, String> {
    match retained_input {
        Some(complete) => crate::source::source_snapshot::retained_scoped_source_input(
            complete,
            request,
            required_source_members(root_path, source_root, filesystem, required_sources)?,
        ),
        None => crate::capture_scoped_source_input(
            source_root,
            request,
            required_sources
                .selected_files()
                .map(|file| file.path.clone())
                .chain(std::iter::once(root_path.to_path_buf())),
        ),
    }
}

fn required_source_members(
    root_path: &Path,
    source_root: &Path,
    filesystem: Option<&source::SourceFileSystem>,
    required_sources: &source::SourceMap,
) -> Result<Vec<std::path::PathBuf>, String> {
    let filesystem =
        filesystem.ok_or_else(|| "retained Build input lost its source namespace".to_owned())?;
    let authored_members = required_sources.selected_files().filter(|file| {
        file.origin == source::SourceOrigin::User
            && file.resolution_stratum == source::SourceResolutionStratum::Base
            && file.package_root == source_root
    });
    // The explicit Product entry is required while its body remains unloaded.
    std::iter::once(root_path)
        .chain(authored_members.map(|file| file.path.as_path()))
        .map(|path| {
            let resolved = filesystem.canonicalize(path).map_err(|error| format!(
                "could not resolve required source member {} in the invocation namespace: {error}",
                path.display()
            ))?;
            resolved
                .strip_prefix(source_root)
                .map(Path::to_path_buf)
                .map_err(|error| {
                    format!(
                        "required source member {} is outside its captured root: {error}",
                        path.display()
                    )
                })
        })
        .collect()
}
