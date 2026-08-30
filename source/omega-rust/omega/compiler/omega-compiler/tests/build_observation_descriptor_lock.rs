use omega_build_evaluation::{
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemOperationResult,
    BuildFilesystemReplayDisposition, BuildFilesystemReplayRecordLimits,
    BuildFilesystemScalarOperandValue, BuildObservationClass,
    capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
use omega_compiler::{
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_sponsored_build_dir,
};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_checked_interpreter::FilesystemSponsor;
use psi_core::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(1);

struct TestProject {
    source: PathBuf,
    session: PathBuf,
}

impl TestProject {
    fn new() -> Self {
        let identity = NEXT_PROJECT.fetch_add(1, Ordering::Relaxed);
        let source = std::env::temp_dir().join(format!(
            "omega-build-output-lock-{}-{identity}",
            std::process::id()
        ));
        let session = std::env::temp_dir().join(format!(
            "omega-build-output-lock-session-{}-{identity}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&session);
        std::fs::create_dir_all(&source).expect("create descriptor-lock source");
        std::fs::create_dir_all(&session).expect("create descriptor-lock session");
        Self { source, session }
    }

    fn write_sources(&self, target: &str) {
        std::fs::write(
            self.source.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.package("descriptor-lock");
    let input: &[u8] in Path = builder.source.resolve("main.omg");
    let source_descriptor: i32 = builder.filesystem.open(input, 0);
    let source_buffer: [u8; 23] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_count: i64 = builder.filesystem.read(source_descriptor, &mut source_buffer, 23);
    let source_close: i32 = builder.filesystem.close(source_descriptor);
    let generated: &[u8] in Path = builder.output.resolve("locked.omg");
    let output_descriptor: i32 = builder.filesystem.create(generated, 438);
    let acquire: i32 = builder.filesystem.lock_file(output_descriptor, 6);
    let release: i32 = builder.filesystem.lock_file(output_descriptor, 8);
    let output_count: i64 = builder.filesystem.write(output_descriptor, "data Locked {{}}\n");
    let output_close: i32 = builder.filesystem.close(output_descriptor);
    builder.output.include_source(generated);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write descriptor-lock build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write descriptor-lock main source");
    }

    fn sponsored_output(&self) -> (PathBuf, FilesystemSponsor) {
        let session = std::fs::canonicalize(&self.session).expect("canonicalize lock session");
        let sponsor = FilesystemSponsor::new(&session).expect("create lock sponsor");
        let output = session.join("output");
        let bound = sponsor.bind_path(&output).expect("bind lock output");
        let prepared = sponsor
            .prepare_create_directory(&bound)
            .expect("prepare lock output");
        std::fs::create_dir(&output).expect("create lock output");
        prepared.commit().expect("commit lock output");
        (output, sponsor)
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        set_tree_permissions(&self.source, false);
        let _ = std::fs::remove_dir_all(&self.source);
        let _ = std::fs::remove_dir_all(&self.session);
    }
}

#[cfg(unix)]
fn set_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(root).expect("inspect lock test source");
    if metadata.is_dir() {
        if !sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .expect("unseal lock test directory");
        }
        for entry in std::fs::read_dir(root).expect("enumerate lock test source") {
            set_tree_permissions(&entry.expect("read lock test entry").path(), sealed);
        }
        if sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555))
                .expect("seal lock test directory");
        }
    } else if metadata.is_file() {
        std::fs::set_permissions(
            root,
            std::fs::Permissions::from_mode(if sealed { 0o444 } else { 0o644 }),
        )
        .expect("set lock test file permissions");
    }
}

#[cfg(not(unix))]
fn set_tree_permissions(_root: &Path, _sealed: bool) {}

fn package_inputs(source: &Path) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([121; 32]).expect("nonzero package identity");
    let source = PackageSourceBinding::new(package, "descriptor-lock", source.to_path_buf())
        .with_canonical_source_metadata()
        .expect("capture descriptor-lock source metadata");
    PackageCompilationInputs::new_package(package, vec![source], Vec::new())
        .expect("descriptor-lock package input")
}

#[test]
fn successful_output_lock_pair_replays_without_host_output() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source);
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("exact successful Output lock pair should receipt");

    let summary = checked
        .build_observation_summary()
        .expect("descriptor-lock build retains observations");
    assert_eq!(summary.schema_version(), 55);
    assert_eq!(
        summary.filesystem_replay_verdict().disposition(),
        BuildFilesystemReplayDisposition::Complete
    );
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 46, 46, 5, 8]
    );
    let acquire = &summary.filesystem_operation_attempts()[4];
    let release = &summary.filesystem_operation_attempts()[5];
    assert_eq!(
        acquire.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(6)
    );
    assert_eq!(
        release.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(8)
    );
    assert_eq!(acquire.result(), BuildFilesystemOperationResult::Scalar(0));
    assert_eq!(release.result(), BuildFilesystemOperationResult::Scalar(0));
    assert_eq!(acquire.post_error(), 0);
    assert_eq!(release.post_error(), 0);
    let BuildFilesystemLogicalHandleInputResolution::Resolved(acquire_identity) =
        acquire.logical_handle_inputs()[0].resolution()
    else {
        panic!("lock acquire descriptor must be resolved")
    };
    let BuildFilesystemLogicalHandleInputResolution::Resolved(release_identity) =
        release.logical_handle_inputs()[0].resolution()
    else {
        panic!("lock release descriptor must be resolved")
    };
    assert_eq!(acquire_identity, release_identity);

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("descriptor-lock receipt encodes")
        .expect("descriptor-lock receipt retains custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("descriptor-lock receipt recovers"),
        record
    );

    std::fs::write(output.join("locked.omg"), "data Spoofed {}\n")
        .expect("drift physical lock output");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("descriptor-lock replay must not consult drifted host Output");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed descriptor lock retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("locked.omg") && source.source.as_ref() == "data Locked {}\n"
    }));
}
