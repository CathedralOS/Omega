use omega_build_evaluation::{
    BuildFilesystemGrantAccess, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReplayRecordLimits, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildObservationClass, capture_verified_build_filesystem_replay_record,
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
            "omega-build-output-directory-{}-{identity}",
            std::process::id()
        ));
        let session = std::env::temp_dir().join(format!(
            "omega-build-output-directory-session-{}-{identity}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&source);
        let _ = std::fs::remove_dir_all(&session);
        std::fs::create_dir_all(&source).expect("create directory source");
        std::fs::create_dir_all(&session).expect("create directory session");
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
    builder.package("empty-directory");
    let input: &[u8] in Path = builder.source.resolve("main.omg");
    let source_descriptor: i32 = builder.filesystem.open(input, 0);
    let source_buffer: [u8; 64] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_count: i64 = builder.filesystem.read(source_descriptor, &mut source_buffer, 64);
    let source_close: i32 = builder.filesystem.close(source_descriptor);
    let generated: &[u8] in Path = builder.output.resolve("generated");
    let generated_result: i32 = builder.filesystem.create_dir(generated, 493);
    let nested: &[u8] in Path = builder.output.resolve("generated/nested");
    let nested_result: i32 = builder.filesystem.create_dir(nested, 493);
    let sibling: &[u8] in Path = builder.output.resolve("sibling");
    let sibling_result: i32 = builder.filesystem.create_dir(sibling, 493);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write directory build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write directory main source");
    }

    fn write_mixed_sources(&self, target: &str) {
        std::fs::write(
            self.source.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.package("mixed-output-tree");
    let input: &[u8] in Path = builder.source.resolve("main.omg");
    let source_descriptor: i32 = builder.filesystem.open(input, 0);
    let source_buffer: [u8; 64] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_count: i64 = builder.filesystem.read(source_descriptor, &mut source_buffer, 64);
    let source_close: i32 = builder.filesystem.close(source_descriptor);
    let generated: &[u8] in Path = builder.output.resolve("generated");
    let generated_result: i32 = builder.filesystem.create_dir(generated, 493);
    let nested_source: &[u8] in Path = builder.output.resolve("generated/table.omg");
    let output_descriptor: i32 = builder.filesystem.create(nested_source, 438);
    let output_count: i64 = builder.filesystem.write(output_descriptor, "data Table {{}}\n");
    let output_close: i32 = builder.filesystem.close(output_descriptor);
    builder.output.include_source(nested_source);
    let sibling: &[u8] in Path = builder.output.resolve("assets");
    let sibling_result: i32 = builder.filesystem.create_dir(sibling, 493);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write mixed Output-tree build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write mixed Output-tree main source");
    }

    fn sponsored_output(&self) -> (PathBuf, FilesystemSponsor) {
        let session = std::fs::canonicalize(&self.session).expect("canonicalize session");
        let sponsor = FilesystemSponsor::new(&session).expect("create sponsor");
        let output = session.join("output");
        let bound = sponsor.bind_path(&output).expect("bind output");
        let prepared = sponsor
            .prepare_create_directory(&bound)
            .expect("prepare output");
        std::fs::create_dir(&output).expect("create output");
        prepared.commit().expect("commit output");
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

    let metadata = std::fs::symlink_metadata(root).expect("inspect directory test source");
    if metadata.is_dir() {
        if !sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .expect("unseal directory test directory");
        }
        for entry in std::fs::read_dir(root).expect("enumerate directory test source") {
            set_tree_permissions(&entry.expect("read directory test entry").path(), sealed);
        }
        if sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555))
                .expect("seal directory test directory");
        }
    } else if metadata.is_file() {
        std::fs::set_permissions(
            root,
            std::fs::Permissions::from_mode(if sealed { 0o444 } else { 0o644 }),
        )
        .expect("set directory test file permissions");
    }
}

#[cfg(not(unix))]
fn set_tree_permissions(_root: &Path, _sealed: bool) {}

fn package_inputs(source: &Path, package_name: &str) -> PackageCompilationInputs {
    let package = PackageKeyIdentity::from_digest([122; 32]).expect("nonzero package identity");
    let source = PackageSourceBinding::new(package, package_name, source.to_path_buf())
        .with_canonical_source_metadata()
        .expect("capture directory source metadata");
    PackageCompilationInputs::new_package(package, vec![source], Vec::new())
        .expect("directory package input")
}

#[test]
fn empty_output_directory_tree_replays_without_host_output() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source, "empty-directory");
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("exact empty Output directory should receipt");

    let summary = checked
        .build_observation_summary()
        .expect("directory build retains observations");
    assert_eq!(summary.schema_version(), 43);
    assert!(summary.operation_replay_verified());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11, 11, 11]
    );
    let directory = &summary.filesystem_operation_attempts()[3];
    assert_eq!(directory.provider(), BuildFilesystemProvider::RealScoped);
    assert_eq!(
        directory.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(493)
    );
    assert_eq!(
        directory.result(),
        BuildFilesystemOperationResult::Scalar(0)
    );
    assert_eq!(directory.post_error(), 0);
    assert_eq!(
        directory.rooted_path_operand_resolutions()[0].root(),
        BuildFilesystemRoot::Output
    );
    assert_eq!(
        directory.rooted_path_operand_resolutions()[0].relative_path(),
        b"generated"
    );
    assert_eq!(
        directory.authorized_paths()[0].access(),
        BuildFilesystemGrantAccess::Write
    );
    let staged = summary
        .staged_output_tree()
        .expect("empty directory has explicit staged custody");
    assert_eq!(staged.entry_count(), 3);
    assert_eq!(staged.file_bytes(), 0);

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("directory receipt encodes")
        .expect("directory receipt retains custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("directory receipt recovers"),
        record
    );

    std::fs::remove_dir_all(output.join("generated")).expect("remove captured directory subtree");
    std::fs::write(output.join("generated"), "spoofed file")
        .expect("replace directory with host file");
    std::fs::remove_dir(output.join("sibling")).expect("remove captured sibling directory");
    std::fs::write(output.join("sibling"), "second spoofed file")
        .expect("replace sibling directory with host file");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("directory replay must not consult drifted host Output");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed directory retains observations");
    assert!(replayed_summary.operation_replay_verified());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("replayed directory staged custody"),
        staged
    );
}

#[test]
fn mixed_output_tree_with_nested_generated_source_replays_without_host_output() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_mixed_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source, "mixed-output-tree");
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("exact mixed Output tree should receipt");

    let summary = checked
        .build_observation_summary()
        .expect("mixed Output tree retains observations");
    assert!(summary.operation_replay_verified());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11, 1, 5, 8, 11]
    );
    assert_eq!(summary.included_source_handoffs().len(), 1);
    assert_eq!(
        summary.included_source_handoffs()[0].relative_path(),
        b"generated/table.omg"
    );
    assert_eq!(
        summary.included_source_handoffs()[0].filesystem_attempt_ordinal(),
        7
    );
    let staged = summary
        .staged_output_tree()
        .expect("mixed tree has explicit staged custody");
    assert_eq!(staged.entry_count(), 3);
    assert_eq!(staged.file_bytes(), 14);

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("mixed Output-tree receipt encodes")
        .expect("mixed Output-tree receipt retains custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("mixed Output-tree receipt recovers"),
        record
    );

    std::fs::remove_dir_all(output.join("generated")).expect("remove captured generated subtree");
    std::fs::write(output.join("generated"), "spoofed host file")
        .expect("replace generated subtree with host file");
    std::fs::remove_dir(output.join("assets")).expect("remove captured assets directory");
    std::fs::write(output.join("assets"), "second spoofed host file")
        .expect("replace assets directory with host file");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("mixed Output-tree replay must not consult drifted host Output");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed mixed Output tree retains observations");
    assert!(replayed_summary.operation_replay_verified());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("replayed mixed Output tree staged custody"),
        staged
    );
}
