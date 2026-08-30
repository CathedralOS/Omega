use omega_build_evaluation::{
    BuildFilesystemGrantAccess, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReplayRecordLimits, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildObservationClass, capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
use omega_build_output::BuildStagedOutputTree;
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
    let current: &[u8] in Path = builder.output.resolve("generated/current");
    let current_result: i32 = builder.filesystem.symlink("table.omg", current);
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

    fn write_hard_link_sources(&self, target: &str) {
        std::fs::write(
            self.source.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.package("hard-link-output-tree");
    let input: &[u8] in Path = builder.source.resolve("main.omg");
    let source_descriptor: i32 = builder.filesystem.open(input, 0);
    let source_buffer: [u8; 64] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_count: i64 = builder.filesystem.read(source_descriptor, &mut source_buffer, 64);
    let source_close: i32 = builder.filesystem.close(source_descriptor);
    let original: &[u8] in Path = builder.output.resolve("tool");
    let output_descriptor: i32 = builder.filesystem.create(original, 438);
    let output_count: i64 = builder.filesystem.write(output_descriptor, "hard-link payload\n");
    let permissions_result: i32 = builder.filesystem.set_file_permissions(output_descriptor, 493);
    let output_close: i32 = builder.filesystem.close(output_descriptor);
    let alias: &[u8] in Path = builder.output.resolve("tool-alias");
    let hard_link_result: i32 = builder.filesystem.hard_link(original, alias);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write hard-link Output-tree build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write hard-link Output-tree main source");
    }

    fn write_constant_output_sources(&self, target: &str) {
        std::fs::write(
            self.source.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.package("constant-output-tree");
    let artifact: &[u8] in Path = builder.output.resolve("constant.bin");
    let output_descriptor: i32 = builder.filesystem.create(artifact, 438);
    let output_count: i64 = builder.filesystem.write(output_descriptor, "constant output");
    let output_close: i32 = builder.filesystem.close(output_descriptor);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write constant Output-tree build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write constant Output-tree main source");
    }

    fn write_absent_remove_sources(&self, target: &str) {
        std::fs::write(
            self.source.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.package("absent-output-remove");
    let missing: &[u8] in Path = builder.output.resolve("missing.bin");
    let remove_result: i32 = builder.filesystem.remove(missing);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write absent-remove build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write absent-remove main source");
    }

    fn write_source_directory_sources(&self, target: &str) {
        let buffer = std::iter::repeat_n("0", 512).collect::<Vec<_>>().join(", ");
        std::fs::write(
            self.source.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.package("source-directory-read");
    let directory: &[u8] in Path = builder.source.resolve("entries");
    let descriptor: i32 = builder.filesystem.open(directory, 0);
    let buffer: [u8; 512] = [{buffer}];
    let position: i64 = 0;
    let count: i64 = builder.filesystem.read_dir(descriptor, &mut buffer, 512, &mut position);
    let close_result: i32 = builder.filesystem.close(descriptor);
    builder.freestanding = false;
}}
"#,
            ),
        )
        .expect("write Source directory build source");
        std::fs::write(self.source.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write Source directory main source");
        std::fs::create_dir(self.source.join("entries")).expect("create Source directory fixture");
        std::fs::write(self.source.join("entries/item.txt"), "item")
            .expect("write Source directory fixture entry");
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

#[cfg(unix)]
fn assert_normalized_hard_link_output(tree: &BuildStagedOutputTree, destination: &Path) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    std::fs::create_dir(destination).expect("create staged-output materialization directory");
    assert_eq!(
        tree.materialize_into(destination)
            .expect("materialize retained hard-link output"),
        tree.commitment()
    );

    let original = destination.join("tool");
    let alias = destination.join("tool-alias");
    assert_eq!(
        std::fs::read(&original).expect("read retained original"),
        b"hard-link payload\n"
    );
    assert_eq!(
        std::fs::read(&alias).expect("read retained alias"),
        b"hard-link payload\n"
    );

    let original_metadata = std::fs::metadata(&original).expect("inspect retained original");
    let alias_metadata = std::fs::metadata(&alias).expect("inspect retained alias");
    assert!(original_metadata.is_file());
    assert!(alias_metadata.is_file());
    assert_eq!(original_metadata.permissions().mode() & 0o777, 0o755);
    assert_eq!(alias_metadata.permissions().mode() & 0o777, 0o755);
    assert_eq!(original_metadata.nlink(), 1);
    assert_eq!(alias_metadata.nlink(), 1);
    assert_ne!(original_metadata.ino(), alias_metadata.ino());
}

#[test]
fn constant_output_tree_replays_without_an_artificial_source_event() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_constant_output_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source, "constant-output-tree");
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("constant Output tree should receipt without reading Source");

    let summary = checked
        .build_observation_summary()
        .expect("constant Output tree retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![1, 5, 8]
    );
    let staged = summary
        .staged_output_tree()
        .expect("constant Output tree has staged custody");
    assert_eq!(staged.entry_count(), 1);
    assert_eq!(staged.file_bytes(), 15);

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("constant Output receipt encodes")
        .expect("constant Output receipt retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("constant Output receipt recovers");

    std::fs::write(output.join("constant.bin"), "spoofed host output")
        .expect("drift physical constant Output after capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        recovered,
    )
    .expect("constant Output replay must not consult drifted host Output");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed constant Output retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("replayed constant Output staged custody"),
        staged
    );
}

#[test]
fn absent_output_remove_replays_its_exact_failure_without_a_tree_entry() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_absent_remove_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source, "absent-output-remove");
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("authorized absent Output remove should replay as an exact failure");

    let summary = checked
        .build_observation_summary()
        .expect("absent remove retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(summary.schema_version(), 56);
    let [remove] = summary.filesystem_operation_attempts() else {
        panic!("absent remove retains one exact attempt")
    };
    assert_eq!(remove.operation_tag(), 9);
    assert_eq!(remove.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(remove.post_error(), 2);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only replay retains empty staged custody")
            .entry_count(),
        0
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("absent-remove receipt encodes")
        .expect("absent-remove receipt retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("absent-remove receipt recovers");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        recovered,
    )
    .expect("absent-remove replay must not consult host Output");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed absent remove retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );
}

#[test]
fn source_directory_records_restart_with_exact_byte_and_cursor_evidence() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_source_directory_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source, "source-directory-read");
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("Source directory enumeration should receipt");
    let summary = checked
        .build_observation_summary()
        .expect("directory enumeration retains observations");
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 23, 8]
    );
    let read = &summary.filesystem_operation_attempts()[1];
    assert_eq!(read.mutable_i64_operand_resolutions().len(), 1);
    assert_eq!(read.mutable_i64_operands().len(), 1);

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("directory receipt encodes")
        .expect("directory receipt retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("directory receipt recovers");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        recovered,
    )
    .expect("directory receipt restarts without live enumeration");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed directory build retains observations");
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
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
    assert_eq!(summary.schema_version(), 56);
    assert!(summary.filesystem_replay_verdict().is_complete());
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
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
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

#[cfg(unix)]
#[test]
fn mixed_output_tree_with_nested_source_and_symlink_replays_without_host_output() {
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
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11, 1, 5, 8, 20, 11]
    );
    assert_eq!(summary.included_source_handoffs().len(), 1);
    assert_eq!(
        summary.included_source_handoffs()[0].relative_path(),
        b"generated/table.omg"
    );
    assert_eq!(
        summary.included_source_handoffs()[0].filesystem_attempt_ordinal(),
        8
    );
    let staged = summary
        .staged_output_tree()
        .expect("mixed tree has explicit staged custody");
    assert_eq!(staged.entry_count(), 4);
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
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
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

#[cfg(unix)]
#[test]
fn portable_hard_link_output_replays_and_stages_as_equal_regular_files() {
    let profile = omega_target::TargetProfile::host();
    let project = TestProject::new();
    project.write_hard_link_sources(profile.target_name());
    let (output, sponsor) = project.sponsored_output();
    set_tree_permissions(&project.source, true);
    let inputs = package_inputs(&project.source, "hard-link-output-tree");
    let checked = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.source.join("main.omg"),
        &output,
        Some(profile.target_name()),
        inputs.clone(),
        sponsor,
    )
    .expect("exact portable hard-link Output tree should receipt");

    let summary = checked
        .build_observation_summary()
        .expect("hard-link Output tree retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 17, 8, 19]
    );
    let hard_link = summary
        .filesystem_operation_attempts()
        .last()
        .expect("hard-link attempt");
    assert_eq!(hard_link.provider(), BuildFilesystemProvider::RealScoped);
    assert_eq!(
        hard_link.result(),
        BuildFilesystemOperationResult::Scalar(0)
    );
    assert_eq!(hard_link.post_error(), 0);
    assert_eq!(hard_link.rooted_path_operand_resolutions().len(), 2);
    assert_eq!(
        hard_link.rooted_path_operand_resolutions()[0].relative_path(),
        b"tool"
    );
    assert_eq!(
        hard_link.rooted_path_operand_resolutions()[1].relative_path(),
        b"tool-alias"
    );
    assert!(
        hard_link
            .rooted_path_operand_resolutions()
            .iter()
            .all(|path| path.root() == BuildFilesystemRoot::Output)
    );
    assert_eq!(hard_link.authorized_paths().len(), 2);
    assert!(
        hard_link
            .authorized_paths()
            .iter()
            .all(|path| path.access() == BuildFilesystemGrantAccess::Write)
    );

    let staged = summary
        .staged_output_tree()
        .expect("hard-link tree has explicit staged custody");
    assert_eq!(staged.entry_count(), 2);
    assert_eq!(staged.file_bytes(), 18);
    assert_normalized_hard_link_output(staged, &project.session.join("captured-staged"));

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("hard-link Output-tree receipt encodes")
        .expect("hard-link Output-tree receipt retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("hard-link Output-tree receipt recovers");
    assert_eq!(recovered, record);

    std::fs::remove_file(output.join("tool-alias")).expect("remove captured hard-link alias");
    std::fs::remove_file(output.join("tool")).expect("remove captured hard-link original");
    std::fs::write(output.join("tool"), "spoofed host file")
        .expect("replace original with drifted host file");
    std::fs::write(output.join("tool-alias"), "second spoofed host file")
        .expect("replace alias with drifted host file");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.source.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        recovered,
    )
    .expect("hard-link Output-tree replay must not consult drifted host Output");
    set_tree_permissions(&project.source, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed hard-link Output tree retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 17, 8, 19]
    );
    let replayed_staged = replayed_summary
        .staged_output_tree()
        .expect("replayed hard-link Output tree staged custody");
    assert_eq!(replayed_staged, staged);
    assert_normalized_hard_link_output(replayed_staged, &project.session.join("replayed-staged"));
}
