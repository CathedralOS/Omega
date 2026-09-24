//! Bind the request's package and root staging scope before build admission.

use crate::BuildMachineFilesystemScope;
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
                    NEXT_CAPTURED_SOURCE_SNAPSHOT.fetch_add(1, Ordering::Relaxed),
                ));
                match BuildMachineFilesystemSponsor::create_private(&root) {
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
        // The snapshot backing lives under a private parent the occurrence
        // alone can traverse: create-exclusive 0o700 custody mirrors
        // `FilesystemSponsor::create_private`, while staying outside the
        // sponsor's session root keeps snapshot reads on the accounting
        // bypass every other outside-session read uses. The leaf name is
        // fixed because the parent is already unique; the release path
        // removes it once every backing it held is discarded.
        let snapshot_parent = loop {
            let candidate = std::env::temp_dir().join(format!(
                "omega-captured-source-session-{}-{}",
                std::process::id(),
                NEXT_CAPTURED_SOURCE_SNAPSHOT.fetch_add(1, Ordering::Relaxed)
            ));
            #[cfg_attr(not(unix), allow(unused_mut))]
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(vec![Diagnostic::error(format!(
                        "could not create the private captured-source snapshot staging directory `{}`: {error}",
                        candidate.display()
                    ))]);
                }
            }
        };
        let snapshot_dir = snapshot_parent.join("captured-source");
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
        .with_named_inputs(build_snapshot.inputs())
        .with_required_outputs(build_snapshot.required_outputs().iter().cloned())?;
    }
    Ok(build_machine_filesystem_scope)
}

#[cfg(test)]
mod tests {
    use super::{
        AtomicU64, BuildMachineFilesystemSponsor, Ordering, Path, prepare_filesystem_scope,
    };
    use build_time_evaluation::BuildMachineFilesystemAccess;
    use package_compilation::BuildSourceCaptureObligation;
    use std::path::PathBuf;

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    fn fresh_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "omega-prepare-scope-{label}-{}-{}",
            std::process::id(),
            NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed),
        ))
    }

    fn fresh_root(label: &str) -> PathBuf {
        let root = fresh_path(label);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the test root");
        root
    }

    fn seal_source_tree(root: &Path, sealed: bool) {
        seal_source_tree_mode(root, sealed);
    }

    #[cfg(unix)]
    fn seal_source_tree_mode(root: &Path, sealed: bool) {
        use std::os::unix::fs::PermissionsExt;

        let metadata = std::fs::symlink_metadata(root).expect("inspect the source member");
        if metadata.is_dir() {
            if !sealed {
                std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                    .expect("unseal the source directory");
            }
            for entry in std::fs::read_dir(root).expect("enumerate the source directory") {
                seal_source_tree_mode(&entry.expect("read the source entry").path(), sealed);
            }
            if sealed {
                std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555))
                    .expect("seal the source directory");
            }
        } else {
            let mode = if sealed { 0o444 } else { 0o644 };
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode))
                .expect("set the source file mode");
        }
    }

    #[cfg(not(unix))]
    fn seal_source_tree_mode(_root: &Path, _sealed: bool) {}

    fn scoped_snapshot_request() -> crate::BuildSnapshotRequest {
        crate::BuildSnapshotRequest::scoped(
            Vec::<Vec<u8>>::new(),
            package_compilation::BuildSourceCaptureRequest::new([(
                b"main.omg".to_vec(),
                BuildSourceCaptureObligation::Required,
            )])
            .expect("a canonical capture request"),
        )
    }

    fn source_read_root(
        scope: &crate::BuildMachineFilesystemScope,
    ) -> (PathBuf, Option<BuildMachineFilesystemSponsor>) {
        match scope.filesystem_access() {
            BuildMachineFilesystemAccess::RealScopedSponsored { grants, sponsor } => {
                (grants.read_roots[0].path().to_path_buf(), Some(sponsor))
            }
            BuildMachineFilesystemAccess::RealScoped(grants) => {
                (grants.read_roots[0].path().to_path_buf(), None)
            }
            other => panic!("a scoped build snapshot stays scoped: {other:?}"),
        }
    }

    fn prepared_scope(
        sponsor: BuildMachineFilesystemSponsor,
    ) -> (PathBuf, PathBuf, crate::BuildMachineFilesystemScope) {
        let project = fresh_root("project");
        std::fs::write(project.join("main.omg"), "machine Main;\n")
            .expect("write the scoped source member");
        seal_source_tree(&project, true);
        let request = scoped_snapshot_request();
        let scope = prepare_filesystem_scope(
            &project.join("main.omg"),
            None,
            &source::SourceMap::default(),
            None,
            None,
            Some(sponsor),
            Some(&request),
        )
        .expect("the scoped snapshot binds a filesystem scope");
        let (snapshot_backing, _) = source_read_root(&scope);
        (project, snapshot_backing, scope)
    }

    /// The snapshot backing's parent is the occurrence's private staging
    /// directory: create-exclusive and owner-only on unix, and deliberately
    /// outside the sponsor session so materialized reads keep bypassing
    /// sponsor accounting like any other outside-session read.
    fn assert_private_backing(snapshot_backing: &Path) {
        let parent = snapshot_backing
            .parent()
            .expect("the snapshot backing has a private parent");
        assert_eq!(
            snapshot_backing.file_name().and_then(|name| name.to_str()),
            Some("captured-source")
        );
        assert!(parent.starts_with(std::env::temp_dir()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::symlink_metadata(parent)
                .expect("inspect the private snapshot parent")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700, "the snapshot parent is owner-only");
        }
    }

    #[test]
    fn captured_source_snapshot_backs_onto_a_private_staging_parent() {
        let sponsor = BuildMachineFilesystemSponsor::create_private(fresh_path("session"))
            .expect("a private session sponsor");
        let session_root = sponsor.session_root().expect("the session root");
        let (project, snapshot_backing, scope) = prepared_scope(sponsor.clone());
        assert_private_backing(&snapshot_backing);
        // The backing must not join the sponsor's accounted namespace:
        // in-session paths stop bypassing sponsor transactions, and a read
        // staged there collides with pending output writes.
        assert!(!snapshot_backing.starts_with(&session_root));
        drop(scope);
        sponsor.dispose_private_staging().expect("dispose");
        let _ = std::fs::remove_dir_all(snapshot_backing.parent().unwrap());
        seal_source_tree(&project, false);
        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn captured_source_snapshot_stays_private_with_a_supplied_session() {
        let supplied = fresh_root("supplied-session");
        let sponsor = BuildMachineFilesystemSponsor::new(&supplied).expect("a session sponsor");
        let session_root = std::fs::canonicalize(&supplied).expect("canonical session root");
        let (project, snapshot_backing, scope) = prepared_scope(sponsor);
        assert_private_backing(&snapshot_backing);
        assert!(!snapshot_backing.starts_with(&session_root));
        drop(scope);
        let _ = std::fs::remove_dir_all(snapshot_backing.parent().unwrap());
        seal_source_tree(&project, false);
        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(supplied);
    }
}
