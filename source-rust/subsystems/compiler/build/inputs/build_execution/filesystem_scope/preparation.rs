// Request-to-scope preparation (`prepare_filesystem_scope`) before build admission.
//
// Binds the compilation request's package or anonymous source custody, execution
// profile, and output staging directory into a `BuildMachineFilesystemScope`.
// Retained package or anonymous source custody automatically activates snapshot
// mode (`BuildSnapshotRequest`) even when the caller did not supply an explicit
// snapshot request, ensuring retained custody never falls back to a real-host grant.
// When snapshot mode is active without a caller-provided session sponsor, we
// provision a fresh private temporary output staging sponsor (`NEXT_PRIVATE_OUTPUT`)
// and re-bind `admitted_build_dir_key` to that newly created private root.
// For narrowed (`Scoped`) package snapshots, we verify that every assembled user
// source file and the product root are covered and that the scoped inventory is
// an exact byte-for-byte subset of the authenticated full package capture.

use crate::BuildMachineFilesystemScope;
use crate::PackageCompilationInputs;
use diagnostics::Diagnostic;
use filesystem_authority::{FilesystemSponsor, FilesystemSponsorLimits};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PRIVATE_OUTPUT: AtomicU64 = AtomicU64::new(0);

pub fn prepare_filesystem_scope(
    root_path: &Path,
    package_inputs: Option<&PackageCompilationInputs>,
    anonymous_source: Option<&crate::AnonymousSourceCapture>,
    required_sources: &source_compiler::representations::source_inputs::SourceMap,
    build_execution_profile: Option<target::TargetProfile>,
    build_dir: Option<&Path>,
    filesystem_sponsor: Option<FilesystemSponsor>,
    build_snapshot: Option<&crate::BuildSnapshotRequest>,
) -> Result<BuildMachineFilesystemScope, Vec<Diagnostic>> {
    if package_inputs.is_some() && anonymous_source.is_some() {
        return Err(vec![Diagnostic::error(
            "Build scope cannot combine anonymous source custody with supplied package inputs",
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
    if retained_input.is_some() && build_snapshot.is_none() {
        return Err(vec![Diagnostic::error(
            "captured Build source requires a selected filesystem inventory",
        )]);
    }
    if let Some(build_snapshot) = build_snapshot {
        if build_machine_filesystem_scope.sponsor.is_none() {
            let sponsor = loop {
                let root = std::env::temp_dir().join(format!(
                    "omega-build-output-{}-{}",
                    std::process::id(),
                    NEXT_PRIVATE_OUTPUT.fetch_add(1, Ordering::Relaxed),
                ));
                match FilesystemSponsor::create_private(
                    &root,
                    FilesystemSponsorLimits::COMPILER_DEFAULT,
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
            build_machine_filesystem_scope.admitted_build_dir_key =
                super::overlap_key(&build_machine_filesystem_scope.build_dir);
            build_machine_filesystem_scope.sponsor = Some(sponsor);
        }
        let captured_input = match build_snapshot.capture() {
            crate::BuildSnapshotCapture::PackageInventory => match retained_input {
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
            },
            selection => {
                let filesystem = package_inputs
                    .map(|inputs| inputs.source_filesystem())
                    .or_else(|| anonymous_source.map(|source| source.filesystem()));
                let required_members =
                    required_source_members(root_path, &source_root, required_sources, filesystem)?;
                let inferred;
                let request = if let crate::BuildSnapshotCapture::Scoped(request) = selection {
                    request
                } else {
                    let entries = required_members
                        .iter()
                        .map(|path| {
                            crate::source::source_snapshot::capture_relative_path_bytes(path)
                                .map(|bytes| (bytes, crate::BuildSourceCaptureObligation::Required))
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    inferred = crate::BuildSourceCaptureRequest::new(entries)
                        .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    &inferred
                };
                let captured = if let Some(complete) = retained_input {
                    crate::source::source_snapshot::retained_scoped_source_input(
                        complete,
                        request,
                        required_members,
                    )
                } else {
                    crate::capture_scoped_source_input(
                        &source_root,
                        request,
                        required_members.iter().map(|path| source_root.join(path)),
                    )
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
                && !matches!(build_snapshot.capture(), crate::BuildSnapshotCapture::PackageInventory)
            {
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
        build_machine_filesystem_scope.directories = build_snapshot.directories().clone();
        build_machine_filesystem_scope.product_source_directory = build_snapshot
            .product_source_directory()
            .map(<[u8]>::to_vec);
    }
    Ok(build_machine_filesystem_scope)
}

fn required_source_members(
    entry: &Path,
    root: &Path,
    sources: &source_compiler::representations::source_inputs::SourceMap,
    filesystem: Option<&source_acquisition::capture::SourceFileSystem>,
) -> Result<std::collections::BTreeSet<std::path::PathBuf>, Vec<Diagnostic>> {
    let authored = sources.selected_files().filter(|file| {
        file.origin == source_admission::SourceOrigin::User
            && file.resolution_stratum == source_admission::SourceResolutionStratum::Base
            && file.package_root == root
    });
    let paths = std::iter::once(entry).chain(authored.map(|file| file.path.as_path()));
    let mut required_members = std::collections::BTreeSet::new();
    for path in paths {
        let resolved = match filesystem {
            Some(filesystem) => filesystem.canonicalize(path),
            None => path.canonicalize(),
        }
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "could not resolve required source member {}: {error}",
                path.display()
            ))]
        })?;
        let relative = resolved.strip_prefix(root).map_err(|error| {
            vec![Diagnostic::error(format!(
                "required source member {} is outside its captured root: {error}",
                path.display()
            ))]
        })?;
        required_members.insert(relative.to_path_buf());
    }
    Ok(required_members)
}
