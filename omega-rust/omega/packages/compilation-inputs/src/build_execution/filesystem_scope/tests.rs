//! Filesystem scope coverage: sponsored session ceilings, canonical Source
//! metadata attachment, captured-snapshot custody, write-root alias and
//! drift rejection, and required-output rosters.

use super::BuildMachineFilesystemScope;
// Only the Unix symlink-drift legs recompute the admitted key directly.
#[cfg(unix)]
use super::overlap_key;
use build_output::{CapturedBuildSourceInput, CapturedSourceEntry};
use build_time_evaluation::BuildMachineFilesystemAccess;
use checked_interpreter::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind, FilesystemSponsor, FilesystemSponsorLimits,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static STAGING_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn temporary_staging_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "omega-build-staging-{label}-{}-{}",
        std::process::id(),
        STAGING_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn sponsored_build_roots_share_one_session_entry_ceiling() {
    let session_root = temporary_staging_root("shared-account");
    fs::create_dir(&session_root).expect("create session root");
    let sponsor = FilesystemSponsor::with_limits(
        &session_root,
        FilesystemSponsorLimits {
            maximum_entries: 1,
            maximum_total_logical_bytes: 64,
            maximum_object_extent: 64,
        },
    )
    .expect("create filesystem sponsor");

    let first_build_dir = session_root.join("first-package");
    BuildMachineFilesystemScope::for_root(
        &session_root.join("first-source/main.omg"),
        first_build_dir.clone(),
        Some(sponsor.clone()),
    )
    .ensure_write_roots()
    .expect("first package consumes the one available entry");

    let second_build_dir = session_root.join("second-package");
    let diagnostics = BuildMachineFilesystemScope::for_root(
        &session_root.join("second-source/main.omg"),
        second_build_dir.clone(),
        Some(sponsor.clone()),
    )
    .ensure_write_roots()
    .expect_err("the second package must share the exhausted session ceiling");

    assert_eq!(sponsor.snapshot().unwrap().entries, 1);
    assert!(first_build_dir.is_dir());
    assert!(!second_build_dir.exists());
    assert!(diagnostics[0].to_string().contains("entry limit 1"));

    fs::remove_dir_all(session_root).expect("remove session root");
}

#[test]
fn build_scope_attaches_canonical_metadata_only_to_source() {
    let metadata = checked_interpreter::CanonicalFilesystemMetadataIndex::version_1(
        [0x5a; 32],
        [checked_interpreter::CanonicalFilesystemMetadataRow::new(
            Vec::new(),
            checked_interpreter::CanonicalFilesystemMetadataRowKind::Directory,
        )],
    )
    .expect("construct canonical source metadata");
    let scope = BuildMachineFilesystemScope::for_package_root(
        PathBuf::from("source"),
        PathBuf::from("build"),
        None,
        Some(metadata.clone()),
    );

    let BuildMachineFilesystemAccess::RealScoped(grants) = scope.filesystem_access() else {
        panic!("unsponsored build scope must use scoped real filesystem grants")
    };
    assert_eq!(grants.read_roots.len(), 1);
    assert_eq!(grants.write_roots.len(), 1);
    assert_eq!(grants.read_roots[0].canonical_metadata(), Some(&metadata));
    assert!(grants.write_roots[0].canonical_metadata().is_none());
}

#[test]
fn package_build_scope_rejects_filesystem_authority_without_canonical_source_metadata() {
    let scope = BuildMachineFilesystemScope::for_package_root(
        PathBuf::from("package-root"),
        PathBuf::from("build"),
        None,
        None,
    );

    let diagnostics = scope
        .ensure_canonical_source_metadata()
        .expect_err("package filesystem access must not fall back to physical metadata");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("canonical Source metadata"));
    assert_eq!(scope.source_root, PathBuf::from("package-root"));
}

fn captured_input() -> CapturedBuildSourceInput {
    let index = CanonicalFilesystemMetadataIndex::version_1(
        [0x5a; 32],
        [
            CanonicalFilesystemMetadataRow::new(
                Vec::new(),
                CanonicalFilesystemMetadataRowKind::Directory,
            ),
            CanonicalFilesystemMetadataRow::new(
                b"template.tmpl".to_vec(),
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 6,
                },
            ),
        ],
    )
    .expect("construct canonical source metadata");
    CapturedBuildSourceInput::from_capture_rows(
        index,
        [(
            b"template.tmpl".to_vec(),
            CapturedSourceEntry::file(b"sealed".to_vec(), false),
        )],
    )
    .expect("assemble captured build source input")
}

#[test]
fn benign_snapshot_execution_requires_exact_private_staging_custody() {
    let fixture = temporary_staging_root("benign-custody");
    fs::create_dir(&fixture).unwrap();
    let sponsor = FilesystemSponsor::create_private(fixture.join("private")).unwrap();
    let private_root = sponsor.session_root().unwrap();
    let source = fixture.join("source");
    let scope = BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        private_root.join("output"),
        Some(sponsor.clone()),
        Some(captured_input().canonical_source_metadata().clone()),
    )
    .with_captured_source_input(captured_input(), fixture.join("captured"))
    .unwrap()
    .with_named_inputs(&std::collections::BTreeMap::from([(
        b"template".to_vec(),
        captured_input(),
    )]));
    let access = scope.filesystem_access();
    assert!(scope.is_private_snapshot_execution(&access));
    let BuildMachineFilesystemAccess::RealScopedSponsored { grants, .. } = &access else {
        panic!("captured private scope must be sponsored")
    };
    let mut broader = grants.clone();
    broader
        .read_roots
        .push(build_time_evaluation::BuildMachineFilesystemGrantRoot::new(
            build_time_evaluation::BuildMachineFilesystemGrantRootIdentity::new(9).unwrap(),
            fixture.join("live"),
        ));
    assert!(!scope.is_private_snapshot_execution(
        &BuildMachineFilesystemAccess::RealScopedSponsored {
            grants: broader,
            sponsor: sponsor.clone(),
        }
    ));
    let mut writable_input = grants.clone();
    writable_input
        .write_roots
        .push(grants.read_roots[1].clone());
    assert!(!scope.is_private_snapshot_execution(
        &BuildMachineFilesystemAccess::RealScopedSponsored {
            grants: writable_input,
            sponsor: sponsor.clone(),
        }
    ));
    let mut substituted = grants.clone();
    substituted.read_roots[0] = build_time_evaluation::BuildMachineFilesystemGrantRoot::new(
        super::BUILD_SOURCE_ROOT_IDENTITY,
        fixture.join("different-backing"),
    )
    .with_canonical_metadata(captured_input().canonical_source_metadata().clone());
    assert!(!scope.is_private_snapshot_execution(
        &BuildMachineFilesystemAccess::RealScopedSponsored {
            grants: substituted,
            sponsor: sponsor.clone(),
        }
    ));
    assert!(!scope.is_private_snapshot_execution(&BuildMachineFilesystemAccess::RealUnscoped));
    assert!(
        !scope.is_private_snapshot_execution(&BuildMachineFilesystemAccess::RealScoped(
            grants.clone()
        ))
    );
    let mut live_source = scope.clone();
    live_source.captured_source_input = None;
    live_source.snapshot_dir = None;
    assert!(!live_source.is_private_snapshot_execution(&live_source.filesystem_access()));
    let mut supplied_directory = scope.clone();
    supplied_directory.sponsor = Some(FilesystemSponsor::new(&private_root).unwrap());
    assert!(
        !supplied_directory.is_private_snapshot_execution(&supplied_directory.filesystem_access())
    );
    sponsor.dispose_private_staging().unwrap();
    assert!(!scope.is_private_snapshot_execution(&access));
    fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn captured_snapshot_release_discards_the_private_backing_on_every_exit() {
    let fixture = temporary_staging_root("snapshot-release");
    let source = fixture.join("source");
    let snapshot = fixture.join("snapshot-backing");
    fs::create_dir_all(&source).expect("create source dir");
    let scope = BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        fixture.join("build"),
        None,
        Some(captured_input().canonical_source_metadata().clone()),
    )
    .with_captured_source_input(captured_input(), snapshot.clone())
    .expect("bind the captured source input");
    // The handle is taken before the run, as the executor does, and
    // outlives the scope's own settlement release.
    let release = scope.captured_snapshot_release();

    scope
        .ensure_captured_snapshot()
        .expect("materialize the fresh private snapshot");
    assert!(snapshot.join("template.tmpl").is_file());
    // A run that halts before settlement never reaches
    // `staged_output_tree`; the executor's final release still clears it.
    release.release();
    assert!(
        !snapshot.exists(),
        "the private backing must not outlive the occurrence"
    );
    // Releasing again after the settlement path already ran is inert.
    release.release();
    assert!(!snapshot.exists());
    assert!(
        source.exists(),
        "release never touches the live source root"
    );

    // A scope without a captured input releases nothing.
    BuildMachineFilesystemScope::for_root(&source.join("main.omg"), fixture.join("build"), None)
        .captured_snapshot_release()
        .release();

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn captured_source_scope_rejects_an_index_mismatch_and_overlap() {
    let fixture = temporary_staging_root("snapshot-mismatch");
    let source = fixture.join("source");
    fs::create_dir_all(&source).expect("create source dir");
    let other_index = CanonicalFilesystemMetadataIndex::version_1(
        [0x99; 32],
        [CanonicalFilesystemMetadataRow::new(
            Vec::new(),
            CanonicalFilesystemMetadataRowKind::Directory,
        )],
    )
    .expect("construct other index");
    let scope = BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        fixture.join("build"),
        None,
        Some(other_index),
    );
    let diagnostics = scope
        .with_captured_source_input(captured_input(), fixture.join("snapshot"))
        .expect_err("a captured input must match the validated index");
    assert!(diagnostics[0].to_string().contains("does not match"));

    let scope = BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        fixture.join("build"),
        None,
        None,
    );
    let diagnostics = scope
        .with_captured_source_input(captured_input(), source.clone())
        .expect_err("the snapshot backing must not overlap the source root");
    assert!(diagnostics[0].to_string().contains("source root"));

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn captured_source_scope_rejects_host_alias_spellings_of_the_roots() {
    let fixture = temporary_staging_root("snapshot-alias");
    let source = fixture.join("source");
    let build = fixture.join("build");
    fs::create_dir_all(&source).expect("create source dir");
    fs::create_dir_all(&build).expect("create build dir");

    // A `..` spelling of the write root names the same directory.
    let scope =
        BuildMachineFilesystemScope::for_package_root(source.clone(), build.clone(), None, None);
    let diagnostics = scope
        .with_captured_source_input(
            captured_input(),
            fixture.join("other").join("..").join("build"),
        )
        .expect_err("a parenthesized spelling of the write root still aliases it");
    assert!(diagnostics[0].to_string().contains("build write root"));

    // A `..` spelling of the source root names the same directory.
    let scope =
        BuildMachineFilesystemScope::for_package_root(source.clone(), build.clone(), None, None);
    let diagnostics = scope
        .with_captured_source_input(captured_input(), source.join("nested").join(".."))
        .expect_err("a parenthesized spelling of the source root still aliases it");
    assert!(diagnostics[0].to_string().contains("source root"));

    // A relative spelling of an absolute read root names the same
    // directory once absolutized.
    let scope = BuildMachineFilesystemScope::for_package_root(
        PathBuf::from("aliased-source"),
        PathBuf::from("aliased-output"),
        None,
        None,
    );
    let snapshot = std::env::current_dir()
        .expect("resolve test cwd")
        .join("aliased-source");
    let diagnostics = scope
        .with_captured_source_input(captured_input(), snapshot)
        .expect_err("a relative read root must not hide an absolute alias");
    assert!(diagnostics[0].to_string().contains("source root"));

    // A symlinked ancestor names the same directory.
    #[cfg(unix)]
    {
        let link = fixture.join("link");
        std::os::unix::fs::symlink(&build, &link).expect("symlink the build root");
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            build.clone(),
            None,
            None,
        );
        let diagnostics = scope
            .with_captured_source_input(captured_input(), link.join("snapshot"))
            .expect_err("a symlinked spelling of the write root still aliases it");
        assert!(diagnostics[0].to_string().contains("build write root"));
        fs::remove_file(&link).expect("remove link");
    }

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn build_write_root_rejects_spelling_that_covers_the_source_root() {
    let fixture = temporary_staging_root("covering-alias");
    let source = fixture.join("source");
    fs::create_dir_all(&source).expect("create source dir");

    // The write root covering the read root — through either a plain or
    // a `..` spelling — would make every read-only source writable.
    for build_dir in [fixture.clone(), fixture.join("nested").join("..")] {
        let diagnostics = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            build_dir.clone(),
            None,
            None,
        )
        .ensure_write_roots()
        .expect_err("a write root covering the source root collides with it");
        assert!(
            diagnostics[0].to_string().contains("source root"),
            "unexpected diagnostic: {diagnostics:?}"
        );
        assert!(!fixture.join("nested").exists());
    }

    // The default layout — the write root nested inside the read root —
    // remains admitted.
    BuildMachineFilesystemScope::for_package_root(source.clone(), source.join("build"), None, None)
        .ensure_write_roots()
        .expect("a write root nested inside the source root stays admitted");

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn build_write_root_rejects_alias_of_a_named_input_snapshot() {
    let fixture = temporary_staging_root("named-input-alias");
    let source = fixture.join("source");
    let snapshot = fixture.join("captured");
    fs::create_dir_all(&source).expect("create source dir");

    // A named input's read root is derived as `<snapshot>.input-N`: a
    // sibling spelling of the captured backing that neither the source
    // root fence nor the captured-input fence sees.
    let named_input_root = snapshot.with_extension("input-0");
    for build_dir in [named_input_root.clone(), named_input_root.join("nested")] {
        let diagnostics = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            build_dir.clone(),
            None,
            None,
        )
        .with_captured_source_input(captured_input(), snapshot.clone())
        .expect("snapshot backing does not overlap the source root")
        .with_named_inputs(&std::collections::BTreeMap::from([(
            b"template".to_vec(),
            captured_input(),
        )]))
        .ensure_write_roots()
        .expect_err("a write root overlapping a named input's read root collides with it");
        assert!(
            diagnostics[0].to_string().contains("named input"),
            "unexpected diagnostic: {diagnostics:?}"
        );
        assert!(
            !named_input_root.exists() && !build_dir.exists(),
            "the rejected write root must not be created"
        );
    }

    // A write root unrelated to any named input's derived root remains
    // admitted.
    BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        fixture.join("output"),
        None,
        None,
    )
    .with_captured_source_input(captured_input(), snapshot.clone())
    .expect("snapshot backing does not overlap the source root")
    .with_named_inputs(&std::collections::BTreeMap::from([(
        b"template".to_vec(),
        captured_input(),
    )]))
    .ensure_write_roots()
    .expect("a disjoint write root stays admitted");

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn materialized_write_root_detects_alias_planted_after_admission() {
    let fixture = temporary_staging_root("race-window-alias");
    let source = fixture.join("source");
    let pivot = fixture.join("pivot");
    let build_dir = pivot.join("source");
    fs::create_dir_all(&source).expect("create source dir");
    fs::create_dir_all(&build_dir).expect("create build dir through a concrete ancestor");
    let scope = BuildMachineFilesystemScope::for_root(&source.join("main.omg"), build_dir, None);
    scope
        .verify_materialized_write_root()
        .expect("the materialized write root resolves where it was admitted");

    #[cfg(unix)]
    {
        // The racing swap: the admitted ancestor becomes a symlink onto
        // the fixture, so `pivot/source` resolves as the source root.
        fs::remove_dir_all(&pivot).expect("remove the admitted ancestor");
        std::os::unix::fs::symlink(&fixture, &pivot).expect("plant the racing alias");
        let diagnostics = scope
            .verify_materialized_write_root()
            .expect_err("an alias planted after admission must refuse the write root");
        assert!(
            diagnostics[0].to_string().contains("racing alias"),
            "unexpected diagnostic: {diagnostics:?}"
        );
        // Refusal never deletes through the aliased spelling.
        assert!(source.is_dir());
        assert!(fixture.join("pivot").is_symlink());
    }

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn materialized_write_root_detects_alias_onto_a_named_input_snapshot() {
    let fixture = temporary_staging_root("race-window-input");
    let source = fixture.join("source");
    let snapshot = fixture.join("captured");
    let pivot = fixture.join("pivot");
    // input-0's read root derives as `captured.input-0`; this build dir
    // names it through the admitted `pivot` ancestor.
    let build_dir = pivot.join("captured.input-0");
    fs::create_dir_all(&source).expect("create source dir");
    fs::create_dir_all(&build_dir).expect("create build dir through a concrete ancestor");
    let scope =
        BuildMachineFilesystemScope::for_package_root(source.clone(), build_dir, None, None)
            .with_captured_source_input(captured_input(), snapshot)
            .expect("snapshot backing does not overlap the source root")
            .with_named_inputs(&std::collections::BTreeMap::from([(
                b"template".to_vec(),
                captured_input(),
            )]));
    scope
        .verify_materialized_write_root()
        .expect("the materialized write root resolves where it was admitted");

    #[cfg(unix)]
    {
        // The aliased spelling lands on the snapshot's real location.
        fs::create_dir_all(fixture.join("captured.input-0"))
            .expect("materialize the resolved snapshot target");
        fs::remove_dir_all(&pivot).expect("remove the admitted ancestor");
        std::os::unix::fs::symlink(&fixture, &pivot).expect("plant the racing alias");
        let diagnostics = scope
            .verify_materialized_write_root()
            .expect_err("an alias planted after admission must refuse the write root");
        assert!(
            diagnostics[0].to_string().contains("named input"),
            "unexpected diagnostic: {diagnostics:?}"
        );
    }

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[test]
fn scoped_package_capture_keeps_provenance_but_grants_only_selected_members() {
    let complete = captured_input();
    let selected_metadata = CanonicalFilesystemMetadataIndex::version_1(
        [0x17; 32],
        [CanonicalFilesystemMetadataRow::new(
            Vec::new(),
            CanonicalFilesystemMetadataRowKind::Directory,
        )],
    )
    .unwrap();
    let selected =
        CapturedBuildSourceInput::from_capture_rows(selected_metadata.clone(), []).unwrap();
    let scope = BuildMachineFilesystemScope::for_package_root(
        PathBuf::from("source"),
        PathBuf::from("output"),
        None,
        Some(complete.canonical_source_metadata().clone()),
    )
    .with_scoped_package_source_input(selected, &complete, PathBuf::from("snapshot"))
    .unwrap();
    assert_eq!(
        scope.canonical_source_metadata.as_ref(),
        Some(complete.canonical_source_metadata())
    );
    assert_eq!(
        scope
            .captured_source_inventory()
            .unwrap()
            .source_metadata_identity()
            .source_content_commitment(),
        [0x17; 32]
    );
    let BuildMachineFilesystemAccess::RealScoped(grants) = scope.filesystem_access() else {
        panic!("captured scope must retain scoped reads")
    };
    assert_eq!(
        grants.read_roots[0].canonical_metadata(),
        Some(&selected_metadata)
    );
}

#[test]
fn required_output_roster_rejects_noncanonical_duplicate_and_prefix_names() {
    let scope = || {
        BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("source"),
            PathBuf::from("build"),
            None,
            None,
        )
    };
    assert!(
        scope()
            .with_required_outputs([b"artifact.txt".to_vec()])
            .is_ok()
    );
    for bad in [
        Vec::new(),
        b"./dot".to_vec(),
        b"../parent".to_vec(),
        b"nested//gap".to_vec(),
        b"back\\slash".to_vec(),
        b"/absolute".to_vec(),
    ] {
        assert!(
            scope().with_required_outputs([bad]).is_err(),
            "noncanonical required names reject"
        );
    }
    assert!(
        scope()
            .with_required_outputs([b"a".to_vec(), b"a".to_vec()])
            .is_err(),
        "duplicates reject"
    );
    assert!(
        scope()
            .with_required_outputs([b"dir".to_vec(), b"dir/file".to_vec()])
            .is_err(),
        "file/directory prefix collisions reject"
    );
}

#[test]
fn required_output_completion_is_linear_against_sealed_custody() {
    let scope = BuildMachineFilesystemScope::for_package_root(
        PathBuf::from("source"),
        PathBuf::from("build"),
        None,
        None,
    )
    .with_required_outputs([b"artifact.txt".to_vec()])
    .expect("declare the roster");

    let diagnostics = scope
        .verify_required_outputs(None, "build")
        .expect_err("no staged custody means no completion");
    assert!(diagnostics[0].to_string().contains("staged-output custody"));

    let tree = build_output::from_entries(&[build_output::OutputTreeEntry::regular_file(
        b"artifact.txt",
        b"done",
        false,
    )])
    .expect("sealed output tree");
    scope
        .verify_required_outputs(Some(&tree), "build")
        .expect("the sealed file completes its required output");

    let diagnostics = BuildMachineFilesystemScope::for_package_root(
        PathBuf::from("source"),
        PathBuf::from("build"),
        None,
        None,
    )
    .with_required_outputs([b"omitted.txt".to_vec()])
    .expect("declare the roster")
    .verify_required_outputs(Some(&tree), "build")
    .expect_err("an omitted required member cannot publish");
    assert!(diagnostics[0].to_string().contains("omitted"));

    let dirs_only =
        build_output::from_entries(&[build_output::OutputTreeEntry::directory(b"artifact.txt")])
            .expect("dir tree");
    let diagnostics = scope
        .verify_required_outputs(Some(&dirs_only), "build")
        .expect_err("a directory cannot complete a required file");
    assert!(
        diagnostics[0]
            .to_string()
            .contains("not a sealed regular file")
    );
}

#[cfg(unix)]
#[test]
fn write_root_establishment_rejects_an_ancestor_alias_planted_after_admission() {
    let session_root = temporary_staging_root("write-root-ancestor-alias");
    let ancestor = session_root.join("real-ancestor");
    fs::create_dir_all(&ancestor).expect("create the ancestor directory");
    let redirect_target = session_root.join("redirected-elsewhere");
    fs::create_dir(&redirect_target).expect("create redirect target");
    let build_dir = ancestor.join("build");

    // Admission records the write root's key while the ancestor is real;
    // a host alias planted on that ancestor before establishment must not
    // become the baseline establishment checks against.
    let scope = BuildMachineFilesystemScope::for_root(
        &session_root.join("source/main.omg"),
        build_dir.clone(),
        None,
    );
    fs::remove_dir(&ancestor).expect("remove the admitted ancestor");
    std::os::unix::fs::symlink(&redirect_target, &ancestor)
        .expect("plant the host alias on an admitted ancestor");

    let diagnostics = scope
        .ensure_write_roots()
        .expect_err("an ancestor swapped to a host alias after admission redirects writes outside the fenced root");
    assert!(
        diagnostics[0]
            .to_string()
            .contains("different directory than admission checked"),
        "{diagnostics:?}"
    );
    assert_eq!(
        fs::read_link(&ancestor).expect("the host alias is host-owned, not ours to remove"),
        redirect_target
    );
    assert_eq!(
        fs::read_dir(&redirect_target)
            .expect("read redirect target")
            .count(),
        0,
        "the refusal must precede any write through the alias"
    );

    fs::remove_dir_all(session_root).expect("remove session root");
}

#[cfg(unix)]
#[test]
fn write_root_establishment_rejects_resolution_drift() {
    let session_root = temporary_staging_root("write-root-drift");
    fs::create_dir(&session_root).expect("create session root");
    let real_dir = session_root.join("real-build");
    fs::create_dir(&real_dir).expect("create the admitted directory");
    let spelling = session_root.join("build");

    // An alias resolving to the admitted directory is accepted at
    // admission by design (`overlap_key` canonicalizes existing
    // prefixes); a spelling that resolves differently once the write
    // root exists is not the directory admission checked.
    std::os::unix::fs::symlink(&real_dir, &spelling).expect("plant the host alias");
    let diagnostics = BuildMachineFilesystemScope::for_root(
        &session_root.join("source/main.omg"),
        spelling.clone(),
        None,
    )
    .ensure_write_roots()
    .expect_err("a write root spelled through a link is not the directory admission checked");
    assert!(diagnostics[0].to_string().contains("symbolic link"));

    fs::remove_dir_all(session_root).expect("remove session root");
}

#[cfg(unix)]
#[test]
fn write_root_establishment_rejects_a_host_alias_on_an_ancestor() {
    let session_root = temporary_staging_root("write-root-ancestor-alias");
    fs::create_dir(&session_root).expect("create session root");
    let redirect_target = session_root.join("redirected-elsewhere");
    fs::create_dir(&redirect_target).expect("create redirect target");
    let ancestor = session_root.join("build");
    let build_dir = ancestor.join("inner");

    let scope = BuildMachineFilesystemScope::for_root(
        &session_root.join("source/main.omg"),
        build_dir.clone(),
        None,
    );
    // The key admission computed while the not-yet-created ancestor was
    // still a real component spelling.
    let admitted_build_dir_key = overlap_key(&build_dir);

    // A host alias planted on the ancestor inside the window redirects
    // the spelling; the leaf materializes through it as a real
    // directory, so a symlink check on the write root alone sees nothing.
    std::os::unix::fs::symlink(&redirect_target, &ancestor)
        .expect("plant the host alias on the ancestor");
    fs::create_dir(redirect_target.join("inner")).expect("materialize the leaf through the alias");

    let diagnostics = scope
        .ensure_established_write_root(&admitted_build_dir_key)
        .expect_err("a symlinked ancestor redirects output writes outside the fenced root");
    assert!(
        diagnostics[0]
            .to_string()
            .contains("different directory than admission checked")
    );

    fs::remove_dir_all(session_root).expect("remove session root");
}

#[test]
fn captured_snapshot_rejects_content_planted_after_admission() {
    // The admission fences compare root spellings once; a hostile tree
    // can still plant an alias at the backing path inside the window
    // before the first write. Materialization re-inspects the leaf and
    // rejects rather than following it.
    let fixture = temporary_staging_root("planted-backing");
    let source = fixture.join("source");
    fs::create_dir_all(&source).expect("create source dir");
    let snapshot = fixture.join("snapshot");
    let scope = BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        fixture.join("build"),
        None,
        None,
    )
    .with_captured_source_input(captured_input(), snapshot.clone())
    .expect("a clean backing path is admitted");

    // A regular file occupying the backing path rejects; the failure
    // release cannot remove it, so its bytes stay intact.
    fs::write(&snapshot, b"planted").expect("plant a file at the backing");
    let diagnostics = scope
        .ensure_captured_snapshot()
        .expect_err("a planted file must not become the snapshot backing");
    assert!(
        diagnostics[0]
            .to_string()
            .contains("not a concrete directory"),
        "unexpected diagnostic: {diagnostics:?}"
    );
    assert_eq!(fs::read(&snapshot).unwrap(), b"planted");
    fs::remove_file(&snapshot).expect("remove the planted file");

    // With the window back to clean the same scope materializes.
    scope
        .ensure_captured_snapshot()
        .expect("a clean backing path materializes");
    assert!(snapshot.join("template.tmpl").is_file());

    fs::remove_dir_all(&fixture).expect("remove fixture");
}

#[cfg(unix)]
#[test]
fn named_input_snapshot_rejects_a_symlink_planted_after_admission() {
    // A named input's read root derives as `<snapshot>.input-N` — a
    // spelling the source and write-root fences never see — so the same
    // write-time re-inspection is the only guard on that path.
    let fixture = temporary_staging_root("planted-input-link");
    let source = fixture.join("source");
    fs::create_dir_all(&source).expect("create source dir");
    let target = fixture.join("target");
    fs::write(&target, b"decoy").expect("create the link target file");
    let snapshot = fixture.join("snapshot");
    let input_backing = snapshot.with_extension("input-0");
    let scope = BuildMachineFilesystemScope::for_package_root(
        source.clone(),
        fixture.join("build"),
        None,
        None,
    )
    .with_captured_source_input(captured_input(), snapshot.clone())
    .expect("a clean backing path is admitted")
    .with_named_inputs(&std::collections::BTreeMap::from([(
        b"template".to_vec(),
        captured_input(),
    )]));

    std::os::unix::fs::symlink(&target, &input_backing)
        .expect("plant a symlink at the named input's backing");
    let diagnostics = scope
        .ensure_captured_snapshot()
        .expect_err("a planted symlink must not redirect the named input backing");
    assert!(
        diagnostics[0]
            .to_string()
            .contains("not a concrete directory"),
        "unexpected diagnostic: {diagnostics:?}"
    );
    assert_eq!(
        fs::read(&target).unwrap(),
        b"decoy",
        "the link target is untouched"
    );
    assert!(
        input_backing.symlink_metadata().is_err(),
        "the failed run's release removes the planted link"
    );

    fs::remove_dir_all(&fixture).expect("remove fixture");
}
