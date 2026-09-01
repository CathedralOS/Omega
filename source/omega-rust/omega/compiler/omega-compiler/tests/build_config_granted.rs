//! Granted build-host staging round trip: a build machine with a declared exact
//! toolchain `FilesystemHost` service ceiling runs at compile time through the
//! granted interpreter entry, scoped to source reads and build-output writes,
//! and stages an asset itself while its ordinary `Build` image facts flow into
//! the pipeline. A declared exact toolchain `Console` boundary write is served
//! without incidentally supplying filesystem authority. Fail canaries cover
//! undeclared services and package-authored boundary lookalikes.

use omega_build_evaluation::{
    BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION, BUILD_OBSERVATION_SCHEMA_VERSION,
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemMetadataObservationKind,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReplayDisposition, BuildFilesystemReplayRecordLimits,
    BuildFilesystemReturnedPathCompleteness, BuildFilesystemReturnedPathKind, BuildFilesystemRoot,
    BuildFilesystemScalarOperandValue, BuildObservationClass,
    capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
use omega_compiler::{
    CheckedCompilation, CompileOptions, compile_to_checked,
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir, compile_to_checked_with_replay_record,
};
use omega_package_compilation::{PackageCompilationInputs, PackageSourceBinding};
use psi_checked_interpreter::FilesystemSponsor;

fn compile_terminal(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    omega_compiler::compile(
        omega_compiler::CompileRequest::new(options)
            .with_requested_product(omega_compiler::RequestedCompileProduct::TerminalArtifact),
    )
}

use psi_core::PackageKeyIdentity;
use std::path::{Path, PathBuf};

#[path = "build_config_granted/descriptor_error_state_tests.rs"]
mod descriptor_error_state_tests;

#[path = "build_config_granted/native_error_state_tests.rs"]
mod native_error_state_tests;

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

fn replace_unique_bytes(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    assert_eq!(needle.len(), replacement.len());
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "fixture byte sequence must be unique");
    let mut changed = bytes.to_vec();
    changed[matches[0]..matches[0] + replacement.len()].copy_from_slice(replacement);
    changed
}

fn insert_single_replay_handoff(
    record_bytes: &[u8],
    has_canonical_source_metadata: bool,
    relative_path: &[u8],
    filesystem_attempt_ordinal: u64,
) -> Vec<u8> {
    let handoff_count_offset = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0".len()
        + 2
        + 4
        + 4
        + 1
        + if has_canonical_source_metadata {
            4 + 32
        } else {
            0
        };
    assert_eq!(
        &record_bytes[handoff_count_offset..handoff_count_offset + 8],
        &0u64.to_le_bytes()
    );
    let mut handoff = Vec::new();
    handoff.extend_from_slice(&1u64.to_le_bytes());
    handoff.extend_from_slice(&replay_handoff_lane(
        relative_path,
        filesystem_attempt_ordinal,
    ));
    let mut changed = record_bytes.to_vec();
    changed.splice(handoff_count_offset..handoff_count_offset + 8, handoff);
    changed
}

fn replay_handoff_lane(relative_path: &[u8], filesystem_attempt_ordinal: u64) -> Vec<u8> {
    let mut lane = Vec::new();
    lane.extend_from_slice(&(relative_path.len() as u64).to_le_bytes());
    lane.extend_from_slice(relative_path);
    lane.extend_from_slice(&filesystem_attempt_ordinal.to_le_bytes());
    lane
}

fn rooted_build_probe_project(label: &str, body: &str) -> (PathBuf, omega_target::TargetProfile) {
    let profile = omega_target::TargetProfile::host();
    let body = body
        .replace("self.fake.resolve", "fake_resolve")
        .replace("self.filesystem", "builder.filesystem")
        .replace("self.descriptor", "descriptor")
        .replace("self.code", "code")
        .replace("self.result", "result")
        .replace("self.position", "position")
        .replace("self.small_buffer", "small_buffer")
        .replace("self.buffer", "buffer")
        .replace("self.times", "times");
    let project = std::env::temp_dir().join(format!(
        "omega-rooted-build-probe-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create rooted build probe");
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine fake_resolve<'path>(relative: &'path [u8] in Path) -> &'path [u8] in Path {{
    relative
}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.application("rooted-build-probe");
    let mut descriptor: i32 = 0;
    let mut code: i32 = 0;
    let mut result: i64 = 0;
    let mut position: i64 = 0;
    let mut buffer: [u8; 4096];
    let mut small_buffer: [u8; 1];
    let mut times: [u8; 32];
{body}
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
        ),
    )
    .expect("write rooted build probe");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n")
        .expect("write rooted build probe main");
    (project, profile)
}

fn rooted_build_session(project: &Path, label: &str) -> PathBuf {
    let project_name = project
        .file_name()
        .and_then(|name| name.to_str())
        .expect("rooted build project has a UTF-8 directory name");
    std::env::temp_dir().join(format!("{project_name}-{label}"))
}

#[cfg(unix)]
fn set_canonical_source_tree_permissions(root: &Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::symlink_metadata(root).expect("inspect canonical test Source path");
    if metadata.is_symlink() {
        return;
    }
    if metadata.is_dir() {
        if !sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755))
                .expect("unseal canonical test Source directory");
        }
        for entry in std::fs::read_dir(root).expect("enumerate canonical test Source directory") {
            set_canonical_source_tree_permissions(
                &entry.expect("read canonical test Source entry").path(),
                sealed,
            );
        }
        if sealed {
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555))
                .expect("seal canonical test Source directory");
        }
    } else if metadata.is_file() {
        let mode = if sealed { 0o444 } else { 0o644 };
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(mode))
            .expect("set canonical test Source file permissions");
    }
}

#[cfg(not(unix))]
fn set_canonical_source_tree_permissions(_root: &Path, _sealed: bool) {}

fn compile_rooted_probe_with_sponsored_output(
    project: &std::path::Path,
    profile: omega_target::TargetProfile,
    label: &str,
) -> Result<CheckedCompilation, Vec<psi_diagnostics::Diagnostic>> {
    compile_rooted_probe_with_sponsored_output_seed(project, profile, label, None)
}

fn compile_rooted_probe_with_sponsored_output_seed(
    project: &std::path::Path,
    profile: omega_target::TargetProfile,
    label: &str,
    seed: Option<(&Path, &[u8])>,
) -> Result<CheckedCompilation, Vec<psi_diagnostics::Diagnostic>> {
    let session = rooted_build_session(project, label);
    let _ = std::fs::remove_dir_all(&session);
    std::fs::create_dir(&session).expect("create generated-source review session");
    let session = std::fs::canonicalize(session).expect("canonicalize review session");
    let sponsor = FilesystemSponsor::new(&session).expect("create generated-source sponsor");
    let build_dir = session.join("output");
    let bound = sponsor
        .bind_path(&build_dir)
        .expect("bind generated output");
    let prepared = sponsor
        .prepare_create_directory(&bound)
        .expect("prepare generated output");
    std::fs::create_dir(&build_dir).expect("create generated output");
    prepared.commit().expect("commit generated output");
    if let Some((relative_path, bytes)) = seed {
        let path = sponsor
            .bind_path(build_dir.join(relative_path))
            .expect("bind seeded output");
        let prepared = sponsor
            .prepare_create_object(&path, bytes.len() as u64)
            .expect("prepare seeded output");
        std::fs::write(build_dir.join(relative_path), bytes).expect("write seeded output");
        prepared.commit().expect("commit seeded output");
    }

    let _ = std::fs::remove_dir_all(project.join("build"));
    set_canonical_source_tree_permissions(project, true);
    let result = (|| {
        let package = PackageKeyIdentity::from_digest([97; 32]).expect("nonzero package identity");
        let package_inputs = PackageCompilationInputs::new_package(
            package,
            vec![
                PackageSourceBinding::new(package, "generated-source", project.to_path_buf())
                    .with_canonical_source_metadata()
                    .expect("capture generated-source canonical metadata"),
            ],
            Vec::new(),
        )
        .expect("single-package generated-source input");
        compile_to_checked_with_packages_in_sponsored_build_dir(
            &project.join("main.omg"),
            &build_dir,
            Some(profile.target_name()),
            package_inputs,
            sponsor,
        )
    })();
    set_canonical_source_tree_permissions(project, false);
    result
}

#[test]
fn declared_filesystem_build_machine_stages_at_compile_time() {
    let profile = omega_target::TargetProfile::host();
    let project =
        std::env::temp_dir().join(format!("omega-build-config-granted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    let build_dir = project.join("build");
    let stage = build_dir.join("stage");
    std::fs::create_dir_all(&stage).expect("create project dirs");
    std::fs::create_dir_all(project.join("inputs")).expect("create fixture input directory");
    std::fs::write(project.join("inputs/table.txt"), "table\n").expect("seed fixture input");

    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches
    FilesystemHost
{{
    builder.application("filesystem-staging");
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    builder.log.write_line("build: staging");
    let source_path: &[u8] in Path = builder.source.resolve("inputs/table.txt");
    let mut buffer: [u8; 6];
    let source_descriptor: i32 = builder.filesystem.open(source_path, 0);
    let source_bytes: i64 = builder.filesystem.read(source_descriptor, &mut buffer, 6);
    let source_handle: i64 = builder.filesystem.get_osfhandle(source_descriptor);
    let source_close: i32 = builder.filesystem.close(source_descriptor);
    let staged_path: &[u8] in Path = builder.output.resolve("stage/asset.tmp");
    let staged_descriptor: i32 = builder.filesystem.create(staged_path, 438);
    transition staged_descriptor >= 0 {{
        true -> put(staged_descriptor, builder)
        _ -> done(builder)
    }}
    state put(staged_descriptor: i32, builder: &mut Build) {{
        let duplicate_descriptor: i32 = builder.filesystem.duplicate(staged_descriptor);
        let staged_bytes: i64 = builder.filesystem.write(duplicate_descriptor, "staged by build\n");
        let synchronized: i32 = builder.filesystem.sync(duplicate_descriptor);
        let duplicate_close: i32 = builder.filesystem.close(duplicate_descriptor);
        let staged_close: i32 = builder.filesystem.close(staged_descriptor);
        let staged_path: &[u8] in Path = builder.output.resolve("stage/asset.tmp");
        let final_path: &[u8] in Path = builder.output.resolve("stage/asset.bin");
        let renamed: i32 = builder.filesystem.rename(staged_path, final_path);
        transition true {{ true -> done(builder) _ -> done(builder) }}
    }}
    state done(builder: &mut Build) {{
        builder.freestanding = false;
    }}
}}
"#,
            target = profile.target_name(),
            root_owner = profile.root_slot_owner_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::std::console;
data Main { console: Console; }
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    )
    .expect("write main.omg");

    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("checked build evaluation should succeed");
    assert_eq!(checked.selected_program_entry_machine(), Some("Main::main"));
    let checked_usage = checked
        .build_evaluation_usage()
        .expect("build machine evaluation must publish precursor usage");
    assert_eq!(checked_usage.usage_schema_version, 7);
    assert_eq!(checked_usage.step_schedule_marker, 1);
    assert_eq!(checked_usage.invocation_fuel_ceiling, 10_000_000);
    assert_eq!(checked_usage.sponsor_schema_version, None);
    assert_eq!(checked_usage.session_fuel_ceiling, None);
    assert_eq!(checked_usage.session_build_log_byte_ceiling, None);
    assert_eq!(checked_usage.session_filesystem_attempt_ceiling, None);
    assert_eq!(checked_usage.session_live_filesystem_handle_ceiling, None);
    assert_eq!(checked_usage.session_live_cell_ceiling, None);
    assert_eq!(checked_usage.session_live_text_byte_ceiling, None);
    assert_eq!(checked_usage.session_result_cell_ceiling, None);
    assert_eq!(checked_usage.session_result_text_byte_ceiling, None);
    assert_eq!(checked_usage.session_peak_live_filesystem_handles, 0);
    assert_eq!(checked_usage.session_peak_live_cells, 0);
    assert_eq!(checked_usage.session_peak_live_text_bytes, 0);
    assert!(checked_usage.peak_live_cells > 0);
    assert!(checked_usage.peak_live_text_bytes > 0);
    assert_eq!(
        checked_usage.filesystem_operation_attempts,
        u64::try_from(
            checked
                .build_observation_summary()
                .expect("filesystem build retains observations")
                .filesystem_operation_attempts()
                .len()
        )
        .expect("small test attempt count")
    );
    assert!(checked_usage.fuel_units > 0);
    assert!(checked_usage.fuel_units <= checked_usage.invocation_fuel_ceiling);
    assert!(checked_usage.replay_fuel_units <= checked_usage.invocation_fuel_ceiling);
    assert!(checked_usage.result_cells > 0);
    assert!(checked_usage.replay_result_cells <= checked_usage.result_cells);
    let checked_observations = checked
        .build_observation_summary()
        .expect("build machine evaluation must publish observation evidence");
    assert_eq!(
        checked_observations.schema_version(),
        BUILD_OBSERVATION_SCHEMA_VERSION
    );
    assert_eq!(
        checked_observations.ceiling(),
        BuildObservationClass::Volatile
    );
    assert_eq!(
        checked_observations.realized(),
        BuildObservationClass::Volatile
    );
    assert_eq!(
        checked_observations.filesystem_operation_schema_version(),
        19
    );
    assert!(
        checked_observations.staged_output_tree().is_none(),
        "caller-owned unsponsored build roots do not claim package-review custody"
    );
    let attempts: Vec<_> = checked_observations
        .filesystem_operation_attempts()
        .iter()
        .map(|attempt| {
            let result = match attempt.result() {
                BuildFilesystemOperationResult::Scalar(value) => (0, value),
                BuildFilesystemOperationResult::LogicalHandle(identity) => (
                    1,
                    i64::try_from(identity.get()).expect("fixture identity fits i64"),
                ),
            };
            (
                attempt.operation_tag(),
                attempt.provider(),
                result,
                attempt.post_error(),
            )
        })
        .collect();
    assert_eq!(
        attempts,
        vec![
            (2, BuildFilesystemProvider::RealScoped, (1, 1), 0),
            (4, BuildFilesystemProvider::RealScoped, (0, 6), 0),
            (30, BuildFilesystemProvider::RealScoped, (1, 2), 0),
            (8, BuildFilesystemProvider::RealScoped, (0, 0), 0),
            (1, BuildFilesystemProvider::RealScoped, (1, 3), 0),
            (45, BuildFilesystemProvider::RealScoped, (1, 4), 0),
            (5, BuildFilesystemProvider::RealScoped, (0, 16), 0),
            (43, BuildFilesystemProvider::RealScoped, (0, 0), 0),
            (8, BuildFilesystemProvider::RealScoped, (0, 0), 0),
            (8, BuildFilesystemProvider::RealScoped, (0, 0), 0),
            (18, BuildFilesystemProvider::RealScoped, (0, 0), 0),
        ]
    );
    assert!(
        checked_observations
            .filesystem_operation_attempts()
            .iter()
            .all(|attempt| attempt.grant_refusals().is_empty())
    );
    let rooted_paths: Vec<_> = checked_observations
        .filesystem_operation_attempts()
        .iter()
        .flat_map(|attempt| {
            attempt.authorized_paths().iter().map(|path| {
                (
                    attempt.operation_tag(),
                    path.operand_ordinal(),
                    path.access(),
                    path.root(),
                    path.relative_path().to_vec(),
                )
            })
        })
        .collect();
    assert_eq!(
        rooted_paths,
        vec![
            (
                2,
                0,
                BuildFilesystemGrantAccess::Read,
                BuildFilesystemRoot::Source,
                b"inputs/table.txt".to_vec(),
            ),
            (
                1,
                0,
                BuildFilesystemGrantAccess::Write,
                BuildFilesystemRoot::Output,
                b"stage/asset.tmp".to_vec(),
            ),
            (
                18,
                0,
                BuildFilesystemGrantAccess::Write,
                BuildFilesystemRoot::Output,
                b"stage/asset.tmp".to_vec(),
            ),
            (
                18,
                1,
                BuildFilesystemGrantAccess::Write,
                BuildFilesystemRoot::Output,
                b"stage/asset.bin".to_vec(),
            ),
        ]
    );

    let [
        open,
        read,
        get_osfhandle,
        close_source,
        create,
        duplicate,
        write,
        sync,
        close_clone,
        close_output,
        rename,
    ] = checked_observations.filesystem_operation_attempts()
    else {
        panic!("fixture must retain its complete logical-handle operation sequence")
    };
    assert_eq!(open.scalar_operands().len(), 1);
    assert_eq!(open.scalar_operands()[0].operand_ordinal(), 1);
    assert_eq!(
        open.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(0)
    );
    assert_eq!(read.scalar_operands().len(), 1);
    assert_eq!(
        read.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::U64(6)
    );
    let [read_buffer] = read.mutable_byte_operands() else {
        panic!("read must retain its complete mutable buffer")
    };
    let [read_buffer_resolution] = read.mutable_byte_operand_resolutions() else {
        panic!("read must retain its mutable buffer at operand resolution")
    };
    assert_eq!(read_buffer_resolution.operand_ordinal(), 1);
    assert_eq!(read_buffer_resolution.bytes(), &[0; 6]);
    assert_eq!(read_buffer.operand_ordinal(), 1);
    assert_eq!(read_buffer.pre_bytes(), &[0; 6]);
    assert_eq!(read_buffer.post_bytes(), b"table\n");
    let [observed] = read.observed_byte_regions() else {
        panic!("successful read retains one semantic observed-byte region")
    };
    assert_eq!(observed.output_operand_ordinal(), 1);
    assert_eq!(
        observed.kind(),
        BuildFilesystemObservedByteRegionKind::SequentialFileRead
    );
    assert_eq!(observed.offset(), 0);
    assert_eq!(observed.length(), 6);
    assert_eq!(read.observed_bytes(observed), Some(b"table\n".as_slice()));
    assert_eq!(create.scalar_operands().len(), 1);
    assert_eq!(
        create.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(438)
    );
    assert!(write.scalar_operands().is_empty());
    let [written_bytes] = write.byte_operands() else {
        panic!("write must retain its immutable payload operand")
    };
    assert_eq!(written_bytes.operand_ordinal(), 1);
    assert_eq!(written_bytes.bytes(), b"staged by build\n");
    let source_descriptor = open
        .logical_handle_output()
        .expect("successful open creates a logical descriptor");
    assert_eq!(
        source_descriptor.kind(),
        BuildFilesystemLogicalHandleKind::Descriptor
    );
    assert_eq!(source_descriptor.identity().get(), 1);
    assert_eq!(
        source_descriptor.source(),
        BuildFilesystemLogicalHandleOutputSource::Created
    );
    let [read_source] = read.logical_handle_inputs() else {
        panic!("read retains its descriptor operand")
    };
    assert_eq!(read_source.operand_ordinal(), 0);
    assert_eq!(
        read_source.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Resolved(source_descriptor.identity())
    );
    let borrowed_handle = get_osfhandle
        .logical_handle_output()
        .expect("get_osfhandle retains a borrowed native handle");
    assert_eq!(
        borrowed_handle.kind(),
        BuildFilesystemLogicalHandleKind::Native
    );
    assert_eq!(borrowed_handle.identity().get(), 2);
    assert_eq!(
        borrowed_handle.source(),
        BuildFilesystemLogicalHandleOutputSource::Borrowed(source_descriptor.identity())
    );
    assert_eq!(
        close_source.retired_logical_handles(),
        &[source_descriptor.identity(), borrowed_handle.identity()],
        "closing the descriptor also retires its borrowed native view"
    );

    let output_descriptor = create
        .logical_handle_output()
        .expect("successful create mints a fresh descriptor lifetime");
    assert_eq!(output_descriptor.identity().get(), 3);
    let duplicate_descriptor = duplicate
        .logical_handle_output()
        .expect("successful duplicate mints a distinct descriptor lifetime");
    assert_eq!(duplicate_descriptor.identity().get(), 4);
    assert_eq!(
        duplicate_descriptor.source(),
        BuildFilesystemLogicalHandleOutputSource::Duplicated(output_descriptor.identity())
    );
    for operation in [write, sync, close_clone] {
        let [input] = operation.logical_handle_inputs() else {
            panic!("clone operation retains one descriptor input")
        };
        assert_eq!(
            input.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Resolved(duplicate_descriptor.identity())
        );
    }
    assert_eq!(
        close_clone.retired_logical_handles(),
        &[duplicate_descriptor.identity()]
    );
    assert_eq!(
        close_output.retired_logical_handles(),
        &[output_descriptor.identity()]
    );
    assert!(rename.logical_handle_inputs().is_empty());

    let report = compile_terminal(CompileOptions {
        root_path: PathBuf::from(project.join("main.omg")),
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
    })
    .expect("declared filesystem build.omg should produce Terminal Psi");
    assert!(!report.wrote_output());
    let terminal = psi_terminal_codec::decode_module(
        report
            .artifact()
            .expect("Terminal request retains one canonical artifact")
            .semantic_bytes(),
    )
    .expect("decode retained Terminal semantics");
    let entry = terminal
        .machines
        .iter()
        .find(|machine| machine.id == terminal.entry)
        .expect("Terminal entry names one retained machine");
    let entry_reach = entry
        .published_service_ceiling
        .iter()
        .map(|service| {
            terminal
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .map(|declaration| declaration.identity.as_str())
                .expect("entry reach names one retained service")
        })
        .collect::<Vec<_>>();
    assert_eq!(entry_reach, vec!["Console"]);

    let staged = std::fs::read_to_string(stage.join("asset.bin"))
        .expect("the build machine should have staged stage/asset.bin at compile time");
    assert_eq!(staged, "staged by build\n");

    let sponsored_session = rooted_build_session(&project, "sponsored-review");
    let _ = std::fs::remove_dir_all(&sponsored_session);
    std::fs::create_dir(&sponsored_session).expect("create sponsored review session");
    let sponsored_session =
        std::fs::canonicalize(sponsored_session).expect("canonicalize sponsored review session");
    let sponsor = FilesystemSponsor::new(&sponsored_session).expect("create staging sponsor");
    let sponsored_build = sponsored_session.join("package-output");
    let sponsored_stage = sponsored_build.join("stage");
    for directory in [&sponsored_build, &sponsored_stage] {
        let bound = sponsor
            .bind_path(directory)
            .expect("bind sponsored directory");
        let prepared = sponsor
            .prepare_create_directory(&bound)
            .expect("prepare sponsored directory");
        std::fs::create_dir(directory).expect("create sponsored directory");
        prepared.commit().expect("commit sponsored directory");
    }
    let original_build_source =
        std::fs::read_to_string(project.join("build.omg")).expect("read original build source");
    let sponsored_build_source = original_build_source.replace(
        &stage.display().to_string().replace('\\', "/"),
        &sponsored_stage.display().to_string().replace('\\', "/"),
    );
    std::fs::write(project.join("build.omg"), sponsored_build_source)
        .expect("write sponsored build source");
    let _ = std::fs::remove_dir_all(&build_dir);
    set_canonical_source_tree_permissions(&project, true);
    let package = PackageKeyIdentity::from_digest([91; 32]).expect("nonzero package identity");
    let package_inputs = PackageCompilationInputs::new_package(
        package,
        vec![
            PackageSourceBinding::new(package, "sponsored-build", project.clone())
                .with_canonical_source_metadata()
                .expect("capture sponsored-build canonical metadata"),
        ],
        Vec::new(),
    )
    .expect("single-package compiler input");
    let sponsored = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.join("main.omg"),
        &sponsored_build,
        Some(profile.target_name()),
        package_inputs,
        sponsor,
    );
    set_canonical_source_tree_permissions(&project, false);
    let sponsored = sponsored.expect("sponsored package build must retain staged-output custody");
    let sponsored_tree = sponsored
        .build_observation_summary()
        .and_then(|summary| summary.staged_output_tree())
        .expect("sponsored filesystem build commits its complete staged tree");
    assert_eq!(sponsored_tree.entry_count(), 2);
    assert_eq!(sponsored_tree.file_bytes(), 16);

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&sponsored_session);
}

#[test]
fn declared_filesystem_build_machine_cannot_write_under_source_root() {
    let profile = omega_target::TargetProfile::host();
    let project = std::env::temp_dir().join(format!(
        "omega-build-config-scoped-deny-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(project.join("stage")).expect("create project dirs");

    let forbidden = project.join("stage/blocked.bin");
    let unresolvable = project.join("missing-parent/blocked.bin");
    let rename_from = project.join("stage/rename-from.bin");
    let rename_to = project.join("stage/rename-to.bin");
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.application("source-write-denial");
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    let forbidden: &[u8] in Path = builder.source.resolve("stage/blocked.bin");
    let unresolvable: &[u8] in Path = builder.source.resolve("missing-parent/blocked.bin");
    let absent_output: &[u8] in Path = builder.output.resolve("absent.bin");
    let mixed_from: &[u8] in Path = builder.output.resolve("mixed-from.bin");
    let mixed_to: &[u8] in Path = builder.source.resolve("stage/mixed-to.bin");
    let rename_from: &[u8] in Path = builder.source.resolve("stage/rename-from.bin");
    let rename_to: &[u8] in Path = builder.source.resolve("stage/rename-to.bin");
    let forbidden_descriptor: i32 = builder.filesystem.create(forbidden, 438);
    let unresolved_descriptor: i32 = builder.filesystem.create(unresolvable, 438);
    let failed_close: i32 = builder.filesystem.close(unresolved_descriptor);
    let absent_remove: i32 = builder.filesystem.remove(absent_output);
    let mixed_rename: i32 = builder.filesystem.rename(mixed_from, mixed_to);
    let denied_rename: i32 = builder.filesystem.rename(rename_from, rename_to);
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
            root_owner = profile.root_slot_owner_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::std::console;
data Main { console: Console; }
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    )
    .expect("write main.omg");

    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect(
            "declared filesystem build.omg should compile while denied source write returns fd < 0",
        );
    let observations = checked
        .build_observation_summary()
        .cloned()
        .expect("denied filesystem attempt remains an observed build-host operation");
    assert_eq!(observations.ceiling(), BuildObservationClass::Volatile);
    assert_eq!(observations.realized(), BuildObservationClass::Volatile);
    let [
        denied_create,
        unresolved_create,
        failed_close,
        absent_remove,
        mixed_rename,
        denied_rename,
    ] = observations.filesystem_operation_attempts()
    else {
        panic!("create and rename denials must remain in ordered operation evidence")
    };
    assert_eq!(denied_create.operation_tag(), 1);
    assert_eq!(
        denied_create.provider(),
        BuildFilesystemProvider::RealScoped
    );
    assert_eq!(
        denied_create.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(denied_create.post_error(), 13);
    assert!(denied_create.authorized_paths().is_empty());
    let create_refusal = denied_create
        .grant_refusals()
        .first()
        .expect("denied create must remain in ordered operation evidence");
    assert_eq!(denied_create.grant_refusals().len(), 1);
    assert_eq!(create_refusal.operand_ordinal(), 0);
    assert_eq!(create_refusal.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(
        create_refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );

    assert_eq!(unresolved_create.operation_tag(), 1);
    assert_eq!(
        unresolved_create.provider(),
        BuildFilesystemProvider::RealScoped
    );
    assert_eq!(
        unresolved_create.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(unresolved_create.post_error(), 2);
    assert!(unresolved_create.authorized_paths().is_empty());
    let unresolved_refusal = unresolved_create
        .grant_refusals()
        .first()
        .expect("unresolvable create must retain the failed operand");
    assert_eq!(unresolved_create.grant_refusals().len(), 1);
    assert_eq!(unresolved_refusal.operand_ordinal(), 0);
    assert_eq!(
        unresolved_refusal.access(),
        BuildFilesystemGrantAccess::Write
    );
    assert_eq!(
        unresolved_refusal.reason(),
        BuildFilesystemGrantRefusalReason::Unresolvable
    );

    assert_eq!(failed_close.operation_tag(), 8);
    assert_eq!(
        failed_close.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    let [unknown_descriptor] = failed_close.logical_handle_inputs() else {
        panic!("failed close retains its unresolved descriptor operand")
    };
    assert_eq!(unknown_descriptor.operand_ordinal(), 0);
    assert_eq!(
        unknown_descriptor.kind(),
        BuildFilesystemLogicalHandleKind::Descriptor
    );
    assert_eq!(
        unknown_descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );
    assert!(failed_close.logical_handle_output().is_none());
    assert!(failed_close.retired_logical_handles().is_empty());

    assert_eq!(absent_remove.operation_tag(), 9);
    assert_eq!(
        absent_remove.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(absent_remove.post_error(), 2);
    assert!(absent_remove.grant_refusals().is_empty());
    let [authorized_absent] = absent_remove.authorized_paths() else {
        panic!("host failure after grant authorization must retain the rooted path")
    };
    assert_eq!(authorized_absent.operand_ordinal(), 0);
    assert_eq!(
        authorized_absent.access(),
        BuildFilesystemGrantAccess::Write
    );
    assert_eq!(authorized_absent.root(), BuildFilesystemRoot::Output);
    assert_eq!(authorized_absent.relative_path(), b"absent.bin");

    assert_eq!(mixed_rename.operation_tag(), 18);
    assert_eq!(
        mixed_rename.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(mixed_rename.post_error(), 13);
    let [authorized_from] = mixed_rename.authorized_paths() else {
        panic!("accepted first rename operand must remain visible when the sibling refuses")
    };
    assert_eq!(authorized_from.operand_ordinal(), 0);
    assert_eq!(authorized_from.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(authorized_from.root(), BuildFilesystemRoot::Output);
    assert_eq!(authorized_from.relative_path(), b"mixed-from.bin");
    let [mixed_refusal] = mixed_rename.grant_refusals() else {
        panic!("mixed rename must retain its refused sibling operand")
    };
    assert_eq!(mixed_refusal.operand_ordinal(), 1);
    assert_eq!(mixed_refusal.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(
        mixed_refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );

    assert_eq!(denied_rename.operation_tag(), 18);
    assert_eq!(
        denied_rename.provider(),
        BuildFilesystemProvider::RealScoped
    );
    assert_eq!(
        denied_rename.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(denied_rename.post_error(), 13);
    assert!(denied_rename.authorized_paths().is_empty());
    let rename_refusals: Vec<_> = denied_rename
        .grant_refusals()
        .iter()
        .map(|refusal| {
            (
                refusal.operand_ordinal(),
                refusal.access(),
                refusal.reason(),
            )
        })
        .collect();
    assert_eq!(
        rename_refusals,
        vec![
            (
                0,
                BuildFilesystemGrantAccess::Write,
                BuildFilesystemGrantRefusalReason::OutsideGrantedRoots,
            ),
            (
                1,
                BuildFilesystemGrantAccess::Write,
                BuildFilesystemGrantRefusalReason::OutsideGrantedRoots,
            ),
        ]
    );

    assert!(
        !forbidden.exists(),
        "scoped build machine filesystem access must deny source-tree writes before touching disk"
    );
    assert!(!rename_from.exists());
    assert!(!rename_to.exists());
    assert!(!unresolvable.exists());

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn source_descriptor_cannot_amplify_read_grant_into_mutation_authority() {
    let profile = omega_target::TargetProfile::host();
    let project = std::env::temp_dir().join(format!(
        "omega-build-config-descriptor-grant-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project directory");
    let source_file = project.join("source.txt");
    std::fs::write(&source_file, "source remains immutable\n").expect("seed source file");

    #[cfg(unix)]
    let original_mode = {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        std::fs::set_permissions(&source_file, std::fs::Permissions::from_mode(0o640))
            .expect("set source fixture mode");
        std::fs::metadata(&source_file).unwrap().mode() & 0o7777
    };

    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.application("source-descriptor-mutation-denial");
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    let source_file: &[u8] in Path = builder.source.resolve("source.txt");
    let descriptor: i32 = builder.filesystem.open(source_file, 0);
    let denied_permissions: i32 = builder.filesystem.set_file_permissions(descriptor, 511);
    let denied_lock: i32 = builder.filesystem.lock_file(descriptor, 6);
    let closed: i32 = builder.filesystem.close(descriptor);
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
            root_owner = profile.root_slot_owner_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::std::console;
data Main { console: Console; }
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    )
    .expect("write main.omg");

    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("a denied descriptor metadata mutation is an ordinary build result");
    let observations = checked
        .build_observation_summary()
        .cloned()
        .expect("descriptor mutation denial remains observable");
    let [opened, denied_mutation, denied_lock, closed] =
        observations.filesystem_operation_attempts()
    else {
        panic!("open, denied metadata/lock mutations, and close must remain ordered evidence")
    };
    assert_eq!(opened.operation_tag(), 2);
    let opened_identity = opened
        .logical_handle_output()
        .expect("successful source open creates a logical descriptor")
        .identity();
    assert_eq!(
        opened.result(),
        BuildFilesystemOperationResult::LogicalHandle(opened_identity)
    );
    assert_eq!(denied_mutation.operation_tag(), 17);
    assert_eq!(
        denied_mutation.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(denied_mutation.post_error(), 13);
    let [descriptor_input] = denied_mutation.logical_handle_inputs() else {
        panic!("denied descriptor mutation must name its logical input")
    };
    assert_eq!(
        descriptor_input.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Resolved(opened_identity)
    );
    assert_eq!(denied_lock.operation_tag(), 46);
    assert_eq!(
        denied_lock.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(denied_lock.post_error(), 13);
    let [lock_input] = denied_lock.logical_handle_inputs() else {
        panic!("denied lock must name its logical descriptor input")
    };
    assert_eq!(
        lock_input.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Resolved(opened_identity)
    );
    assert_eq!(closed.operation_tag(), 8);
    assert_eq!(closed.result(), BuildFilesystemOperationResult::Scalar(0));
    assert_eq!(closed.retired_logical_handles(), &[opened_identity]);
    assert_eq!(
        std::fs::read_to_string(&source_file).unwrap(),
        "source remains immutable\n"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(&source_file).unwrap().mode() & 0o7777,
            original_mode,
            "read-authorized source descriptor must not mutate source metadata"
        );
    }

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn failed_filesystem_build_reports_partial_non_admission_evidence() {
    let profile = omega_target::TargetProfile::host();
    let project = std::env::temp_dir().join(format!(
        "omega-build-config-partial-evidence-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    let stage = project.join("build/stage");
    std::fs::create_dir_all(&stage).expect("create staging directory");
    let staged = stage.join("partial.bin");

    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
{{
    builder.application("failing-filesystem-staging");
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    let staged: &[u8] in Path = builder.output.resolve("stage/partial.bin");
    let descriptor: i32 = builder.filesystem.create(staged, 438);
    let mut buffer: [u8; 1];
    let bytes: i64 = builder.filesystem.read(descriptor, &mut buffer, 16777217);
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
            root_owner = profile.root_slot_owner_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(
        project.join("main.omg"),
        r#"use omega::language::std::console;
data Main { console: Console; }
machine Main::main(&mut self) { self.console.exit_process(70); }
"#,
    )
    .expect("write main.omg");

    let diagnostics = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("oversized filesystem transfer must reject build evaluation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("filesystem transfer count exceeds evaluator limit"),
        "{rendered}"
    );
    assert!(
        rendered.contains(
            "partial non-admission filesystem evidence: 2 call(s), 1 evaluator-halted, 0 grant refusal(s)"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("2 scalar operand(s), 0 immutable byte operand(s)"),
        "the successful create mode and rejected transfer count must remain in the prepared prefixes: {rendered}"
    );
    assert!(
        staged.exists(),
        "the diagnostic must acknowledge that a prior staged side effect occurred"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn failed_filesystem_preparation_retains_the_completed_immutable_operand_prefix() {
    let profile = omega_target::TargetProfile::host();
    for (case, last_access, last_write, expected_bytes) in [
        ("first-byte", "bad", "12345678", 1),
        ("final-byte", "12345678", "bad", 2),
    ] {
        let project = std::env::temp_dir().join(format!(
            "omega-build-config-preparation-prefix-{case}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).expect("create preparation-prefix project");
        std::fs::write(
            project.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{{
    builder.application("filesystem-preparation-prefix");
    let result: i32 = builder.filesystem.set_file_time(
        0,
        7,
        "{last_access}",
        "{last_write}"
    );
}}
"#,
                target = profile.target_name(),
            ),
        )
        .expect("write preparation-prefix build.omg");
        std::fs::write(project.join("main.omg"), "const RESULT: u32 = 42;\n")
            .expect("write preparation-prefix main.omg");

        let diagnostics =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect_err("short FILETIME input must reject build evaluation");
        let rendered = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("filesystem FILETIME") && rendered.contains("shorter than 8 bytes"),
            "{case}: {rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "1 scalar operand(s), {expected_bytes} immutable byte operand(s)"
            )),
            "{case}: completed preparation prefix was not retained: {rendered}"
        );

        let _ = std::fs::remove_dir_all(&project);
    }
}

#[test]
fn rooted_build_rejects_unrooted_find_before_operand_preparation() {
    let (project, profile) = rooted_build_probe_project(
        "path-like-preparation-prefix",
        r#"    self.result = self.filesystem.find_first("missing/*", &mut self.small_buffer);"#,
    );
    let diagnostics = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("unrooted find protocol must reject package build evaluation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("unrooted find-cursor protocol not admitted by the Build facet"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("filesystem output requires 320 bytes")
            && !rendered.contains("1 path-like operand(s)"),
        "package rejection must precede operand evaluation and buffer preparation: {rendered}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn failed_filesystem_preparation_retains_the_completed_rooted_path_operand_prefix() {
    let (project, profile) = rooted_build_probe_project(
        "rooted-path-preparation-prefix",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.code = self.filesystem.read_metadata(path, &mut self.small_buffer);"#,
    );
    let diagnostics = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("undersized metadata output must reject build evaluation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("filesystem output requires 144 bytes"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1 rooted-path operand(s)"),
        "the completed Source-rooted path must survive the later buffer-capacity failure: {rendered}"
    );
    assert!(
        rendered.contains("1 mutable-carrier operand(s)"),
        "the completed metadata carrier must survive its capacity failure: {rendered}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn metadata_observation_uses_each_selected_checked_target_layout() {
    let layouts: [(&str, usize, [(usize, usize); 14]); 4] = [
        (
            "macos_arm64",
            144,
            [
                (0, 4),
                (4, 2),
                (6, 2),
                (8, 8),
                (16, 4),
                (20, 4),
                (24, 4),
                (32, 8),
                (48, 8),
                (64, 8),
                (80, 8),
                (96, 8),
                (104, 8),
                (112, 4),
            ],
        ),
        (
            "linux_x86_64",
            144,
            [
                (0, 8),
                (24, 4),
                (16, 8),
                (8, 8),
                (28, 4),
                (32, 4),
                (40, 8),
                (72, 8),
                (88, 8),
                (104, 8),
                (120, 8),
                (48, 8),
                (64, 8),
                (56, 8),
            ],
        ),
        (
            "linux_arm64",
            128,
            [
                (0, 8),
                (16, 4),
                (20, 4),
                (8, 8),
                (24, 4),
                (28, 4),
                (32, 8),
                (72, 8),
                (88, 8),
                (104, 8),
                (120, 8),
                (48, 8),
                (64, 8),
                (56, 4),
            ],
        ),
        (
            "windows_x86_64",
            144,
            [
                (0, 4),
                (6, 2),
                (8, 2),
                (64, 8),
                (72, 4),
                (76, 4),
                (16, 4),
                (32, 8),
                (40, 8),
                (80, 8),
                (48, 8),
                (24, 8),
                (88, 8),
                (96, 4),
            ],
        ),
    ];

    for (target, record_size, fields) in layouts {
        let project = std::env::temp_dir().join(format!(
            "omega-build-metadata-layout-{target}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).expect("create metadata-layout project");
        std::fs::write(
            project.join("build.omg"),
            format!(
                r#"use omega::language::std::filesystem_host;

target {target} {{}}

machine build(builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{{
    builder.application("filesystem-metadata-layout");
    let mut buffer: [u8; 144];
    buffer[36] = 255;
    buffer[143] = 255;
    let path: &[u8] in Path = builder.source.resolve("main.omg");
    let followed: i32 = builder.filesystem.read_metadata(path, &mut buffer);
    let descriptor: i32 = builder.filesystem.open(path, 0);
    let descriptor_metadata: i32 = builder.filesystem.read_file_metadata(descriptor, &mut buffer);
    let closed: i32 = builder.filesystem.close(descriptor);
    let unfollowed: i32 = builder.filesystem.read_symlink_metadata(path, &mut buffer);
    let missing: &[u8] in Path = builder.source.resolve("missing.omg");
    let missing_result: i32 = builder.filesystem.read_metadata(missing, &mut buffer);
}}
"#,
            ),
        )
        .expect("write metadata-layout build.omg");
        std::fs::write(project.join("main.omg"), "const VALUE: u32 = 42;\n")
            .expect("write metadata-layout main.omg");

        let compilation = compile_to_checked(&project.join("main.omg"), Some(target))
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "{target} metadata build failed: {}",
                    diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
        let summary = compilation
            .build_observation_summary()
            .expect("metadata call produces build observations");
        let [followed, open, descriptor, close, unfollowed, missing] =
            summary.filesystem_operation_attempts()
        else {
            panic!("{target}: expected stat/open/fstat/close/lstat/missing-stat operations")
        };
        assert_eq!(
            [
                followed.operation_tag(),
                open.operation_tag(),
                descriptor.operation_tag(),
                close.operation_tag(),
                unfollowed.operation_tag(),
                missing.operation_tag()
            ],
            [38, 2, 39, 8, 40, 38],
            "{target}"
        );
        for (attempt, kind) in [
            (
                followed,
                BuildFilesystemMetadataObservationKind::FollowedPath,
            ),
            (
                descriptor,
                BuildFilesystemMetadataObservationKind::OpenDescriptor,
            ),
            (
                unfollowed,
                BuildFilesystemMetadataObservationKind::UnfollowedFinalPath,
            ),
        ] {
            assert_eq!(
                attempt.result(),
                BuildFilesystemOperationResult::Scalar(0),
                "{target}"
            );
            let [metadata] = attempt.metadata_observations() else {
                panic!("{target}: successful metadata operation must retain one row")
            };
            assert_eq!(metadata.kind(), kind);
            assert_eq!(metadata.output_operand_ordinal(), 1);
            assert_eq!(metadata.referenced_device(), 0);
            assert_eq!(metadata.size(), 23);
        }
        assert_eq!(
            missing.result(),
            BuildFilesystemOperationResult::Scalar(-1),
            "{target}"
        );
        assert!(
            missing.metadata_observations().is_empty(),
            "{target}: failed stat retained metadata"
        );
        let carrier = followed
            .mutable_byte_operands()
            .iter()
            .find(|operand| operand.operand_ordinal() == 1)
            .expect("metadata output carrier")
            .post_bytes();
        assert_eq!(carrier.len(), 144, "{target}");
        let mut occupied = [false; 144];
        for (offset, width) in fields {
            occupied[offset..offset + width].fill(true);
        }
        for (offset, byte) in carrier.iter().copied().enumerate() {
            if !occupied[offset] {
                assert_eq!(
                    byte, 0,
                    "{target}: padding/tail byte {offset} was not zeroed"
                );
            }
        }
        assert!(
            carrier[record_size..].iter().all(|byte| *byte == 0),
            "{target}"
        );
        let (rdev_offset, rdev_width) = fields[6];
        assert!(
            carrier[rdev_offset..rdev_offset + rdev_width]
                .iter()
                .all(|byte| *byte == 0)
        );
        let _ = std::fs::remove_dir_all(&project);
    }
}

#[test]
#[cfg(unix)]
fn source_path_metadata_is_replayed_without_a_filesystem_provider() {
    use std::os::unix::fs::symlink;

    let (project, profile) = rooted_build_probe_project(
        "source-path-metadata-replay",
        r#"    let link: &[u8] in Path = builder.source.resolve("inputs/main.link");
    self.code = self.filesystem.read_metadata(link, &mut self.buffer);
    self.code = self.filesystem.read_symlink_metadata(link, &mut self.buffer);"#,
    );
    std::fs::create_dir(project.join("inputs")).expect("create source metadata input directory");
    symlink("../main.omg", project.join("inputs/main.link"))
        .expect("create source metadata symlink");

    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("source path metadata should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("source metadata build retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    let [followed, unfollowed] = summary.filesystem_operation_attempts() else {
        panic!("source metadata fixture has two events")
    };
    assert_eq!(
        [followed.operation_tag(), unfollowed.operation_tag()],
        [38, 40]
    );
    assert_eq!(
        followed.rooted_path_operand_resolutions()[0].relative_path(),
        b"inputs/main.link"
    );
    assert_eq!(
        followed.authorized_paths()[0].relative_path(),
        b"main.omg",
        "followed metadata must retain its distinct canonical authorized target"
    );
    assert_eq!(
        unfollowed.authorized_paths()[0].relative_path(),
        b"inputs/main.link",
        "no-follow metadata must authorize the inert link itself"
    );
    assert_eq!(
        followed.metadata_observations()[0].kind(),
        BuildFilesystemMetadataObservationKind::FollowedPath
    );
    assert_eq!(
        unfollowed.metadata_observations()[0].kind(),
        BuildFilesystemMetadataObservationKind::UnfollowedFinalPath
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified source metadata record must encode")
        .expect("verified source metadata must retain review-only custody");
    recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
        .expect("canonical source metadata record must recover");

    let metadata = followed.metadata_observations()[0];
    let mut metadata_prefix = Vec::new();
    metadata_prefix.extend_from_slice(&1u64.to_le_bytes());
    metadata_prefix.extend_from_slice(&[1, 0]);
    metadata_prefix.extend_from_slice(&metadata.device().to_le_bytes());
    metadata_prefix.extend_from_slice(&metadata.mode().to_le_bytes());
    let mut wrong_kind_prefix = metadata_prefix.clone();
    wrong_kind_prefix[9] = 2;
    let wrong_kind = replace_unique_bytes(
        record.canonical_bytes(),
        &metadata_prefix,
        &wrong_kind_prefix,
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(&wrong_kind, limits).is_err(),
        "followed metadata cannot be relabeled as no-follow metadata"
    );

    let mut complete_metadata_row = metadata_prefix;
    complete_metadata_row.extend_from_slice(&metadata.link_count().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.inode().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.user().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.group().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.referenced_device().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.access_time().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.modification_time().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.change_time().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.birth_time().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.size().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.blocks_512().to_le_bytes());
    complete_metadata_row.extend_from_slice(&metadata.preferred_block_size().to_le_bytes());
    let mut wrong_field_row = complete_metadata_row.clone();
    wrong_field_row[86..94].copy_from_slice(&(metadata.size() + 1).to_le_bytes());
    let wrong_field_bytes = replace_unique_bytes(
        record.canonical_bytes(),
        &complete_metadata_row,
        &wrong_field_row,
    );
    let wrong_field_record =
        recover_review_only_build_filesystem_replay_record(&wrong_field_bytes, limits)
            .expect("structurally canonical but semantically changed metadata must recover");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        wrong_field_record,
    )
    .expect_err("metadata field that disagrees with its retained carrier must reject replay");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("metadata carrier disagrees with its selected-target semantic row")),
        "metadata/carrier mismatch must fail at the selected-layout gate: {diagnostics:?}"
    );

    let authorized = followed.authorized_paths()[0].relative_path();
    assert_eq!(authorized.len(), b"../x.omg".len());
    let mut authorized_lane = Vec::new();
    authorized_lane.extend_from_slice(&1u64.to_le_bytes());
    authorized_lane.extend_from_slice(&[0, 0, 0]);
    authorized_lane.extend_from_slice(
        &u64::try_from(authorized.len())
            .expect("authorized path length fits u64")
            .to_le_bytes(),
    );
    authorized_lane.extend_from_slice(authorized);
    let mut noncanonical_authorized_lane = authorized_lane.clone();
    let path_start = noncanonical_authorized_lane.len() - authorized.len();
    noncanonical_authorized_lane[path_start..].copy_from_slice(b"../x.omg");
    let noncanonical_authorized = replace_unique_bytes(
        record.canonical_bytes(),
        &authorized_lane,
        &noncanonical_authorized_lane,
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(&noncanonical_authorized, limits,)
            .is_err(),
        "a noncanonical authorized target must not survive recovery"
    );

    let mutable = &followed.mutable_byte_operands()[0];
    let mut mutable_lane = Vec::new();
    mutable_lane.extend_from_slice(&1u64.to_le_bytes());
    mutable_lane.push(1);
    mutable_lane.extend_from_slice(
        &u64::try_from(mutable.pre_bytes().len())
            .expect("metadata pre-carrier length fits u64")
            .to_le_bytes(),
    );
    mutable_lane.extend_from_slice(mutable.pre_bytes());
    mutable_lane.extend_from_slice(
        &u64::try_from(mutable.post_bytes().len())
            .expect("metadata post-carrier length fits u64")
            .to_le_bytes(),
    );
    mutable_lane.extend_from_slice(mutable.post_bytes());
    let mut impossible_padding_lane = mutable_lane.clone();
    *impossible_padding_lane
        .last_mut()
        .expect("metadata carrier is nonempty") = 1;
    let impossible_padding_bytes = replace_unique_bytes(
        record.canonical_bytes(),
        &mutable_lane,
        &impossible_padding_lane,
    );
    let impossible_padding_record =
        recover_review_only_build_filesystem_replay_record(&impossible_padding_bytes, limits)
            .expect("structural recovery defers target-layout carrier validation");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        impossible_padding_record,
    )
    .expect_err("nonzero metadata padding or tail must reject replay");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("metadata carrier disagrees with its selected-target semantic row")),
        "impossible metadata padding must fail at the selected-layout gate: {diagnostics:?}"
    );

    let mut stale_record = record.canonical_bytes().to_vec();
    let version_offset = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0".len();
    stale_record[version_offset..version_offset + 2].copy_from_slice(&4u16.to_le_bytes());
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(&stale_record, limits)
            .expect_err("stale replay record schema must reject")
            .message(),
        "unsupported filesystem replay record version"
    );

    std::fs::write(
        project.join("main.omg"),
        "data Main { value: u8; extra: u64; }\n",
    )
    .expect("change followed host metadata after replay capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record.clone(),
    )
    .expect("reopened metadata replay must not consult changed host metadata");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened metadata replay retains observations");
    assert!(
        replayed_summary
            .filesystem_replay_verdict()
            .replays_source_inputs()
    );
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read metadata replay build");
    let changed_build = original_build.replacen(
        "builder.filesystem.read_metadata(link, &mut buffer)",
        "builder.filesystem.read_symlink_metadata(link, &mut buffer)",
        1,
    );
    assert_ne!(
        changed_build, original_build,
        "fixture must change operation"
    );
    std::fs::write(&build_path, changed_build).expect("change metadata replay operation");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record,
    )
    .expect_err("changed metadata operation order must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("changed order")),
        "metadata replay mismatch must identify changed order: {diagnostics:?}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn source_metadata_and_read_chains_replay_in_exact_order() {
    let (project, profile) = rooted_build_probe_project(
        "source-metadata-read-chain-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.code = self.filesystem.read_metadata(path, &mut self.buffer);
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 5);
    self.code = self.filesystem.close(self.descriptor);
    self.code = self.filesystem.read_symlink_metadata(path, &mut self.buffer);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("ordered source metadata/read events should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("ordered source input build retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![38, 2, 4, 8, 40]
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified ordered source inputs must encode")
        .expect("verified ordered source inputs must retain custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("ordered source input record must recover");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("ordered source inputs must replay without a provider");
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("replayed source inputs retain observations")
            .filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn failed_source_metadata_does_not_claim_source_input_replay() {
    let (project, profile) = rooted_build_probe_project(
        "failed-source-metadata-no-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("missing.omg");
    self.code = self.filesystem.read_metadata(path, &mut self.buffer);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("failed metadata remains an ordinary observed build");
    let summary = compilation
        .build_observation_summary()
        .expect("metadata build retains observations");
    assert_eq!(
        summary.filesystem_replay_verdict().disposition(),
        BuildFilesystemReplayDisposition::NotReplayed
    );
    assert!(!summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(
        capture_verified_build_filesystem_replay_record(
            summary,
            BuildFilesystemReplayRecordLimits::default(),
        )
        .expect("non-replayed metadata summary is not a codec error")
        .is_none()
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
#[ignore = "OPTIONAL-STDLIB-BUILD-PROTOCOL-AND-SEMANTIC-BINDINGS: migrate sponsored reads off std FilesystemHost"]
fn source_open_descriptor_metadata_close_replays_without_a_filesystem_provider() {
    let (project, profile) = rooted_build_probe_project(
        "descriptor-metadata-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.code = self.filesystem.read_file_metadata(self.descriptor, &mut self.buffer);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("descriptor metadata should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("descriptor metadata retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    let [open, metadata_attempt, close] = summary.filesystem_operation_attempts() else {
        panic!("descriptor metadata replay fixture has three attempts")
    };
    assert_eq!(
        [
            open.operation_tag(),
            metadata_attempt.operation_tag(),
            close.operation_tag()
        ],
        [2, 39, 8]
    );
    assert_eq!(
        metadata_attempt.metadata_observations()[0].kind(),
        BuildFilesystemMetadataObservationKind::OpenDescriptor
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified descriptor metadata must encode")
        .expect("verified descriptor metadata retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical descriptor metadata must recover");

    std::fs::write(
        project.join("main.omg"),
        "data Main { value: u8; changed: u64; }\n",
    )
    .expect("change host source after descriptor metadata capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("descriptor metadata replay must not consult changed host metadata");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed descriptor metadata retains observations");
    assert!(
        replayed_summary
            .filesystem_replay_verdict()
            .replays_source_inputs()
    );
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn source_open_read_close_is_replayed_without_a_filesystem_provider() {
    let (project, profile) = rooted_build_probe_project(
        "open-read-close-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("open/read/close source build should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build retains observations");
    assert_eq!(
        summary.filesystem_replay_verdict().schema_version(),
        BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION
    );
    assert_eq!(
        summary.filesystem_replay_verdict().disposition(),
        BuildFilesystemReplayDisposition::SourceInputsOnly
    );
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(!summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Volatile);
    assert!(summary.staged_output_tree().is_none());
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8]
    );
    let [_, read, _] = summary.filesystem_operation_attempts() else {
        panic!("open/read/close replay fixture has three events")
    };
    assert_eq!(
        read.observed_bytes(&read.observed_byte_regions()[0]),
        Some(&b"data Main { value: u8; "[..])
    );
    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified replay record must encode")
        .expect("verified replay must publish review-only custody bytes");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical replay record must recover");
    assert_eq!(recovered, record);
    assert_ne!(record.commitment(), [0; 32]);

    let mut corrupted = record.canonical_bytes().to_vec();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    assert!(
        recover_review_only_build_filesystem_replay_record(&corrupted, limits).is_err(),
        "changed replay semantics must reject"
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(
            &record.canonical_bytes()[..record.canonical_bytes().len() - 1],
            limits,
        )
        .is_err(),
        "truncated replay record must reject"
    );
    let mut wrong_schema = record.canonical_bytes().to_vec();
    let schema_offset = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0".len() + 2;
    wrong_schema[schema_offset..schema_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(&wrong_schema, limits)
            .expect_err("unknown observation schema must reject")
            .message(),
        "unsupported filesystem replay semantic schema"
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(
            record.canonical_bytes(),
            BuildFilesystemReplayRecordLimits::new(record.canonical_bytes().len() - 1, 4_096),
        )
        .is_err(),
        "record byte ceiling must reject before parsing"
    );
    let mut spoofed_lane = record.canonical_bytes().to_vec();
    let record_header_bytes =
        b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0".len() + 2 + 4 + 4 + 1 + 8 + 8;
    let open_byte_lane_offset = record_header_bytes + 2 + 1 + 1 + 8 + 4 + 8 + 1 + 1 + 4;
    let mut fake_byte_operand = Vec::new();
    fake_byte_operand.extend_from_slice(&1u64.to_le_bytes());
    fake_byte_operand.push(0);
    fake_byte_operand.extend_from_slice(&0u64.to_le_bytes());
    spoofed_lane.splice(
        open_byte_lane_offset..open_byte_lane_offset + 8,
        fake_byte_operand,
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(&spoofed_lane, limits).is_err(),
        "an operation-inapplicable lane must not survive semantic recovery"
    );

    std::fs::write(project.join("main.omg"), "data Main { value: u16; }\n")
        .expect("change host source after replay capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record.clone(),
    )
    .expect("reopened replay should not consult the changed host source during build evaluation");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened replay retains observations");
    assert!(
        replayed_summary
            .filesystem_replay_verdict()
            .replays_source_inputs()
    );
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(replayed_summary.ceiling(), BuildObservationClass::Volatile);
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("provider-free replay reconstructs exact empty Output custody")
            .entry_count(),
        0
    );
    let [_, replayed_read, _] = replayed_summary.filesystem_operation_attempts() else {
        panic!("reopened replay has three events")
    };
    assert_eq!(
        replayed_read.observed_bytes(&replayed_read.observed_byte_regions()[0]),
        Some(&b"data Main { value: u8; "[..]),
        "build evaluation must receive the retained bytes, not changed host bytes"
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read build probe");
    let changed_build = original_build.replace(
        "builder.filesystem.read(descriptor, &mut buffer, 23)",
        "builder.filesystem.read(descriptor, &mut buffer, 22)",
    );
    assert_ne!(
        changed_build, original_build,
        "fixture must change replay input"
    );
    std::fs::write(&build_path, changed_build).expect("change build replay input");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record,
    )
    .expect_err("changed authored replay input must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("prepared inputs changed")),
        "replay mismatch must identify changed prepared inputs: {diagnostics:?}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn operand_free_unknown_descriptor_failures_are_receipted_and_replayed_without_a_provider() {
    for (label, statement, operation_tag) in [
        ("close", "self.code = self.filesystem.close(-1);", 8),
        ("sync", "self.code = self.filesystem.sync(-1);", 43),
        (
            "sync-data",
            "self.code = self.filesystem.sync_data(-1);",
            44,
        ),
        (
            "duplicate",
            "self.code = self.filesystem.duplicate(-1);",
            45,
        ),
    ] {
        assert_operand_free_unknown_descriptor_failure_replay(label, statement, operation_tag);
    }
}

#[test]
fn unknown_descriptor_seek_failure_replays_exact_authored_scalars_without_a_provider() {
    let (project, profile) = rooted_build_probe_project(
        "unknown-descriptor-seek-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.position = self.filesystem.seek(-1, -17, 2);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("unknown-descriptor seek failure should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor seek failure retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only seek replay retains exact empty Output custody")
            .entry_count(),
        0
    );
    let [open, read, close, seek] = summary.filesystem_operation_attempts() else {
        panic!("unknown-descriptor seek fixture has one Source chain and one failed seek")
    };
    assert_eq!(
        [
            open.operation_tag(),
            read.operation_tag(),
            close.operation_tag(),
            seek.operation_tag()
        ],
        [2, 4, 8, 10]
    );
    assert_eq!(seek.operation_tag(), 10);
    assert_eq!(seek.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(seek.post_error(), 9);
    assert_eq!(
        seek.scalar_operands()
            .iter()
            .map(|operand| (operand.operand_ordinal(), operand.value()))
            .collect::<Vec<_>>(),
        vec![
            (1, BuildFilesystemScalarOperandValue::I64(-17)),
            (2, BuildFilesystemScalarOperandValue::I32(2)),
        ]
    );
    let [descriptor] = seek.logical_handle_inputs() else {
        panic!("failed seek retains one descriptor input")
    };
    assert_eq!(
        descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified unknown-descriptor seek must encode")
        .expect("verified unknown-descriptor seek retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical unknown-descriptor seek record must recover");
    std::fs::write(
        project.join("main.omg"),
        "data Main { value: u64; changed: u8; }\n",
    )
    .expect("change host source after unknown-descriptor seek capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("unknown-descriptor seek replay must not invoke the host provider");
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("replayed seek retains observations")
            .filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unknown_descriptor_at_failures_replay_exact_authored_inputs_without_a_provider() {
    for (label, statement, operation_tag) in [
        (
            "open-at",
            r#"    self.result = self.filesystem.open_at(-1, "generated.omg", 577);"#,
            14,
        ),
        (
            "unlink-at",
            r#"    self.result = self.filesystem.unlink_at(-1, "generated.omg", 577);"#,
            15,
        ),
    ] {
        assert_unknown_descriptor_at_failure_replay(label, statement, operation_tag);
    }
}

fn assert_unknown_descriptor_at_failure_replay(label: &str, statement: &str, operation_tag: u16) {
    let (project, profile) =
        rooted_build_probe_project(&format!("unknown-descriptor-{label}-replay"), statement);
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("unknown-descriptor at-operation failure should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor at-operation failure retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only at-operation replay retains exact empty Output custody")
            .entry_count(),
        0
    );
    let [at_operation] = summary.filesystem_operation_attempts() else {
        panic!("unknown-descriptor at-operation fixture retains one failed operation")
    };
    assert_eq!(at_operation.operation_tag(), operation_tag);
    assert_eq!(
        at_operation.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(at_operation.post_error(), 9);
    let [component] = at_operation.byte_operands() else {
        panic!("failed at-operation retains one exact relative component")
    };
    assert_eq!(component.operand_ordinal(), 1);
    assert_eq!(component.bytes(), b"generated.omg");
    let [flags] = at_operation.scalar_operands() else {
        panic!("failed at-operation retains one exact flags operand")
    };
    assert_eq!(flags.operand_ordinal(), 2);
    assert_eq!(flags.value(), BuildFilesystemScalarOperandValue::I32(577));
    let [descriptor] = at_operation.logical_handle_inputs() else {
        panic!("failed at-operation retains one descriptor input")
    };
    assert_eq!(
        descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );
    assert!(at_operation.rooted_path_operand_resolutions().is_empty());
    assert!(at_operation.authorized_paths().is_empty());
    assert!(at_operation.grant_refusals().is_empty());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified unknown-descriptor at-operation must encode")
        .expect("verified at-operation failure retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical unknown-descriptor at-operation record must recover");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("unknown-descriptor at-operation replay must not invoke the host provider");
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("replayed at-operation retains observations")
            .filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unknown_descriptor_write_operation_failures_replay_exact_authored_scalars() {
    let fixtures = [
        (
            "set-file-permissions",
            "self.code = self.filesystem.set_file_permissions(-1, 493);",
            17,
            vec![BuildFilesystemScalarOperandValue::U32(493)],
        ),
        (
            "set-length",
            "self.code = self.filesystem.set_len(-1, -17);",
            41,
            vec![BuildFilesystemScalarOperandValue::I64(-17)],
        ),
        (
            "lock-file",
            "self.code = self.filesystem.lock_file(-1, 6);",
            46,
            vec![BuildFilesystemScalarOperandValue::I32(6)],
        ),
        (
            "change-file-owner",
            "self.code = self.filesystem.change_file_owner(-1, -1, -2);",
            49,
            vec![
                BuildFilesystemScalarOperandValue::I32(-1),
                BuildFilesystemScalarOperandValue::I32(-2),
            ],
        ),
    ];

    for (label, statement, operation_tag, scalar_values) in fixtures {
        assert_unknown_descriptor_write_operation_failure_replay(
            label,
            statement,
            operation_tag,
            &scalar_values,
        );
    }
}

#[test]
fn unknown_descriptor_set_file_times_failure_replays_exact_authored_carrier() {
    let (project, profile) = rooted_build_probe_project(
        "unknown-descriptor-set-file-times-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.times[0] = 11;
    self.times[16] = 29;
    self.times[31] = 173;
    self.code = self.filesystem.set_file_times(-1, &mut self.times);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("unknown-descriptor set_file_times failure should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor set_file_times failure retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only set_file_times replay retains empty Output custody")
            .entry_count(),
        0
    );
    let [open, read, close, set_file_times] = summary.filesystem_operation_attempts() else {
        panic!("set_file_times fixture retains one Source chain and one failed operation")
    };
    assert_eq!(
        [
            open.operation_tag(),
            read.operation_tag(),
            close.operation_tag(),
            set_file_times.operation_tag(),
        ],
        [2, 4, 8, 42]
    );
    assert_eq!(
        set_file_times.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(set_file_times.post_error(), 9);
    let [descriptor] = set_file_times.logical_handle_inputs() else {
        panic!("failed set_file_times retains one descriptor input")
    };
    assert_eq!(
        descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );
    let [resolution] = set_file_times.mutable_byte_operand_resolutions() else {
        panic!("failed set_file_times retains one resolution-time carrier")
    };
    let [carrier] = set_file_times.mutable_byte_operands() else {
        panic!("failed set_file_times retains one provider carrier")
    };
    assert_eq!(resolution.operand_ordinal(), 1);
    assert_eq!(resolution.bytes().len(), 32);
    assert_eq!(resolution.bytes()[0], 11);
    assert_eq!(resolution.bytes()[16], 29);
    assert_eq!(resolution.bytes()[31], 173);
    assert_eq!(carrier.operand_ordinal(), 1);
    assert_eq!(resolution.bytes(), carrier.pre_bytes());
    assert_eq!(carrier.pre_bytes(), carrier.post_bytes());
    assert!(set_file_times.authorized_paths().is_empty());
    assert!(set_file_times.grant_refusals().is_empty());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified set_file_times failure must encode")
        .expect("verified set_file_times failure retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical set_file_times failure record must recover");
    std::fs::write(
        project.join("main.omg"),
        "data Main { value: u64; changed: u8; }\n",
    )
    .expect("change host source after set_file_times capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("set_file_times failure replay must not invoke the host provider");
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("replayed set_file_times failure retains observations")
            .filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unknown_descriptor_read_failures_replay_exact_authored_inputs() {
    let fixtures = [
        (
            "read",
            r#"    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[4095] = 173;
    self.result = self.filesystem.read(-1, &mut self.buffer, 23);"#,
            4,
            vec![BuildFilesystemScalarOperandValue::U64(23)],
            vec![4],
        ),
        (
            "read-at-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[4095] = 173;
    self.result = self.filesystem.read_at(-1, &mut self.buffer, 19, -17);"#,
            6,
            vec![
                BuildFilesystemScalarOperandValue::U64(19),
                BuildFilesystemScalarOperandValue::I64(-17),
            ],
            vec![2, 4, 8, 6],
        ),
    ];

    for (label, body, operation_tag, scalar_values, operation_tags) in fixtures {
        let (project, profile) =
            rooted_build_probe_project(&format!("unknown-descriptor-{label}-replay"), body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-descriptor read failure should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-descriptor read failure retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("failure-only read replay retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(-1));
        assert_eq!(attempt.post_error(), 9);
        assert_eq!(
            attempt
                .scalar_operands()
                .iter()
                .map(|operand| operand.value())
                .collect::<Vec<_>>(),
            scalar_values
        );
        let [descriptor] = attempt.logical_handle_inputs() else {
            panic!("failed read retains one descriptor input")
        };
        assert_eq!(
            descriptor.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        let [resolution] = attempt.mutable_byte_operand_resolutions() else {
            panic!("failed read retains one resolution-time carrier")
        };
        let [carrier] = attempt.mutable_byte_operands() else {
            panic!("failed read retains one provider carrier")
        };
        assert_eq!(resolution.operand_ordinal(), 1);
        assert_eq!(resolution.bytes().len(), 4096);
        assert_eq!(resolution.bytes()[0], 11);
        assert_eq!(resolution.bytes()[23], 29);
        assert_eq!(resolution.bytes()[4095], 173);
        assert_eq!(carrier.operand_ordinal(), 1);
        assert_eq!(resolution.bytes(), carrier.pre_bytes());
        assert_eq!(carrier.pre_bytes(), carrier.post_bytes());
        assert!(attempt.observed_byte_regions().is_empty());
        assert!(attempt.authorized_paths().is_empty());
        assert!(attempt.grant_refusals().is_empty());

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified unknown-descriptor read must encode")
            .expect("verified read failure retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical unknown-descriptor read record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after unknown-descriptor read capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("unknown-descriptor read replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed read retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn unknown_descriptor_read_dir_failure_replays_exact_carriers_without_a_provider() {
    let (project, profile) = rooted_build_probe_project(
        "unknown-descriptor-read-dir-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.buffer[0] = 11;
    self.buffer[30] = 29;
    self.buffer[4095] = 173;
    self.position = -19;
    self.result = self.filesystem.read_dir(-1, &mut self.buffer, 31, &mut self.position);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("unknown-descriptor read_dir failure should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor read_dir failure retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only read_dir replay retains empty Output custody")
            .entry_count(),
        0
    );

    let [open, read, close, attempt] = summary.filesystem_operation_attempts() else {
        panic!("unknown-descriptor read_dir fixture retains a Source prefix and one failure")
    };
    assert_eq!(
        [
            open.operation_tag(),
            read.operation_tag(),
            close.operation_tag(),
            attempt.operation_tag(),
        ],
        [2, 4, 8, 23]
    );
    assert_eq!(attempt.operation_tag(), 23);
    assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(attempt.post_error(), 9);
    let [count] = attempt.scalar_operands() else {
        panic!("failed read_dir retains one exact count")
    };
    assert_eq!(count.operand_ordinal(), 2);
    assert_eq!(count.value(), BuildFilesystemScalarOperandValue::U64(31));
    let [descriptor] = attempt.logical_handle_inputs() else {
        panic!("failed read_dir retains one descriptor input")
    };
    assert_eq!(
        descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );
    let [buffer_resolution] = attempt.mutable_byte_operand_resolutions() else {
        panic!("failed read_dir retains one buffer resolution")
    };
    let [buffer] = attempt.mutable_byte_operands() else {
        panic!("failed read_dir retains one provider buffer")
    };
    assert_eq!(buffer_resolution.bytes(), buffer.pre_bytes());
    assert_eq!(buffer.pre_bytes(), buffer.post_bytes());
    assert_eq!(buffer.pre_bytes()[0], 11);
    assert_eq!(buffer.pre_bytes()[30], 29);
    assert_eq!(buffer.pre_bytes()[4095], 173);
    let [position_resolution] = attempt.mutable_i64_operand_resolutions() else {
        panic!("failed read_dir retains one position resolution")
    };
    let [position] = attempt.mutable_i64_operands() else {
        panic!("failed read_dir retains one provider position")
    };
    assert_eq!(position_resolution.value(), -19);
    assert_eq!(position.pre_value(), -19);
    assert_eq!(position.post_value(), -19);
    assert!(attempt.observed_byte_regions().is_empty());
    assert!(attempt.authorized_paths().is_empty());
    assert!(attempt.grant_refusals().is_empty());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified unknown-descriptor read_dir must encode")
        .expect("verified read_dir failure retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical unknown-descriptor read_dir record must recover");
    std::fs::write(
        project.join("main.omg"),
        "data Main { value: u64; changed: u8; }\n",
    )
    .expect("change host source after unknown-descriptor read_dir capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("unknown-descriptor read_dir replay must not invoke the host provider");
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("replayed read_dir failure retains observations")
            .filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unknown_descriptor_write_payload_failures_replay_exact_authored_inputs() {
    let fixtures = [
        (
            "write",
            r#"    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[46] = 173;
    self.buffer[4095] = 197;
    self.result = self.filesystem.write(-1, &self.buffer);"#,
            5,
            None,
            vec![5],
        ),
        (
            "write-at-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[46] = 173;
    self.buffer[4095] = 197;
    self.result = self.filesystem.write_at(-1, &self.buffer, -17);"#,
            7,
            Some(-17),
            vec![2, 4, 8, 7],
        ),
    ];

    for (label, body, operation_tag, offset, operation_tags) in fixtures {
        let (project, profile) =
            rooted_build_probe_project(&format!("unknown-descriptor-{label}-replay"), body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-descriptor write payload should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-descriptor write payload retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("failure-only write replay retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(-1));
        assert_eq!(attempt.post_error(), 9);
        let [payload] = attempt.byte_operands() else {
            panic!("failed write retains one immutable payload")
        };
        assert_eq!(payload.operand_ordinal(), 1);
        assert_eq!(payload.bytes().len(), 4096);
        assert_eq!(payload.bytes()[0], 11);
        assert_eq!(payload.bytes()[23], 29);
        assert_eq!(payload.bytes()[46], 173);
        assert_eq!(payload.bytes()[4095], 197);
        assert_eq!(
            attempt
                .scalar_operands()
                .iter()
                .map(|operand| (operand.operand_ordinal(), operand.value()))
                .collect::<Vec<_>>(),
            offset
                .map(|offset| vec![(2, BuildFilesystemScalarOperandValue::I64(offset),)])
                .unwrap_or_default()
        );
        let [descriptor] = attempt.logical_handle_inputs() else {
            panic!("failed write retains one descriptor input")
        };
        assert_eq!(
            descriptor.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(attempt.authorized_paths().is_empty());
        assert!(attempt.grant_refusals().is_empty());

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified unknown-descriptor write must encode")
            .expect("verified write failure retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical unknown-descriptor write record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after unknown-descriptor write capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("unknown-descriptor write replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed write retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn unknown_descriptor_read_file_metadata_failure_replays_exact_authored_carrier() {
    let fixtures = [
        (
            "read-file-metadata",
            r#"    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[46] = 173;
    self.buffer[4095] = 197;
    self.code = self.filesystem.read_file_metadata(-1, &mut self.buffer);"#,
            vec![39],
        ),
        (
            "read-file-metadata-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[46] = 173;
    self.buffer[4095] = 197;
    self.code = self.filesystem.read_file_metadata(-1, &mut self.buffer);"#,
            vec![2, 4, 8, 39],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) =
            rooted_build_probe_project(&format!("unknown-descriptor-{label}-replay"), body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-descriptor read_file_metadata should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-descriptor read_file_metadata retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("failure-only metadata replay retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.operation_tag(), 39);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(-1));
        assert_eq!(attempt.post_error(), 9);
        let [resolution] = attempt.mutable_byte_operand_resolutions() else {
            panic!("failed read_file_metadata retains one resolution-time carrier")
        };
        let [carrier] = attempt.mutable_byte_operands() else {
            panic!("failed read_file_metadata retains one provider carrier")
        };
        assert_eq!(resolution.operand_ordinal(), 1);
        assert_eq!(resolution.bytes().len(), 4096);
        assert_eq!(resolution.bytes()[0], 11);
        assert_eq!(resolution.bytes()[23], 29);
        assert_eq!(resolution.bytes()[46], 173);
        assert_eq!(resolution.bytes()[4095], 197);
        assert_eq!(carrier.operand_ordinal(), 1);
        assert_eq!(carrier.pre_bytes(), resolution.bytes());
        assert_eq!(carrier.post_bytes(), resolution.bytes());
        assert!(attempt.metadata_observations().is_empty());
        let [descriptor] = attempt.logical_handle_inputs() else {
            panic!("failed read_file_metadata retains one descriptor input")
        };
        assert_eq!(
            descriptor.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(attempt.authorized_paths().is_empty());
        assert!(attempt.grant_refusals().is_empty());

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified unknown-descriptor read_file_metadata must encode")
            .expect("verified metadata failure retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical unknown-descriptor metadata record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after unknown-descriptor metadata capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("unknown-descriptor metadata replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed metadata retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn unknown_descriptor_get_osfhandle_failure_replays_exact_modeled_result() {
    let fixtures = [
        (
            "get-osfhandle",
            "    self.result = self.filesystem.get_osfhandle(-1);",
            vec![30],
        ),
        (
            "get-osfhandle-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.result = self.filesystem.get_osfhandle(-1);"#,
            vec![2, 4, 8, 30],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-descriptor get_osfhandle should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-descriptor get_osfhandle retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("modeled handle failure retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.operation_tag(), 30);
        assert_eq!(attempt.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(-2));
        assert_eq!(attempt.post_error(), 0);
        let [descriptor] = attempt.logical_handle_inputs() else {
            panic!("failed get_osfhandle retains one descriptor input")
        };
        assert_eq!(descriptor.operand_ordinal(), 0);
        assert_eq!(
            descriptor.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(attempt.scalar_operands().is_empty());
        assert!(attempt.byte_operands().is_empty());
        assert!(attempt.mutable_byte_operand_resolutions().is_empty());
        assert!(attempt.mutable_byte_operands().is_empty());
        assert!(attempt.observed_byte_regions().is_empty());
        assert!(attempt.authorized_paths().is_empty());
        assert!(attempt.metadata_observations().is_empty());
        assert!(attempt.grant_refusals().is_empty());
        assert!(attempt.logical_handle_output().is_none());
        assert!(attempt.retired_logical_handles().is_empty());

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified unknown-descriptor get_osfhandle must encode")
            .expect("verified modeled handle failure retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical modeled handle failure record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after modeled handle failure capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("modeled handle failure replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed modeled handle failure retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn unknown_native_handle_close_failure_replays_exact_modeled_result() {
    let fixtures = [
        (
            "close-handle",
            "    self.code = self.filesystem.close_handle(-1);",
            vec![29],
        ),
        (
            "close-handle-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.code = self.filesystem.close_handle(-1);"#,
            vec![2, 4, 8, 29],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-native-handle close should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-native-handle close retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("modeled close failure retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.operation_tag(), 29);
        assert_eq!(attempt.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(0));
        assert_eq!(attempt.post_error(), 6);
        let [handle] = attempt.logical_handle_inputs() else {
            panic!("failed close_handle retains one native-handle input")
        };
        assert_eq!(handle.operand_ordinal(), 0);
        assert_eq!(handle.kind(), BuildFilesystemLogicalHandleKind::Native);
        assert_eq!(
            handle.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(attempt.scalar_operands().is_empty());
        assert!(attempt.byte_operands().is_empty());
        assert!(attempt.mutable_byte_operand_resolutions().is_empty());
        assert!(attempt.mutable_byte_operands().is_empty());
        assert!(attempt.logical_handle_output().is_none());
        assert!(attempt.retired_logical_handles().is_empty());

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified unknown-native-handle close must encode")
            .expect("verified modeled close failure retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical modeled close failure record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after modeled close failure capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("modeled close failure replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed modeled close failure retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn unknown_native_handle_final_path_failure_replays_exact_authored_carrier() {
    let fixtures = [
        (
            "final-path-name-by-handle",
            r#"    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[46] = 173;
    self.result = self.filesystem.final_path_name_by_handle(-1, &mut self.buffer, 47, 0);"#,
            vec![31],
        ),
        (
            "final-path-name-by-handle-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.buffer[0] = 11;
    self.buffer[23] = 29;
    self.buffer[46] = 173;
    self.result = self.filesystem.final_path_name_by_handle(-1, &mut self.buffer, 47, 0);"#,
            vec![2, 4, 8, 31],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-native-handle final path should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-native-handle final path retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("modeled final-path failure retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.operation_tag(), 31);
        assert_eq!(attempt.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(0));
        assert_eq!(attempt.post_error(), 6);
        assert_eq!(
            attempt
                .scalar_operands()
                .iter()
                .map(|operand| (operand.operand_ordinal(), operand.value()))
                .collect::<Vec<_>>(),
            vec![
                (2, BuildFilesystemScalarOperandValue::U64(47)),
                (3, BuildFilesystemScalarOperandValue::U32(0)),
            ]
        );
        let [resolution] = attempt.mutable_byte_operand_resolutions() else {
            panic!("failed final path retains one resolution-time carrier")
        };
        let [carrier] = attempt.mutable_byte_operands() else {
            panic!("failed final path retains one provider carrier")
        };
        assert_eq!(resolution.operand_ordinal(), 1);
        assert_eq!(resolution.bytes().len(), 4096);
        assert_eq!(resolution.bytes()[0], 11);
        assert_eq!(resolution.bytes()[23], 29);
        assert_eq!(resolution.bytes()[46], 173);
        assert_eq!(carrier.pre_bytes(), resolution.bytes());
        assert_eq!(carrier.post_bytes(), resolution.bytes());
        let [handle] = attempt.logical_handle_inputs() else {
            panic!("failed final path retains one native-handle input")
        };
        assert_eq!(handle.kind(), BuildFilesystemLogicalHandleKind::Native);
        assert_eq!(
            handle.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(attempt.returned_paths().is_empty());
        assert!(attempt.logical_handle_output().is_none());
        assert!(attempt.retired_logical_handles().is_empty());

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified unknown-native-handle final path must encode")
            .expect("verified modeled final-path failure retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical modeled final-path failure record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after modeled final-path failure capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("modeled final-path failure replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed modeled final-path failure retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

#[test]
fn unknown_native_handle_mutation_failures_replay_exact_authored_inputs() {
    let fixtures = [
        (
            "set-file-time-invalid-handle",
            r#"    self.buffer[0] = 11;
    self.buffer[4095] = 173;
    self.times[0] = 29;
    self.times[31] = 197;
    self.code = self.filesystem.set_file_time(-1, 37, &self.buffer, &self.times);"#,
            vec![32],
        ),
        (
            "lock-file-ex-invalid-handle-after-source",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    self.times[0] = 41;
    self.times[31] = 211;
    self.code = self.filesystem.lock_file_ex(-1, 1, 0, 4294967295, 4294967295, &mut self.times);"#,
            vec![2, 4, 8, 33],
        ),
        (
            "unlock-file-invalid-handle",
            "    self.code = self.filesystem.unlock_file(-1, 3, 5, 7, 11);",
            vec![34],
        ),
    ];

    for (label, body, operation_tags) in fixtures {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("unknown-native-handle mutation should compile and replay");
        let summary = compilation
            .build_observation_summary()
            .expect("unknown-native-handle mutation retains observations");
        assert!(summary.filesystem_replay_verdict().is_complete());
        assert_eq!(summary.realized(), BuildObservationClass::Receipted);
        assert_eq!(
            summary
                .staged_output_tree()
                .expect("modeled native mutation failure retains empty Output custody")
                .entry_count(),
            0
        );
        assert_eq!(
            summary
                .filesystem_operation_attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            operation_tags
        );
        let attempt = summary.filesystem_operation_attempts().last().unwrap();
        assert_eq!(attempt.provider(), BuildFilesystemProvider::RealScoped);
        assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(0));
        assert_eq!(attempt.post_error(), 6);
        let [handle] = attempt.logical_handle_inputs() else {
            panic!("failed native mutation retains one native-handle input")
        };
        assert_eq!(handle.operand_ordinal(), 0);
        assert_eq!(handle.kind(), BuildFilesystemLogicalHandleKind::Native);
        assert_eq!(
            handle.resolution(),
            BuildFilesystemLogicalHandleInputResolution::Unknown
        );
        assert!(attempt.authorized_paths().is_empty());
        assert!(attempt.grant_refusals().is_empty());
        assert!(attempt.logical_handle_output().is_none());
        assert!(attempt.retired_logical_handles().is_empty());

        match attempt.operation_tag() {
            32 => {
                assert_eq!(
                    attempt
                        .scalar_operands()
                        .iter()
                        .map(|operand| (operand.operand_ordinal(), operand.value()))
                        .collect::<Vec<_>>(),
                    vec![(1, BuildFilesystemScalarOperandValue::I64(37))]
                );
                let [last_access, last_write] = attempt.byte_operands() else {
                    panic!("set_file_time retains both complete FILETIME inputs")
                };
                assert_eq!(last_access.operand_ordinal(), 2);
                assert_eq!(last_access.bytes().len(), 4096);
                assert_eq!(last_access.bytes()[0], 11);
                assert_eq!(last_access.bytes()[4095], 173);
                assert_eq!(last_write.operand_ordinal(), 3);
                assert_eq!(last_write.bytes().len(), 32);
                assert_eq!(last_write.bytes()[0], 29);
                assert_eq!(last_write.bytes()[31], 197);
            }
            33 => {
                assert_eq!(
                    attempt
                        .scalar_operands()
                        .iter()
                        .map(|operand| (operand.operand_ordinal(), operand.value()))
                        .collect::<Vec<_>>(),
                    vec![
                        (1, BuildFilesystemScalarOperandValue::U32(1)),
                        (2, BuildFilesystemScalarOperandValue::U32(0)),
                        (3, BuildFilesystemScalarOperandValue::U32(u32::MAX)),
                        (4, BuildFilesystemScalarOperandValue::U32(u32::MAX)),
                    ]
                );
                let [resolution] = attempt.mutable_byte_operand_resolutions() else {
                    panic!("lock_file_ex retains one resolution-time OVERLAPPED carrier")
                };
                let [carrier] = attempt.mutable_byte_operands() else {
                    panic!("lock_file_ex retains one provider OVERLAPPED carrier")
                };
                assert_eq!(resolution.operand_ordinal(), 5);
                assert_eq!(resolution.bytes().len(), 32);
                assert_eq!(resolution.bytes()[0], 41);
                assert_eq!(resolution.bytes()[31], 211);
                assert_eq!(carrier.pre_bytes(), resolution.bytes());
                assert_eq!(carrier.post_bytes(), resolution.bytes());
            }
            34 => assert_eq!(
                attempt
                    .scalar_operands()
                    .iter()
                    .map(|operand| (operand.operand_ordinal(), operand.value()))
                    .collect::<Vec<_>>(),
                vec![
                    (1, BuildFilesystemScalarOperandValue::U32(3)),
                    (2, BuildFilesystemScalarOperandValue::U32(5)),
                    (3, BuildFilesystemScalarOperandValue::U32(7)),
                    (4, BuildFilesystemScalarOperandValue::U32(11)),
                ]
            ),
            tag => panic!("unexpected native mutation tag {tag}"),
        }

        let limits = BuildFilesystemReplayRecordLimits::default();
        let record = capture_verified_build_filesystem_replay_record(summary, limits)
            .expect("verified native mutation must encode")
            .expect("verified native mutation retains review-only custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
                .expect("canonical native mutation record must recover");
        std::fs::write(
            project.join("main.omg"),
            "data Main { value: u64; changed: u8; }\n",
        )
        .expect("change host source after native mutation capture");
        let replayed = compile_to_checked_with_replay_record(
            &project.join("main.omg"),
            Some(profile.target_name()),
            recovered,
        )
        .expect("native mutation replay must not invoke the host provider");
        assert_eq!(
            replayed
                .build_observation_summary()
                .expect("replayed native mutation retains observations")
                .filesystem_operation_attempts(),
            summary.filesystem_operation_attempts()
        );

        let _ = std::fs::remove_dir_all(project);
    }
}

fn assert_unknown_descriptor_write_operation_failure_replay(
    label: &str,
    statement: &str,
    operation_tag: u16,
    scalar_values: &[BuildFilesystemScalarOperandValue],
) {
    let (project, profile) = rooted_build_probe_project(
        &format!("unknown-descriptor-{label}-replay"),
        &format!("    {statement}"),
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("unknown-descriptor write operation should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor write operation retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only write operation retains empty Output custody")
            .entry_count(),
        0
    );
    let [attempt] = summary.filesystem_operation_attempts() else {
        panic!("unknown-descriptor write fixture has one attempt")
    };
    assert_eq!(attempt.operation_tag(), operation_tag);
    assert_eq!(attempt.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(attempt.post_error(), 9);
    assert_eq!(
        attempt
            .scalar_operands()
            .iter()
            .map(|operand| operand.value())
            .collect::<Vec<_>>(),
        scalar_values
    );
    let [descriptor] = attempt.logical_handle_inputs() else {
        panic!("failed write operation retains one descriptor input")
    };
    assert_eq!(
        descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );
    assert!(attempt.authorized_paths().is_empty());
    assert!(attempt.grant_refusals().is_empty());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified unknown-descriptor write operation must encode")
        .expect("verified write operation retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical unknown-descriptor write operation must recover");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("unknown-descriptor write operation replay must not invoke the host provider");
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("replayed write operation retains observations")
            .filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(project);
}

fn assert_operand_free_unknown_descriptor_failure_replay(
    label: &str,
    statement: &str,
    operation_tag: u16,
) {
    let (project, profile) = rooted_build_probe_project(
        &format!("unknown-descriptor-{label}-replay"),
        &format!("    {statement}"),
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("operand-free unknown-descriptor failure should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor failure retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("failure-only replay retains exact empty Output custody")
            .entry_count(),
        0
    );
    let [failed_operation] = summary.filesystem_operation_attempts() else {
        panic!("unknown-descriptor failure fixture has one attempt")
    };
    assert_eq!(failed_operation.operation_tag(), operation_tag);
    assert_eq!(
        failed_operation.provider(),
        BuildFilesystemProvider::RealScoped
    );
    assert_eq!(
        failed_operation.result(),
        BuildFilesystemOperationResult::Scalar(-1)
    );
    assert_eq!(failed_operation.post_error(), 9);
    let [unknown_descriptor] = failed_operation.logical_handle_inputs() else {
        panic!("failed operation retains one logical descriptor input")
    };
    assert_eq!(
        unknown_descriptor.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Unknown
    );
    assert!(failed_operation.retired_logical_handles().is_empty());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified unknown-descriptor failure must encode")
        .expect("verified unknown-descriptor failure retains review-only custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("canonical unknown-descriptor failure record must recover");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        recovered,
    )
    .expect("unknown-descriptor failure replay must not invoke the host provider");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed unknown-descriptor failure retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn source_only_replay_requires_exact_empty_sponsored_output_custody() {
    let (project, profile) = rooted_build_probe_project(
        "source-only-empty-output-receipt",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "empty-output-receipt")
            .expect("source-only replay with exact empty Output custody should compile");
    let summary = checked
        .build_observation_summary()
        .expect("source-only receipt retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.ceiling(), BuildObservationClass::Volatile);
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("empty Output tree remains explicit custody")
            .entry_count(),
        0
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("source-only receipt should encode")
        .expect("source-only receipt should retain replay custody");
    let package = PackageKeyIdentity::from_digest([97; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source =
        PackageSourceBinding::new(package, "source-only-empty-output", project.clone())
            .with_canonical_source_metadata()
            .expect("capture source-only replay metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package source-only replay input");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("source-only receipt should reopen without host Output custody");
    set_canonical_source_tree_permissions(&project, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened source-only receipt retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("reopened receipt reconstructs empty Output")
            .entry_count(),
        0
    );

    let diagnostics = compile_rooted_probe_with_sponsored_output_seed(
        &project,
        profile,
        "unexpected-output-reject",
        Some((Path::new("unexpected.bin"), b"unexplained output")),
    )
    .expect_err("an unexplained physical Output entry must reject receipt issuance");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("Output tree that differs from sponsored staged-output custody")),
        "unexpected diagnostics: {diagnostics:?}"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "empty-output-receipt"));
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "unexpected-output-reject"));
}

#[test]
fn ordinary_output_file_replays_without_generated_source_handoff() {
    let (project, profile) = rooted_build_probe_project(
        "ordinary-output-file-receipt",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let artifact: &[u8] in Path = builder.output.resolve("artifact.bin");
    self.descriptor = self.filesystem.create(artifact, 438);
    self.result = self.filesystem.write(self.descriptor, "ordinary artifact");
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "ordinary-output-review")
            .expect("ordinary output with exact sponsored custody should compile");
    let summary = checked
        .build_observation_summary()
        .expect("ordinary output receipt retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert!(summary.included_source_handoffs().is_empty());
    let staged = summary
        .staged_output_tree()
        .expect("ordinary output retains exact staged-tree custody");
    assert_eq!(staged.entry_count(), 1);
    assert_eq!(staged.file_bytes(), b"ordinary artifact".len() as u64);
    let staged_digest = staged.digest();

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("ordinary output receipt should encode")
        .expect("ordinary output receipt should retain replay custody");
    let changed_handoff_bytes = insert_single_replay_handoff(
        record.canonical_bytes(),
        summary.canonical_source_metadata_identity().is_some(),
        b"artifact.bin",
        summary.filesystem_operation_attempts().len() as u64,
    );
    let changed_handoff =
        recover_review_only_build_filesystem_replay_record(&changed_handoff_bytes, limits)
            .expect("changed handoff disposition remains canonically framed");

    let package = PackageKeyIdentity::from_digest([98; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "ordinary-output", project.clone())
        .with_canonical_source_metadata()
        .expect("capture ordinary-output canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package ordinary-output input");
    let changed_diagnostics = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs.clone(),
        changed_handoff,
    )
    .expect_err("an invented generated-source handoff must reject replay");
    assert!(changed_diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("handoff") && diagnostic.message.contains("changed")
    }));

    std::fs::write(
        rooted_build_session(&project, "ordinary-output-review").join("output/artifact.bin"),
        "host drift",
    )
    .expect("change physical Output after receipt capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("ordinary output should reopen without consulting drifted host Output");
    set_canonical_source_tree_permissions(&project, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened ordinary output retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert!(replayed_summary.included_source_handoffs().is_empty());
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("replay reconstructs ordinary output tree")
            .digest(),
        staged_digest
    );
    assert!(
        replayed
            .typed
            .symbols
            .source_files()
            .all(|source| !source.path.ends_with("artifact.bin")),
        "ordinary output must not become generated Omega source"
    );

    let mismatch = compile_rooted_probe_with_sponsored_output_seed(
        &project,
        profile,
        "ordinary-output-mismatch",
        Some((Path::new("unexpected.bin"), b"unexplained output")),
    )
    .expect_err("an unexplained physical Output entry must reject receipt issuance");
    assert!(mismatch.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Output tree that differs from sponsored staged-output custody")
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "ordinary-output-review"));
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "ordinary-output-mismatch"));
}

#[test]
fn multiple_ordinary_output_files_replay_as_one_exact_tree() {
    let (project, profile) = rooted_build_probe_project(
        "multiple-ordinary-output-files-receipt",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let artifact: &[u8] in Path = builder.output.resolve("artifact.bin");
    self.descriptor = self.filesystem.create(artifact, 438);
    self.result = self.filesystem.write(self.descriptor, "ordinary artifact");
    self.code = self.filesystem.close(self.descriptor);
    let metadata: &[u8] in Path = builder.output.resolve("metadata.bin");
    self.descriptor = self.filesystem.create(metadata, 438);
    self.result = self.filesystem.write(self.descriptor, "ordinary metadata");
    self.code = self.filesystem.close(self.descriptor);
    let index: &[u8] in Path = builder.output.resolve("index.bin");
    self.descriptor = self.filesystem.create(index, 438);
    self.result = self.filesystem.write(self.descriptor, "ordinary index");
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "multiple-output-review")
            .expect("multiple ordinary outputs with exact sponsored custody should compile");
    let summary = checked
        .build_observation_summary()
        .expect("multiple-output receipt retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert!(summary.included_source_handoffs().is_empty());
    let staged = summary
        .staged_output_tree()
        .expect("multiple outputs retain exact staged-tree custody");
    assert_eq!(staged.entry_count(), 3);
    assert_eq!(
        staged.file_bytes(),
        (b"ordinary artifact".len() + b"ordinary metadata".len() + b"ordinary index".len()) as u64
    );
    let staged_digest = staged.digest();

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("multiple-output receipt should encode")
        .expect("multiple-output receipt should retain replay custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("multiple-output replay record should recover"),
        record
    );
    let package = PackageKeyIdentity::from_digest([99; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "multiple-output", project.clone())
        .with_canonical_source_metadata()
        .expect("capture multiple-output canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package multiple-output input");
    std::fs::write(
        rooted_build_session(&project, "multiple-output-review").join("output/metadata.bin"),
        "host drift",
    )
    .expect("change one physical Output file after receipt capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("multiple ordinary outputs should reopen without consulting host Output");
    set_canonical_source_tree_permissions(&project, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened multiple-output build retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert!(replayed_summary.included_source_handoffs().is_empty());
    assert_eq!(
        replayed_summary
            .staged_output_tree()
            .expect("replay reconstructs the complete ordinary output tree")
            .digest(),
        staged_digest
    );
    assert!(replayed.typed.symbols.source_files().all(|source| {
        !["artifact.bin", "metadata.bin", "index.bin"]
            .iter()
            .any(|artifact| source.path.ends_with(artifact))
    }));

    let mismatch = compile_rooted_probe_with_sponsored_output_seed(
        &project,
        profile,
        "multiple-output-mismatch",
        Some((Path::new("unexpected.bin"), b"unexplained output")),
    )
    .expect_err("an unexplained physical Output entry must reject receipt issuance");
    assert!(mismatch.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("Output tree that differs from sponsored staged-output custody")
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "multiple-output-review"));
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "multiple-output-mismatch"));
}

#[test]
fn source_open_read_at_close_is_replayed_without_a_filesystem_provider() {
    let (project, profile) = rooted_build_probe_project(
        "open-read-at-close-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read_at(self.descriptor, &mut self.buffer, 7, 5);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("open/read_at/close source build should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 6, 8]
    );
    let [_, read, _] = summary.filesystem_operation_attempts() else {
        panic!("open/read_at/close replay fixture has three events")
    };
    assert_eq!(
        read.scalar_operands()
            .iter()
            .map(|operand| (operand.operand_ordinal(), operand.value()))
            .collect::<Vec<_>>(),
        vec![
            (2, BuildFilesystemScalarOperandValue::U64(7)),
            (3, BuildFilesystemScalarOperandValue::I64(5)),
        ]
    );
    assert_eq!(
        read.observed_byte_regions()[0].kind(),
        BuildFilesystemObservedByteRegionKind::PositionedFileRead
    );
    assert_eq!(
        read.observed_bytes(&read.observed_byte_regions()[0]),
        Some(&b"Main { "[..])
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified positioned replay record must encode")
        .expect("verified positioned replay must publish review-only custody bytes");
    recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
        .expect("canonical positioned replay record must recover");

    let mut read_attempt_prefix = Vec::new();
    read_attempt_prefix.extend_from_slice(&6u16.to_le_bytes());
    read_attempt_prefix.extend_from_slice(&[2, 0, 0]);
    read_attempt_prefix.extend_from_slice(&7i64.to_le_bytes());
    read_attempt_prefix.extend_from_slice(&0i32.to_le_bytes());
    read_attempt_prefix.extend_from_slice(&2u64.to_le_bytes());
    let mut sequential_tag_prefix = read_attempt_prefix.clone();
    sequential_tag_prefix[..2].copy_from_slice(&4u16.to_le_bytes());
    let wrong_operation = replace_unique_bytes(
        record.canonical_bytes(),
        &read_attempt_prefix,
        &sequential_tag_prefix,
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(&wrong_operation, limits).is_err(),
        "a positioned scalar shape cannot be relabeled as sequential read"
    );

    let mut positioned_region = Vec::new();
    positioned_region.extend_from_slice(&1u64.to_le_bytes());
    positioned_region.extend_from_slice(&[1, 1]);
    positioned_region.extend_from_slice(&0u64.to_le_bytes());
    positioned_region.extend_from_slice(&7u64.to_le_bytes());
    let mut sequential_region = positioned_region.clone();
    sequential_region[9] = 0;
    let wrong_region = replace_unique_bytes(
        record.canonical_bytes(),
        &positioned_region,
        &sequential_region,
    );
    assert!(
        recover_review_only_build_filesystem_replay_record(&wrong_region, limits).is_err(),
        "a positioned read must retain its exact observed-region kind"
    );

    std::fs::write(project.join("main.omg"), "data Drifted { value: u64; }\n")
        .expect("change host source after positioned replay capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record.clone(),
    )
    .expect("reopened positioned replay should not consult changed host source");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened positioned replay retains observations");
    let [_, replayed_read, _] = replayed_summary.filesystem_operation_attempts() else {
        panic!("reopened positioned replay has three events")
    };
    assert_eq!(
        replayed_read.observed_bytes(&replayed_read.observed_byte_regions()[0]),
        Some(&b"Main { "[..]),
        "positioned replay must receive retained bytes, not changed host bytes"
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read build probe");
    let changed_build = original_build.replace(
        "builder.filesystem.read_at(descriptor, &mut buffer, 7, 5)",
        "builder.filesystem.read_at(descriptor, &mut buffer, 7, 6)",
    );
    assert_ne!(changed_build, original_build, "fixture must change offset");
    std::fs::write(&build_path, changed_build).expect("change positioned replay offset");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record,
    )
    .expect_err("changed authored positioned offset must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("prepared inputs changed")),
        "positioned replay mismatch must identify changed prepared inputs: {diagnostics:?}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn source_open_mixed_read_sequence_close_replays_exact_cursor_semantics() {
    let (project, profile) = rooted_build_probe_project(
        "open-mixed-read-sequence-close-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 5);
    self.result = self.filesystem.read_at(self.descriptor, &mut self.buffer, 4, 10);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 5);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 4096);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 1);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("mixed source-read sequence should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 6, 4, 4, 4, 8]
    );
    let reads = &summary.filesystem_operation_attempts()[1..6];
    assert_eq!(
        reads
            .iter()
            .map(|read| match read.result() {
                BuildFilesystemOperationResult::Scalar(result) => result,
                BuildFilesystemOperationResult::LogicalHandle(_) => {
                    panic!("read returned a logical handle")
                }
            })
            .collect::<Vec<_>>(),
        vec![5, 4, 5, 15, 0]
    );
    assert_eq!(
        reads[0].observed_bytes(&reads[0].observed_byte_regions()[0]),
        Some(&b"data "[..])
    );
    assert_eq!(
        reads[1].observed_bytes(&reads[1].observed_byte_regions()[0]),
        Some(&b"{ va"[..])
    );
    assert_eq!(
        reads[2].observed_bytes(&reads[2].observed_byte_regions()[0]),
        Some(&b"Main "[..]),
        "positioned read must not advance the sequential cursor"
    );
    assert_eq!(reads[4].observed_byte_regions()[0].length(), 0);

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified read sequence must encode")
        .expect("verified read sequence must retain review-only custody");
    recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
        .expect("canonical mixed read sequence must recover");

    std::fs::write(project.join("main.omg"), "data Drifted { value: u64; }\n")
        .expect("change host source after mixed replay capture");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record.clone(),
    )
    .expect("reopened mixed replay must not consult changed host source");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened mixed replay retains observations");
    assert!(
        replayed_summary
            .filesystem_replay_verdict()
            .replays_source_inputs()
    );
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts(),
        "reopened replay must reproduce every ordered read lane exactly"
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read build probe");
    let changed_build = original_build.replace(
        "builder.filesystem.read_at(descriptor, &mut buffer, 4, 10)",
        "builder.filesystem.read_at(descriptor, &mut buffer, 4, 11)",
    );
    assert_ne!(changed_build, original_build, "fixture must change offset");
    std::fs::write(&build_path, changed_build).expect("change middle positioned offset");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record,
    )
    .expect_err("changed middle read input must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("prepared inputs changed")),
        "middle-read mismatch must identify changed prepared inputs: {diagnostics:?}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn multiple_source_read_chains_replay_distinct_files_and_cursors() {
    let (project, profile) = rooted_build_probe_project(
        "multiple-source-read-chains-replay",
        r#"    let main_path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(main_path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 5);
    self.code = self.filesystem.close(self.descriptor);
    let second_path: &[u8] in Path = builder.source.resolve("second.txt");
    self.descriptor = self.filesystem.open(second_path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 3);
    self.result = self.filesystem.read_at(self.descriptor, &mut self.buffer, 4, 7);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 3);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    std::fs::write(project.join("second.txt"), "second payload\n")
        .expect("write second source fixture");
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("multiple source-read chains should compile and replay");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 2, 4, 6, 4, 8]
    );
    let attempts = summary.filesystem_operation_attempts();
    let first_identity = attempts[0]
        .logical_handle_output()
        .expect("first open handle")
        .identity();
    let second_identity = attempts[3]
        .logical_handle_output()
        .expect("second open handle")
        .identity();
    assert_ne!(first_identity, second_identity);
    assert_eq!(
        attempts[1].observed_bytes(&attempts[1].observed_byte_regions()[0]),
        Some(&b"data "[..])
    );
    assert_eq!(
        attempts[4].observed_bytes(&attempts[4].observed_byte_regions()[0]),
        Some(&b"sec"[..])
    );
    assert_eq!(
        attempts[5].observed_bytes(&attempts[5].observed_byte_regions()[0]),
        Some(&b"payl"[..])
    );
    assert_eq!(
        attempts[6].observed_bytes(&attempts[6].observed_byte_regions()[0]),
        Some(&b"ond"[..]),
        "positioned read must not advance the second chain's sequential cursor"
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("verified source-read chains must encode")
        .expect("verified source-read chains must retain review-only custody");
    recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
        .expect("canonical source-read chains must recover");

    std::fs::write(project.join("main.omg"), "data Drifted { value: u64; }\n")
        .expect("change first host source");
    std::fs::write(project.join("second.txt"), "changed second source\n")
        .expect("change second host source");
    let replayed = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record.clone(),
    )
    .expect("reopened source-read chains must not consult changed host files");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened source-read chains retain observations");
    assert!(
        replayed_summary
            .filesystem_replay_verdict()
            .replays_source_inputs()
    );
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read build probe");
    let changed_build = original_build.replace(
        "builder.source.resolve(\"second.txt\")",
        "builder.source.resolve(\"third.txt\")",
    );
    assert_ne!(
        changed_build, original_build,
        "fixture must change second path"
    );
    std::fs::write(&build_path, changed_build).expect("change second chain path");
    let diagnostics = compile_to_checked_with_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        record,
    )
    .expect_err("changed second chain path must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("prepared inputs changed")),
        "second-chain mismatch must identify changed prepared inputs: {diagnostics:?}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn failed_source_read_at_does_not_claim_bounded_replay() {
    let (project, profile) = rooted_build_probe_project(
        "failed-read-at-no-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read_at(self.descriptor, &mut self.buffer, 1, -1);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("failed positioned read remains an ordinary observed build result");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build retains observations");
    assert!(!summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(
        capture_verified_build_filesystem_replay_record(
            summary,
            BuildFilesystemReplayRecordLimits::default(),
        )
        .expect("non-replayed summary is not a codec error")
        .is_none()
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn empty_or_non_read_middle_source_sequences_do_not_claim_bounded_replay() {
    for (label, body) in [
        (
            "open-close-no-replay",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.code = self.filesystem.close(self.descriptor);"#,
        ),
        (
            "open-read-sync-close-no-replay",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 1);
    self.code = self.filesystem.sync(self.descriptor);
    self.code = self.filesystem.close(self.descriptor);"#,
        ),
        (
            "trailing-incomplete-chain-no-replay",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 1);
    self.code = self.filesystem.close(self.descriptor);
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 1);"#,
        ),
        (
            "interleaved-source-chains-no-replay",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.code = self.filesystem.open(path, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 1);
    self.descriptor = self.filesystem.close(self.descriptor);
    self.code = self.filesystem.close(self.code);"#,
        ),
    ] {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("out-of-grammar source sequence remains an ordinary observed build");
        let summary = compilation
            .build_observation_summary()
            .expect("filesystem build retains observations");
        assert!(
            !summary.filesystem_replay_verdict().replays_source_inputs(),
            "{label}"
        );
        assert!(
            capture_verified_build_filesystem_replay_record(
                summary,
                BuildFilesystemReplayRecordLimits::default(),
            )
            .expect("non-replayed summary is not a codec error")
            .is_none(),
            "{label}"
        );
        let _ = std::fs::remove_dir_all(&project);
    }
}

#[test]
fn write_like_source_open_does_not_claim_bounded_replay() {
    let (project, profile) = rooted_build_probe_project(
        "write-like-open-no-replay",
        r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 1);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 1);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let compilation = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("denied write-like open remains an ordinary observed build result");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build retains observations");
    assert!(!summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(
        capture_verified_build_filesystem_replay_record(
            summary,
            BuildFilesystemReplayRecordLimits::default(),
        )
        .expect("non-replayed summary is not a codec error")
        .is_none()
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn rooted_path_prefix_precedes_a_later_resolver_failure() {
    let (project, profile) = rooted_build_probe_project(
        "rooted-path-before-resolver-failure",
        r#"    self.code = self.filesystem.rename(
        builder.output.resolve("stage/old"),
        builder.output.resolve("../invalid")
    );"#,
    );
    let diagnostics = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("noncanonical second rooted operand must reject build evaluation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("canonical relative components"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1 rooted-path operand(s)"),
        "only the first successfully resolved rename operand must survive: {rendered}"
    );
    assert!(
        rendered.contains("0 grant refusal(s)"),
        "preparation failure must not masquerade as grant authorization: {rendered}"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn rooted_build_rejects_unrooted_find_continuations_before_operand_preparation() {
    for (case, body) in [
        (
            "find-next",
            r#"    self.code = self.filesystem.find_next(7, &mut self.small_buffer);"#,
        ),
        (
            "find-close",
            r#"    self.code = self.filesystem.find_close(7);"#,
        ),
    ] {
        let (project, profile) = rooted_build_probe_project(case, body);
        let diagnostics =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect_err("unrooted find continuation must reject package build evaluation");
        let rendered = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("unrooted find-cursor protocol not admitted by the Build facet"),
            "{case}: {rendered}"
        );
        assert!(
            !rendered.contains("filesystem output requires 320 bytes")
                && !rendered.contains("1 logical-handle operand(s)")
                && !rendered.contains("1 mutable-carrier operand(s)"),
            "{case}: package rejection must precede handle and buffer preparation: {rendered}"
        );
        let _ = std::fs::remove_dir_all(&project);
    }
}

#[test]
fn canonical_build_roots_reject_unrooted_and_noncanonical_paths() {
    let cases = [
        (
            "unrooted",
            r#"    self.descriptor = self.filesystem.open("input.txt", 0);"#,
            "must come from BuildSource::resolve or BuildOutput::resolve",
        ),
        (
            "fake-resolver",
            r#"    let path: &[u8] in Path = self.fake.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);"#,
            "must come from BuildSource::resolve or BuildOutput::resolve",
        ),
        (
            "parent",
            r#"    let path: &[u8] in Path = builder.source.resolve("../input.txt");"#,
            "canonical relative components",
        ),
        (
            "absolute",
            r#"    let path: &[u8] in Path = builder.source.resolve("/input.txt");"#,
            "absolute or host-specific spelling",
        ),
        (
            "backslash",
            r#"    let path: &[u8] in Path = builder.source.resolve("dir\\input.txt");"#,
            "absolute or host-specific spelling",
        ),
        (
            "empty-component",
            r#"    let path: &[u8] in Path = builder.source.resolve("dir//input.txt");"#,
            "canonical relative components",
        ),
        (
            "drive-prefix",
            r#"    let path: &[u8] in Path = builder.output.resolve("C:input.txt");"#,
            "absolute or host-specific spelling",
        ),
        (
            "source-handoff",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    builder.output.include_source(path);"#,
            "belongs to a different build root",
        ),
        (
            "fake-handoff",
            r#"    let path: &[u8] in Path = self.fake.resolve("generated.omg");
    builder.output.include_source(path);"#,
            "requires an Output-rooted path",
        ),
        (
            "duplicate-handoff",
            r#"    let path: &[u8] in Path = builder.output.resolve("generated.omg");
    self.descriptor = self.filesystem.create(path, 438);
    self.descriptor = self.filesystem.close(self.descriptor);
    builder.output.include_source(path);
    builder.output.include_source(path);"#,
            "names the same path more than once",
        ),
    ];

    for (label, body, expected) in cases {
        let (project, profile) = rooted_build_probe_project(label, body);
        let diagnostics =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect_err("invalid rooted build path must reject before host access");
        let rendered = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains(expected), "{label}: {rendered}");
        let _ = std::fs::remove_dir_all(&project);
    }
}

#[test]
fn canonical_source_root_cannot_be_used_for_writes() {
    let (project, profile) = rooted_build_probe_project(
        "source-write",
        r#"    let path: &[u8] in Path = builder.source.resolve("blocked.bin");
    self.descriptor = self.filesystem.create(path, 438);"#,
    );
    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("a denied source-root write is an observed build result");
    let observations = checked
        .build_observation_summary()
        .cloned()
        .expect("source-root denial remains observable");
    let [create] = observations.filesystem_operation_attempts() else {
        panic!("source-root write must retain one attempted create")
    };
    assert_eq!(observations.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        observations.filesystem_replay_verdict().disposition(),
        BuildFilesystemReplayDisposition::Complete
    );
    assert_eq!(
        create.observation_class(),
        omega_build_evaluation::BuildFilesystemOperationObservationClass::Receipted
    );
    assert_eq!(create.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(create.post_error(), 13);
    assert_eq!(
        create.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(438)
    );
    let [rooted] = create.rooted_path_operand_resolutions() else {
        panic!("source-root write must retain one rooted input coordinate")
    };
    assert_eq!(rooted.operand_ordinal(), 0);
    assert_eq!(rooted.root(), BuildFilesystemRoot::Source);
    assert_eq!(rooted.relative_path(), b"blocked.bin");
    assert!(create.authorized_paths().is_empty());
    let [refusal] = create.grant_refusals() else {
        panic!("source-root write must retain its refused path")
    };
    assert_eq!(refusal.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(
        refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );
    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(&observations, limits)
        .expect("refused Source write replay record must encode")
        .expect("complete refused Source write replay must retain restart custody");
    recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
        .expect("refused Source write replay record must recover canonically");
    assert!(!project.join("blocked.bin").exists());
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn canonical_source_root_cannot_remove_existing_file() {
    let (project, profile) = rooted_build_probe_project(
        "source-remove",
        r#"    let path: &[u8] in Path = builder.source.resolve("blocked.bin");
    self.code = self.filesystem.remove(path);"#,
    );
    std::fs::write(project.join("blocked.bin"), b"retained\n").expect("seed canonical Source file");

    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("a denied source-root remove is an observed build result");
    let observations = checked
        .build_observation_summary()
        .cloned()
        .expect("source-root remove denial remains observable");
    let [remove] = observations.filesystem_operation_attempts() else {
        panic!("source-root remove must retain one attempted operation")
    };
    assert_eq!(observations.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        observations.filesystem_replay_verdict().disposition(),
        BuildFilesystemReplayDisposition::Complete
    );
    assert_eq!(remove.operation_tag(), 9);
    assert_eq!(
        remove.observation_class(),
        omega_build_evaluation::BuildFilesystemOperationObservationClass::Receipted
    );
    assert_eq!(remove.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(remove.post_error(), 13);
    assert!(remove.scalar_operands().is_empty());
    let [rooted] = remove.rooted_path_operand_resolutions() else {
        panic!("source-root remove must retain one rooted input coordinate")
    };
    assert_eq!(rooted.operand_ordinal(), 0);
    assert_eq!(rooted.root(), BuildFilesystemRoot::Source);
    assert_eq!(rooted.relative_path(), b"blocked.bin");
    assert!(remove.authorized_paths().is_empty());
    let [refusal] = remove.grant_refusals() else {
        panic!("source-root remove must retain its refused path")
    };
    assert_eq!(refusal.operand_ordinal(), 0);
    assert_eq!(refusal.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(
        refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(&observations, limits)
        .expect("refused Source remove replay record must encode")
        .expect("complete refused Source remove replay retains restart custody");
    recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
        .expect("refused Source remove replay record must recover canonically");
    // This guards evaluator ordering only; it is not evidence that Omega
    // contains arbitrary host processes or globally protects this path.
    assert_eq!(
        std::fs::read(project.join("blocked.bin")).expect("read retained Source file"),
        b"retained\n"
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[cfg(any(unix, windows))]
#[test]
fn source_root_follow_rejects_a_symlink_escape_before_the_requested_operation() {
    let (project, profile) = rooted_build_probe_project(
        "source-symlink-escape",
        r#"    let path: &[u8] in Path = builder.source.resolve("escape/input.bin");
    self.descriptor = self.filesystem.open(path, 0);"#,
    );
    let outside = project.with_file_name(format!(
        "omega-rooted-build-probe-source-symlink-escape-outside-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir(&outside).expect("create outside source directory");
    std::fs::write(outside.join("input.bin"), b"outside source\n").expect("seed outside source");
    create_directory_symlink(&outside, &project.join("escape"))
        .expect("create escaping source directory symlink; Windows requires symlink privilege");

    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("a denied source symlink escape remains an observed build result");
    let observations = checked
        .build_observation_summary()
        .expect("source symlink denial remains observable");
    let [open] = observations.filesystem_operation_attempts() else {
        panic!("source symlink escape must retain one attempted open")
    };
    assert_eq!(open.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(open.post_error(), 13);
    assert!(open.authorized_paths().is_empty());
    let [rooted] = open.rooted_path_operand_resolutions() else {
        panic!("source symlink escape must retain its rooted operand")
    };
    assert_eq!(rooted.root(), BuildFilesystemRoot::Source);
    assert_eq!(rooted.relative_path(), b"escape/input.bin");
    let [refusal] = open.grant_refusals() else {
        panic!("source symlink escape must retain its refused path")
    };
    assert_eq!(refusal.operand_ordinal(), 0);
    assert_eq!(refusal.access(), BuildFilesystemGrantAccess::Read);
    assert_eq!(
        refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );
    assert_eq!(
        std::fs::read(outside.join("input.bin")).expect("read outside source after denial"),
        b"outside source\n"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(any(unix, windows))]
#[test]
fn output_root_follow_rejects_a_symlink_escape_before_the_requested_operation() {
    let (project, profile) = rooted_build_probe_project(
        "output-symlink-escape",
        r#"    let path: &[u8] in Path = builder.output.resolve("escape/new.bin");
    self.descriptor = self.filesystem.create(path, 438);"#,
    );
    let build_dir = project.join("build");
    std::fs::create_dir(&build_dir).expect("create output root");
    let outside = project.with_file_name(format!(
        "omega-rooted-build-probe-output-symlink-escape-outside-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&outside);
    std::fs::create_dir(&outside).expect("create outside output directory");
    create_directory_symlink(&outside, &build_dir.join("escape"))
        .expect("create escaping output directory symlink; Windows requires symlink privilege");

    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("a denied output symlink escape remains an observed build result");
    let observations = checked
        .build_observation_summary()
        .cloned()
        .expect("output symlink denial remains observable");
    let [create] = observations.filesystem_operation_attempts() else {
        panic!("output symlink escape must retain one attempted create")
    };
    assert_eq!(create.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(create.post_error(), 13);
    assert!(create.authorized_paths().is_empty());
    let [rooted] = create.rooted_path_operand_resolutions() else {
        panic!("output symlink escape must retain its rooted operand")
    };
    assert_eq!(rooted.root(), BuildFilesystemRoot::Output);
    assert_eq!(rooted.relative_path(), b"escape/new.bin");
    let [refusal] = create.grant_refusals() else {
        panic!("output symlink escape must retain its refused path")
    };
    assert_eq!(refusal.operand_ordinal(), 0);
    assert_eq!(refusal.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(
        refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );
    assert!(
        !outside.join("new.bin").exists(),
        "the denied create must not create a file through the escaping parent"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&outside);
}

#[test]
fn path_like_filesystem_operands_survive_compiler_projection() {
    let (project, profile) = rooted_build_probe_project(
        "path-like-symlink-target",
        r#"    let link: &[u8] in Path = builder.output.resolve("missing-parent/link");
    self.code = self.filesystem.symlink("missing-target", link);"#,
    );
    let checked = compile_rooted_probe_with_sponsored_output(&project, profile, "review")
        .expect("a refused symlink remains a successful observed build");
    let observations = checked
        .build_observation_summary()
        .expect("filesystem build publishes observation evidence");
    let [symlink] = observations.filesystem_operation_attempts() else {
        panic!("fixture performs one symlink operation")
    };
    let [target] = symlink.path_like_operands() else {
        panic!("symlink retains its exact target spelling in the path-like lane")
    };
    assert_eq!(target.operand_ordinal(), 0);
    assert_eq!(target.bytes(), b"missing-target");
    let [link] = symlink.rooted_path_operand_resolutions() else {
        panic!("symlink retains its compiler-rooted link operand before grant refusal")
    };
    assert_eq!(link.operand_ordinal(), 1);
    assert_eq!(link.root(), BuildFilesystemRoot::Output);
    assert_eq!(link.relative_path(), b"missing-parent/link");
    assert!(symlink.byte_operands().is_empty());
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "review"));
}

#[cfg(unix)]
#[test]
fn source_read_link_complete_and_truncated_results_restart_replay() {
    use std::os::unix::fs::symlink;

    let (project, profile) = rooted_build_probe_project(
        "returned-read-link",
        r#"    let link: &[u8] in Path = builder.source.resolve("fixture-link");
    self.result = self.filesystem.read_link(link, &mut self.buffer, 4096);
    self.result = self.filesystem.read_link(link, &mut self.small_buffer, 1);"#,
    );
    symlink("returned-target", project.join("fixture-link"))
        .expect("create source symlink fixture");
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "read-link-receipt")
            .expect("read_link of a granted source symlink receipts");
    let summary = checked
        .build_observation_summary()
        .expect("filesystem build publishes observation evidence");
    assert_eq!(summary.schema_version(), BUILD_OBSERVATION_SCHEMA_VERSION);
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    let [complete, truncated] = summary.filesystem_operation_attempts() else {
        panic!("fixture performs two read_link operations")
    };
    let [returned] = complete.returned_paths() else {
        panic!("read_link retains one exact meaningful returned path payload")
    };
    assert_eq!(returned.operand_ordinal(), 1);
    assert_eq!(
        returned.kind(),
        BuildFilesystemReturnedPathKind::ReadLinkPayload
    );
    assert_eq!(
        returned.completeness(),
        BuildFilesystemReturnedPathCompleteness::Complete
    );
    assert_eq!(returned.bytes(), b"returned-target");
    assert_eq!(
        complete.result(),
        BuildFilesystemOperationResult::Scalar(15)
    );
    let [returned_prefix] = truncated.returned_paths() else {
        panic!("truncated read_link retains one exact meaningful prefix")
    };
    assert_eq!(
        returned_prefix.completeness(),
        BuildFilesystemReturnedPathCompleteness::LimitReached
    );
    assert_eq!(returned_prefix.bytes(), b"r");
    assert_eq!(
        truncated.result(),
        BuildFilesystemOperationResult::Scalar(1)
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("source read_link receipt encodes")
        .expect("source read_link receipt retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("source read_link receipt recovers");
    assert_eq!(recovered, record);

    let package = PackageKeyIdentity::from_digest([105; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "source-read-link", project.clone())
        .with_canonical_source_metadata()
        .expect("capture source read_link canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package source read_link input");
    set_canonical_source_tree_permissions(&project, false);

    std::fs::remove_file(project.join("fixture-link")).expect("remove original source symlink");
    symlink("host-drift-target", project.join("fixture-link"))
        .expect("replace source symlink after receipt capture");
    set_canonical_source_tree_permissions(&project, true);
    let drift = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs.clone(),
        recovered.clone(),
    )
    .expect_err("changed Source symlink must reject before replay");
    assert!(
        drift
            .iter()
            .any(|diagnostic| diagnostic.message.contains("canonical Source metadata"))
    );
    set_canonical_source_tree_permissions(&project, false);
    std::fs::remove_file(project.join("fixture-link")).expect("remove drifted source symlink");
    symlink("returned-target", project.join("fixture-link"))
        .expect("restore receipted source symlink");
    set_canonical_source_tree_permissions(&project, true);
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        recovered,
    )
    .expect("source read_link receipt must restart from matching canonical custody");
    set_canonical_source_tree_permissions(&project, false);
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("replayed source read_link retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "read-link-receipt"));
}

#[test]
fn zero_length_positioned_read_region_survives_compiler_projection() {
    let (project, profile) = rooted_build_probe_project(
        "empty-positioned-read",
        r#"    let input: &[u8] in Path = builder.source.resolve("empty.txt");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read_at(self.descriptor, &mut self.buffer, 4096, 7);"#,
    );
    std::fs::write(project.join("empty.txt"), b"").expect("create empty source fixture");
    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("positioned EOF read of a granted source file succeeds");
    let observations = checked
        .build_observation_summary()
        .expect("filesystem build publishes observation evidence");
    let [_, read_at] = observations.filesystem_operation_attempts() else {
        panic!("fixture opens and reads one source file")
    };
    let [region] = read_at.observed_byte_regions() else {
        panic!("successful positioned EOF retains one empty observed region")
    };
    assert_eq!(region.output_operand_ordinal(), 1);
    assert_eq!(
        region.kind(),
        BuildFilesystemObservedByteRegionKind::PositionedFileRead
    );
    assert_eq!(region.offset(), 0);
    assert_eq!(region.length(), 0);
    assert_eq!(read_at.observed_bytes(region), Some(b"".as_slice()));
    assert_eq!(read_at.result(), BuildFilesystemOperationResult::Scalar(0));
    let _ = std::fs::remove_dir_all(&project);
}

#[cfg(unix)]
#[test]
fn directory_record_region_survives_compiler_projection() {
    let (project, profile) = rooted_build_probe_project(
        "directory-record-region",
        r#"    let directory: &[u8] in Path = builder.source.resolve("entries");
    self.descriptor = self.filesystem.open(directory, 0);
    self.result = self.filesystem.read_dir(
        self.descriptor,
        &mut self.buffer,
        4096,
        &mut self.position
    );"#,
    );
    std::fs::create_dir(project.join("entries")).expect("create source directory fixture");
    std::fs::write(project.join("entries/item.txt"), b"item")
        .expect("create directory entry fixture");
    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("enumerating a granted source directory succeeds");
    let observations = checked
        .build_observation_summary()
        .expect("filesystem build publishes observation evidence");
    let [_, read_dir] = observations.filesystem_operation_attempts() else {
        panic!("fixture opens and enumerates one source directory")
    };
    let [region] = read_dir.observed_byte_regions() else {
        panic!("successful directory read retains one record region")
    };
    assert_eq!(
        region.kind(),
        BuildFilesystemObservedByteRegionKind::DirectoryRecords
    );
    assert_eq!(region.output_operand_ordinal(), 1);
    assert_eq!(region.offset(), 0);
    let BuildFilesystemOperationResult::Scalar(result) = read_dir.result() else {
        panic!("read_dir returns a scalar byte count")
    };
    assert!(result > 0);
    assert_eq!(region.length(), result as u64);
    assert_eq!(
        read_dir
            .observed_bytes(region)
            .expect("directory record bytes remain custodied")
            .len(),
        region.length() as usize
    );
    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn canonical_build_roots_reject_host_absolute_path_results() {
    let cases = [
        (
            "canonicalize",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.result = self.filesystem.canonicalize(path, &mut self.buffer);"#,
            "canonicalize",
        ),
        (
            "final-path",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.get_osfhandle(self.descriptor);
    self.result = self.filesystem.final_path_name_by_handle(self.result, &mut self.buffer, 4096, 0);"#,
            "final_path_name_by_handle",
        ),
    ];
    for (label, body, operation) in cases {
        let (project, profile) = rooted_build_probe_project(label, body);
        let diagnostics =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect_err("host-absolute path result must reject in rooted build mode");
        let rendered = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains(operation), "{label}: {rendered}");
        assert!(
            rendered.contains("would expose a host-absolute path"),
            "{label}: {rendered}"
        );
        let _ = std::fs::remove_dir_all(&project);
    }
}

#[test]
fn generated_plain_data_handoff_reuses_the_retained_typed_base() {
    let (project, profile) = rooted_build_probe_project(
        "included-source",
        r#"    let generated: &[u8] in Path = builder.output.resolve("generated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Generated { base: Main; }\n");
    self.descriptor = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );

    let unsponsored = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("generated source cannot leave an uncaptured output root");
    let unsponsored = unsponsored
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        unsponsored.contains("without sponsored staged-output custody"),
        "{unsponsored}"
    );

    let checked = compile_rooted_probe_with_sponsored_output(&project, profile, "sponsored-review")
        .expect("captured generated source must enter final checked compilation");
    let generated = checked
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with(".omega/generated/generated.omg"))
        .expect("final checked program retains generated-source custody");
    assert_eq!(
        generated.source.as_ref(),
        "data Generated { base: Main; }\n"
    );
    let main = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("authored Main data");
    let generated_data = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Generated")
        .expect("seeded generated data reaches final typing");
    let [psi_typed_trees::data::DataMember::Field(base_field)] =
        checked.typed.data_members(generated_data)
    else {
        panic!("Generated has one base field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .typed
        .type_reference_table
        .type_reference(base_field.type_reference)
    else {
        panic!("Generated.base remains nominal")
    };
    assert_eq!(*symbol, main.symbol);
    let selected_build = checked
        .selected_build_machine_symbol()
        .expect("generated-source build retains its exact selected symbol");
    assert!(checked.typed.machines().iter().any(|machine| {
        machine.symbol == selected_build
            && checked
                .typed
                .symbols
                .source_file(
                    checked
                        .typed
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .expect("selected build source span"),
                )
                .is_some_and(|source| source.path.ends_with("build.omg"))
    }));
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("filesystem build remains observed")
            .filesystem_operation_attempts()
            .len(),
        3,
        "the final compilation pass must not execute the build machine again"
    );
    checked
        .verify_current_source_consumption()
        .expect("generated bytes remain tied to retained staged-output custody");
    assert_eq!(
        std::fs::read_to_string(
            rooted_build_session(&project, "sponsored-review").join("output/generated.omg"),
        )
        .unwrap(),
        "data Generated { base: Main; }\n"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "sponsored-review"));
}

#[test]
fn generated_erased_lifetime_data_handoff_retains_configuration_and_evidence() {
    const GENERATED: &str =
        "data View<'buf> { body: &'buf Main; }\ndata Envelope<'msg> { view: View<'msg>; }\n";
    let (project, profile) = rooted_build_probe_project(
        "generated-erased-lifetime-data",
        r#"    builder.subsystem = Subsystem::Gui;
    let generated: &[u8] in Path = builder.output.resolve("generated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data View<'buf> { body: &'buf Main; }\ndata Envelope<'msg> { view: View<'msg>; }\n");
    self.descriptor = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );

    let checked = compile_rooted_probe_with_sponsored_output(
        &project,
        profile,
        "generated-erased-lifetime-data-review",
    )
    .expect("erased lifetime-only generated data must continue from the retained frontend");
    assert_eq!(
        checked.subsystem(),
        2,
        "the retained build configuration wins"
    );
    let generated = checked
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with(".omega/generated/generated.omg"))
        .expect("final checked program retains generated lifetime-source custody");
    assert_eq!(generated.source.as_ref(), GENERATED);

    let main = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Main")
        .expect("authored Main data");
    let view = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "View")
        .expect("generated View data");
    let envelope = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Envelope")
        .expect("generated Envelope data");
    assert_eq!(
        view.lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["buf"]
    );
    assert_eq!(
        envelope
            .lifetime_parameters
            .iter()
            .map(|parameter| parameter.as_str())
            .collect::<Vec<_>>(),
        ["msg"]
    );
    let [psi_typed_trees::data::DataMember::Field(body)] = checked.typed.data_members(view) else {
        panic!("View has one body field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Reference {
        referee, lifetime, ..
    } = checked
        .typed
        .type_reference_table
        .type_reference(body.type_reference)
    else {
        panic!("View.body remains a named-lifetime reference")
    };
    assert_eq!(lifetime.as_ref().map(|name| name.as_str()), Some("buf"));
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.typed.type_reference_table.type_reference(*referee)
    else {
        panic!("View.body referee remains nominal")
    };
    assert_eq!(*symbol, main.symbol);
    let [psi_typed_trees::data::DataMember::Field(view_field)] =
        checked.typed.data_members(envelope)
    else {
        panic!("Envelope has one view field")
    };
    let psi_typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = checked
        .typed
        .type_reference_table
        .type_reference(view_field.type_reference)
    else {
        panic!("Envelope.view remains an erased lifetime application")
    };
    assert_eq!(*base_symbol, view.symbol);
    assert_eq!(
        lifetime_arguments
            .iter()
            .map(|argument| argument.as_str())
            .collect::<Vec<_>>(),
        ["msg"]
    );
    assert!(
        checked
            .typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
    );

    let selected_build = checked
        .selected_build_machine_symbol()
        .expect("lifetime continuation retains its exact selected build symbol");
    assert!(
        checked
            .typed
            .machines()
            .iter()
            .any(|machine| machine.symbol == selected_build)
    );
    let observation_count = checked
        .build_observation_summary()
        .expect("filesystem build retains observation evidence")
        .filesystem_operation_attempts()
        .len();
    assert_eq!(observation_count, 3);
    assert_eq!(
        checked
            .build_evaluation_usage()
            .expect("filesystem build retains evaluation evidence")
            .filesystem_operation_attempts,
        u64::try_from(observation_count).expect("observation count")
    );
    checked
        .verify_current_source_consumption()
        .expect("lifetime source bytes remain tied to retained output custody");

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(
        &project,
        "generated-erased-lifetime-data-review",
    ));
}

#[test]
fn unsupported_generic_generated_data_uses_the_whole_program_rebuild_fallback() {
    let (project, profile) = rooted_build_probe_project(
        "generated-generic-data-fallback",
        r#"    let generated: &[u8] in Path = builder.output.resolve("generated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Generated<T> { value: T; }\n");
    self.descriptor = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );

    let checked = compile_rooted_probe_with_sponsored_output(
        &project,
        profile,
        "generated-generic-data-fallback-review",
    )
    .expect("unsupported seeded shape must retain the whole-program rebuild path");
    assert!(checked.typed.data_definitions().iter().any(|definition| {
        definition.name.as_str() == "Generated"
            && checked.typed.data_type_parameters(definition).len() == 1
    }));
    let selected_build = checked
        .selected_build_machine_symbol()
        .expect("fallback compilation rebinds the selected build machine");
    assert!(
        checked
            .typed
            .machines()
            .iter()
            .any(|machine| machine.symbol == selected_build)
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(
        &project,
        "generated-generic-data-fallback-review",
    ));
}

#[test]
fn current_activation_generated_sources_form_one_extension_resolution_stratum() {
    const ALPHA: &str = r#"machine choose() -> i32 { 2 }
machine alpha_leaf() -> i32 { authored_helper() }
machine alpha_to_beta() -> i32 { beta_leaf() }
trait Shape {
    machine Self::code(&self) -> i32;
}
data WireShared { #1 value: u8; }
Primary: Item satisfies Shape {
    machine code(&self) -> i32 { code }
}
Secondary: Item satisfies Shape {
    machine code(&self) -> i32 { code }
}
"#;
    const BETA: &str = r#"machine beta_leaf() -> i32 { authored_helper() }
machine beta_to_alpha() -> i32 { alpha_leaf() }
"#;
    let (project, profile) = rooted_build_probe_project(
        "generated-resolution-stratum",
        r#"    let alpha: &[u8] in Path = builder.output.resolve("alpha.omg");
    self.descriptor = self.filesystem.create(alpha, 438);
    self.result = self.filesystem.write(self.descriptor, "machine choose() -> i32 { 2 }\nmachine alpha_leaf() -> i32 { authored_helper() }\nmachine alpha_to_beta() -> i32 { beta_leaf() }\ntrait Shape {\n    machine Self::code(&self) -> i32;\n}\ndata WireShared { #1 value: u8; }\nPrimary: Item satisfies Shape {\n    machine code(&self) -> i32 { code }\n}\nSecondary: Item satisfies Shape {\n    machine code(&self) -> i32 { code }\n}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(alpha);
    let beta: &[u8] in Path = builder.output.resolve("beta.omg");
    self.descriptor = self.filesystem.create(beta, 438);
    self.result = self.filesystem.write(self.descriptor, "machine beta_leaf() -> i32 { authored_helper() }\nmachine beta_to_alpha() -> i32 { alpha_leaf() }\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(beta);"#,
    );
    std::fs::write(
        project.join("main.omg"),
        r#"trait Shape {
    machine Self::code(&self) -> i32;
}
data Item { code: i32; }
data WireShared { #1 value: u8; }
Primary: Item satisfies Shape {
    machine code(&self) -> i32 { self.code }
}
machine choose() -> i32 in Saturating { 1 as i32 in Saturating }
machine authored_helper() -> i32 { 0 }
machine consume(value: &dyn Shape) -> i32 { value.code() }
machine authored_overload_call() -> i32 {
    let selected: i32 = choose();
    selected
}
machine authored_bare_conformance(item: &Item) -> i32 { consume(item) }
"#,
    )
    .expect("write authored generated-resolution fixture");

    let checked = compile_rooted_probe_with_sponsored_output(
        &project,
        profile,
        "generated-resolution-stratum-review",
    )
    .unwrap_or_else(|diagnostics| {
        panic!(
            "current-activation extension sources should compile as one resolution stratum: {diagnostics:#?}"
        )
    });

    let source_files = checked.typed.symbols.source_files().collect::<Vec<_>>();
    assert_eq!(source_files.len(), checked.source_file_count());
    let generated = source_files
        .iter()
        .filter(|source| {
            source.resolution_stratum
                == psi_source::SourceResolutionStratum::CurrentActivationExtension
        })
        .collect::<Vec<_>>();
    assert_eq!(generated.len(), 2);
    assert!(
        generated.iter().any(|source| {
            source.path.ends_with("alpha.omg") && source.source.as_ref() == ALPHA
        })
    );
    assert!(
        generated
            .iter()
            .any(|source| { source.path.ends_with("beta.omg") && source.source.as_ref() == BETA })
    );
    assert!(source_files.iter().all(|source| {
        source.path.ends_with("alpha.omg")
            || source.path.ends_with("beta.omg")
            || source.resolution_stratum == psi_source::SourceResolutionStratum::Base
    }));

    let bundle = checked
        .package_generated_source_bundle()
        .expect("checked package retains exact generated-source bundle");
    assert_eq!(bundle.sources().len(), 2);
    assert_eq!(bundle.sources()[0].relative_path(), b"alpha.omg");
    assert_eq!(bundle.sources()[0].bytes(), ALPHA.as_bytes());
    assert_eq!(bundle.sources()[1].relative_path(), b"beta.omg");
    assert_eq!(bundle.sources()[1].bytes(), BETA.as_bytes());
    assert_ne!(
        checked
            .base_source_consumption_commitment()
            .expect("package build retains the admitted base commitment"),
        checked
            .source_consumption_commitment()
            .expect("final package subject retains its post-generation commitment"),
        "own current-activation generated source must extend, not rewrite, the admitted base"
    );
    assert_eq!(
        checked
            .build_observation_summary()
            .expect("generated-source build retains exact handoffs")
            .included_source_handoffs()
            .len(),
        2
    );

    let call_target_strata = checked
        .typed
        .expression_table
        .expression_entries()
        .filter_map(|(_, expression)| {
            let psi_typed_trees::expression::ExpressionNode::Call(call) = expression else {
                return None;
            };
            let declaration = checked
                .typed
                .symbols
                .symbol_provenance_source_span(call.target_symbol)
                .and_then(|span| checked.typed.symbols.source_file(span))?;
            Some((call.target.as_str(), declaration.resolution_stratum))
        })
        .collect::<Vec<_>>();
    let base = psi_source::SourceResolutionStratum::Base;
    let extension = psi_source::SourceResolutionStratum::CurrentActivationExtension;
    assert!(
        call_target_strata.contains(&("choose", base)),
        "retained call targets: {call_target_strata:?}"
    );
    assert!(call_target_strata.contains(&("authored_helper", base)));
    assert!(call_target_strata.contains(&("beta_leaf", extension)));
    assert!(call_target_strata.contains(&("alpha_leaf", extension)));

    let selected_shape_strata = checked
        .typed
        .conformances()
        .iter()
        .filter(|conformance| {
            matches!(
                checked.typed.symbols.name(conformance.symbol),
                "Primary" | "Secondary"
            )
        })
        .map(|conformance| {
            let declaration = checked
                .typed
                .symbols
                .symbol_provenance_source_span(conformance.symbol)
                .and_then(|span| checked.typed.symbols.source_file(span))
                .expect("source-backed conformance");
            let selected_trait = checked
                .typed
                .symbols
                .symbol_provenance_source_span(conformance.trait_symbol)
                .and_then(|span| checked.typed.symbols.source_file(span))
                .expect("source-backed selected Shape trait");
            (
                checked.typed.symbols.name(conformance.symbol),
                declaration.resolution_stratum,
                selected_trait.resolution_stratum,
            )
        })
        .collect::<Vec<_>>();
    assert!(selected_shape_strata.contains(&("Primary", base, base)));
    assert!(selected_shape_strata.contains(&("Primary", extension, extension)));
    assert!(selected_shape_strata.contains(&("Secondary", extension, extension)));

    checked
        .verify_current_source_consumption()
        .expect("authored and generated source custody remains exact");
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(
        &project,
        "generated-resolution-stratum-review",
    ));
}

#[test]
fn distinct_current_activation_extension_units_remain_one_duplicate_scope() {
    let (project, profile) = rooted_build_probe_project(
        "generated-extension-duplicate-scope",
        r#"    let alpha: &[u8] in Path = builder.output.resolve("alpha.omg");
    self.descriptor = self.filesystem.create(alpha, 438);
    self.result = self.filesystem.write(self.descriptor, "trait ExtensionCollision {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(alpha);
    let beta: &[u8] in Path = builder.output.resolve("beta.omg");
    self.descriptor = self.filesystem.create(beta, 438);
    self.result = self.filesystem.write(self.descriptor, "trait ExtensionCollision {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(beta);"#,
    );

    let diagnostics = compile_rooted_probe_with_sponsored_output(
        &project,
        profile,
        "generated-extension-duplicate-scope-review",
    )
    .expect_err("separate extension units must not become separate duplicate scopes");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("duplicate trait `ExtensionCollision`")),
        "unexpected generated-extension duplicate diagnostics: {diagnostics:#?}"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(
        &project,
        "generated-extension-duplicate-scope-review",
    ));
}

#[test]
fn receipted_generated_source_reopens_with_source_custody_and_rejects_source_drift() {
    let (project, profile) = rooted_build_probe_project(
        "receipted-generated-source",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.code = self.filesystem.read_file_metadata(self.descriptor, &mut self.buffer);
    self.code = self.filesystem.close(self.descriptor);
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("generated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Generated { value: u8; }\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let checked = compile_rooted_probe_with_sponsored_output(&project, profile, "receipted-review")
        .expect("bounded source/output build should compile");
    let summary = checked
        .build_observation_summary()
        .expect("bounded source/output build retains observations");
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.ceiling(), BuildObservationClass::Volatile);
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 39, 8, 2, 4, 8, 1, 5, 8]
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("receipted source/output record should encode")
        .expect("receipted source/output record should retain custody");
    let changed_record_bytes = replace_unique_bytes(
        record.canonical_bytes(),
        b"data Generated { value: u8; }\n",
        b"data Generated { value: u9; }\n",
    );
    let changed_record =
        recover_review_only_build_filesystem_replay_record(&changed_record_bytes, limits)
            .expect("changed payload remains canonically framed but has different semantics");
    let package = PackageKeyIdentity::from_digest([97; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "generated-source", project.clone())
        .with_canonical_source_metadata()
        .expect("capture replayed generated-source canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package generated-source input");
    let changed_diagnostics = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs.clone(),
        changed_record,
    )
    .expect_err("changed retained output bytes must disagree with authored replay");
    assert!(changed_diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("replay") && diagnostic.message.contains("changed")
    }));

    std::fs::write(
        rooted_build_session(&project, "receipted-review").join("output/generated.omg"),
        "data Spoofed {}\n",
    )
    .expect("change host output after receipt capture");

    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs.clone(),
        record.clone(),
    )
    .expect("reopened receipt should reproduce build input and ignore stale host output");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened receipt retains observations");
    assert!(replayed_summary.filesystem_replay_verdict().is_complete());
    assert_eq!(
        replayed_summary.realized(),
        BuildObservationClass::Receipted
    );
    let generated = replayed
        .typed
        .symbols
        .source_files()
        .find(|source| source.path.ends_with(".omega/generated/generated.omg"))
        .expect("replayed generated source enters the final checked program");
    assert_eq!(generated.source.as_ref(), "data Generated { value: u8; }\n");

    set_canonical_source_tree_permissions(&project, false);
    std::fs::write(project.join("main.omg"), "data Main { value: u16; }\n")
        .expect("change host source after receipt capture");
    let source_drift = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect_err("package-aware replay must reject changed canonical Source custody");
    assert!(source_drift.iter().any(|diagnostic| {
        diagnostic.message.contains("canonical Source metadata")
            || diagnostic.message.contains("source consumption")
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "receipted-review"));
}

#[test]
fn multiple_generated_source_handoffs_retain_exact_order_and_ordinals() {
    let (project, profile) = rooted_build_probe_project(
        "multiple-generated-source-handoffs",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let alpha: &[u8] in Path = builder.output.resolve("alpha.omg");
    self.descriptor = self.filesystem.create(alpha, 438);
    self.result = self.filesystem.write(self.descriptor, "data Alpha {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(alpha);
    let artifact: &[u8] in Path = builder.output.resolve("artifact.bin");
    self.descriptor = self.filesystem.create(artifact, 438);
    self.result = self.filesystem.write(self.descriptor, "ordinary artifact");
    self.code = self.filesystem.close(self.descriptor);
    let beta: &[u8] in Path = builder.output.resolve("beta1.omg");
    self.descriptor = self.filesystem.create(beta, 438);
    self.result = self.filesystem.write(self.descriptor, "data Beta {}\n");
    self.code = self.filesystem.close(self.descriptor);
    let gamma: &[u8] in Path = builder.output.resolve("gamma.omg");
    self.descriptor = self.filesystem.create(gamma, 438);
    self.result = self.filesystem.write(self.descriptor, "data Gamma {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(gamma);
    builder.output.include_source(beta);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "multiple-generated-review")
            .expect("multiple generated sources with one ordinary artifact should compile");
    let summary = checked
        .build_observation_summary()
        .expect("multiple generated-source receipt retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .included_source_handoffs()
            .iter()
            .map(|handoff| (
                handoff.relative_path(),
                handoff.filesystem_attempt_ordinal()
            ))
            .collect::<Vec<_>>(),
        vec![
            (b"alpha.omg".as_slice(), 6),
            (b"gamma.omg".as_slice(), 15),
            (b"beta1.omg".as_slice(), 15),
        ]
    );
    assert_eq!(
        summary
            .staged_output_tree()
            .expect("mixed generated and ordinary outputs retain one exact tree")
            .entry_count(),
        4
    );
    for (path, source) in [
        ("alpha.omg", "data Alpha {}\n"),
        ("gamma.omg", "data Gamma {}\n"),
        ("beta1.omg", "data Beta {}\n"),
    ] {
        let generated = checked
            .typed
            .symbols
            .source_files()
            .find(|candidate| candidate.path.ends_with(path))
            .unwrap_or_else(|| panic!("generated source {path} enters final compilation"));
        assert_eq!(generated.source.as_ref(), source);
    }
    assert!(
        checked
            .typed
            .symbols
            .source_files()
            .all(|source| !source.path.ends_with("artifact.bin"))
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("multiple generated-source receipt should encode")
        .expect("multiple generated-source receipt should retain replay custody");
    let mut gamma_then_beta = replay_handoff_lane(b"gamma.omg", 15);
    gamma_then_beta.extend_from_slice(&replay_handoff_lane(b"beta1.omg", 15));
    let mut beta_then_gamma = replay_handoff_lane(b"beta1.omg", 15);
    beta_then_gamma.extend_from_slice(&replay_handoff_lane(b"gamma.omg", 15));
    let reordered_record = recover_review_only_build_filesystem_replay_record(
        &replace_unique_bytes(record.canonical_bytes(), &gamma_then_beta, &beta_then_gamma),
        limits,
    )
    .expect("reordered distinct handoffs remain canonically framed");
    let mut early_gamma = replay_handoff_lane(b"gamma.omg", 15);
    let early_ordinal_offset = early_gamma.len() - 8;
    early_gamma[early_ordinal_offset..].copy_from_slice(&14u64.to_le_bytes());
    assert!(
        recover_review_only_build_filesystem_replay_record(
            &replace_unique_bytes(
                record.canonical_bytes(),
                &replay_handoff_lane(b"gamma.omg", 15),
                &early_gamma,
            ),
            limits,
        )
        .expect_err("handoff before its matching close must reject recovery")
        .message()
        .contains("does not follow its Output close")
    );

    let package = PackageKeyIdentity::from_digest([100; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "multiple-generated", project.clone())
        .with_canonical_source_metadata()
        .expect("capture multiple-generated canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package multiple-generated input");
    let reordered = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs.clone(),
        reordered_record,
    )
    .expect_err("reordered generated-source handoffs must reject authored replay");
    assert!(reordered.iter().any(|diagnostic| {
        diagnostic.message.contains("handoff")
            && (diagnostic
                .message
                .contains("requires a scoped build-output grant")
                || diagnostic.message.contains("sequence changed"))
    }));

    std::fs::write(
        rooted_build_session(&project, "multiple-generated-review").join("output/gamma.omg"),
        "data Spoofed {}\n",
    )
    .expect("change one physical generated Output after receipt capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("multiple generated-source replay ignores host Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert_eq!(
        replayed
            .build_observation_summary()
            .expect("reopened multiple-generated receipt retains observations")
            .included_source_handoffs(),
        summary.included_source_handoffs()
    );
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("gamma.omg") && source.source.as_ref() == "data Gamma {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "multiple-generated-review"));
}

#[test]
fn sequential_full_writes_replay_as_concatenated_output_files() {
    let (project, profile) = rooted_build_probe_project(
        "sequential-full-output-writes",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("multiwrite.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Multi");
    self.result = self.filesystem.write(self.descriptor, "");
    self.result = self.filesystem.write(self.descriptor, "Write {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);
    let artifact: &[u8] in Path = builder.output.resolve("multiwrite.bin");
    self.descriptor = self.filesystem.create(artifact, 438);
    self.result = self.filesystem.write(self.descriptor, "first-");
    self.result = self.filesystem.write(self.descriptor, "second");
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "sequential-writes-review")
            .expect("multiple full sequential writes should receipt");
    let summary = checked
        .build_observation_summary()
        .expect("multiwrite receipt retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 5, 5, 8, 1, 5, 5, 8]
    );
    assert_eq!(
        summary
            .included_source_handoffs()
            .iter()
            .map(|handoff| (
                handoff.relative_path(),
                handoff.filesystem_attempt_ordinal()
            ))
            .collect::<Vec<_>>(),
        vec![(b"multiwrite.omg".as_slice(), 8)]
    );
    assert_eq!(
        checked
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("multiwrite.omg"))
            .expect("concatenated generated source enters final compilation")
            .source
            .as_ref(),
        "data MultiWrite {}\n"
    );
    assert_eq!(
        std::fs::read(
            rooted_build_session(&project, "sequential-writes-review")
                .join("output/multiwrite.bin")
        )
        .unwrap(),
        b"first-second"
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("multiwrite receipt should encode")
        .expect("multiwrite receipt should retain replay custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("multiwrite replay record should recover"),
        record
    );

    let package = PackageKeyIdentity::from_digest([101; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "multiwrite", project.clone())
        .with_canonical_source_metadata()
        .expect("capture multiwrite canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package multiwrite input");
    std::fs::write(
        rooted_build_session(&project, "sequential-writes-review").join("output/multiwrite.omg"),
        "data Spoofed {}\n",
    )
    .expect("change physical multiwrite Output after receipt capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("multiwrite replay ignores host Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert_eq!(
        replayed
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("multiwrite.omg"))
            .expect("replayed concatenated source enters final compilation")
            .source
            .as_ref(),
        "data MultiWrite {}\n"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "sequential-writes-review"));
}

#[test]
fn full_positioned_writes_replay_exact_cursor_and_extent() {
    let (project, profile) = rooted_build_probe_project(
        "positioned-full-output-writes",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("positioned.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data ");
    self.result = self.filesystem.write_at(self.descriptor, "Positioned {}\n", 5);
    self.result = self.filesystem.write(self.descriptor, "Positioned {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);
    let artifact: &[u8] in Path = builder.output.resolve("positioned.bin");
    self.descriptor = self.filesystem.create(artifact, 438);
    self.result = self.filesystem.write(self.descriptor, "head");
    self.result = self.filesystem.write_at(self.descriptor, "tail", 8);
    self.result = self.filesystem.write(self.descriptor, "-cur");
    self.result = self.filesystem.write_at(self.descriptor, "", 40);
    self.code = self.filesystem.close(self.descriptor);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "positioned-writes-review")
            .expect("full positioned writes should receipt");
    let summary = checked
        .build_observation_summary()
        .expect("positioned-write receipt retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 7, 5, 8, 1, 5, 7, 5, 7, 8]
    );
    assert_eq!(
        summary
            .included_source_handoffs()
            .iter()
            .map(|handoff| (
                handoff.relative_path(),
                handoff.filesystem_attempt_ordinal()
            ))
            .collect::<Vec<_>>(),
        vec![(b"positioned.omg".as_slice(), 8)]
    );
    assert_eq!(
        checked
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("positioned.omg"))
            .expect("positioned generated source enters final compilation")
            .source
            .as_ref(),
        "data Positioned {}\n"
    );
    assert_eq!(
        std::fs::read(
            rooted_build_session(&project, "positioned-writes-review")
                .join("output/positioned.bin")
        )
        .unwrap(),
        b"head-curtail"
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("positioned-write receipt should encode")
        .expect("positioned-write receipt should retain replay custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("positioned-write replay record should recover"),
        record
    );

    let package = PackageKeyIdentity::from_digest([102; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "positioned", project.clone())
        .with_canonical_source_metadata()
        .expect("capture positioned-write canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package positioned-write input");
    std::fs::write(
        rooted_build_session(&project, "positioned-writes-review").join("output/positioned.omg"),
        "data Spoofed {}\n",
    )
    .expect("change physical positioned Output after receipt capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("positioned-write replay ignores host Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert_eq!(
        replayed
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("positioned.omg"))
            .expect("replayed positioned source enters final compilation")
            .source
            .as_ref(),
        "data Positioned {}\n"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "positioned-writes-review"));
}

#[test]
fn empty_output_file_replays_without_synthetic_write() {
    let (project, profile) = rooted_build_probe_project(
        "empty-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let artifact: &[u8] in Path = builder.output.resolve("empty.bin");
    self.descriptor = self.filesystem.create(artifact, 438);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("after-empty.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data AfterEmpty {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "empty-output-review")
            .expect("empty ordinary Output file should receipt");
    let summary = checked
        .build_observation_summary()
        .expect("empty-output receipt retains observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 8, 1, 5, 8]
    );
    assert_eq!(
        summary
            .included_source_handoffs()
            .iter()
            .map(|handoff| (
                handoff.relative_path(),
                handoff.filesystem_attempt_ordinal()
            ))
            .collect::<Vec<_>>(),
        vec![(b"after-empty.omg".as_slice(), 8)]
    );
    let tree = summary
        .staged_output_tree()
        .expect("empty and generated Output files retain one exact tree");
    assert_eq!(tree.entry_count(), 2);
    assert!(
        std::fs::read(
            rooted_build_session(&project, "empty-output-review").join("output/empty.bin")
        )
        .unwrap()
        .is_empty()
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .expect("empty-output receipt should encode")
        .expect("empty-output receipt should retain replay custody");
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .expect("empty-output replay record should recover"),
        record
    );

    let package = PackageKeyIdentity::from_digest([103; 32]).expect("nonzero package identity");
    set_canonical_source_tree_permissions(&project, true);
    let package_source = PackageSourceBinding::new(package, "empty-output", project.clone())
        .with_canonical_source_metadata()
        .expect("capture empty-output canonical metadata");
    let package_inputs =
        PackageCompilationInputs::new_package(package, vec![package_source], Vec::new())
            .expect("single-package empty-output input");
    std::fs::write(
        rooted_build_session(&project, "empty-output-review").join("output/empty.bin"),
        b"spoofed",
    )
    .expect("change physical empty Output after receipt capture");
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs,
        record,
    )
    .expect("empty-output replay ignores host Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert_eq!(
        replayed
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("after-empty.omg"))
            .expect("source after empty Output enters final compilation")
            .source
            .as_ref(),
        "data AfterEmpty {}\n"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "empty-output-review"));
}

#[test]
fn output_sync_operations_replay_in_authored_order() {
    let (project, profile) = rooted_build_probe_project(
        "synced-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("synced.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.code = self.filesystem.sync(self.descriptor);
    self.result = self.filesystem.write(self.descriptor, "data Synced {}\n");
    self.code = self.filesystem.sync_data(self.descriptor);
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "synced-output-review")
            .expect("successful Output sync operations should receipt");
    let summary = checked.build_observation_summary().unwrap();
    assert_eq!(summary.schema_version(), BUILD_OBSERVATION_SCHEMA_VERSION);
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 43, 5, 44, 8]
    );
    assert_eq!(
        summary.included_source_handoffs()[0].filesystem_attempt_ordinal(),
        8
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .unwrap(),
        record
    );
    let package = PackageKeyIdentity::from_digest([104; 32]).unwrap();
    set_canonical_source_tree_permissions(&project, true);
    let source = PackageSourceBinding::new(package, "synced-output", project.clone())
        .with_canonical_source_metadata()
        .unwrap();
    let inputs = PackageCompilationInputs::new_package(package, vec![source], Vec::new()).unwrap();
    std::fs::write(
        rooted_build_session(&project, "synced-output-review").join("output/synced.omg"),
        "data Spoofed {}\n",
    )
    .unwrap();
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("sync replay ignores physical Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("synced.omg") && source.source.as_ref() == "data Synced {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "synced-output-review"));
}

#[test]
fn output_duplicate_and_immediate_close_replay_exact_lineage() {
    let (project, profile) = rooted_build_probe_project(
        "duplicated-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("duplicated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.code = self.filesystem.duplicate(self.descriptor);
    self.code = self.filesystem.close(self.code);
    self.result = self.filesystem.write(self.descriptor, "data Duplicated {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let session = rooted_build_session(&project, "duplicated-output-review");
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "duplicated-output-review")
            .expect("successful Output duplicate and immediate close should receipt");
    let summary = checked.build_observation_summary().unwrap();
    assert_eq!(summary.schema_version(), BUILD_OBSERVATION_SCHEMA_VERSION);
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 45, 8, 5, 8]
    );
    let duplicate = &summary.filesystem_operation_attempts()[4];
    let omega_build_evaluation::BuildFilesystemLogicalHandleInputResolution::Resolved(
        source_identity,
    ) = duplicate.logical_handle_inputs()[0].resolution()
    else {
        panic!("duplicate source descriptor is resolved")
    };
    let output = duplicate
        .logical_handle_output()
        .expect("duplicate retains its fresh logical descriptor");
    assert!(matches!(
        output.source(),
        omega_build_evaluation::BuildFilesystemLogicalHandleOutputSource::Duplicated(identity)
            if identity == source_identity
    ));
    assert_eq!(
        summary.included_source_handoffs()[0].filesystem_attempt_ordinal(),
        8
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .unwrap(),
        record
    );

    let package = PackageKeyIdentity::from_digest([109; 32]).unwrap();
    set_canonical_source_tree_permissions(&project, true);
    let source = PackageSourceBinding::new(package, "duplicated-output", project.clone())
        .with_canonical_source_metadata()
        .unwrap();
    let inputs = PackageCompilationInputs::new_package(package, vec![source], Vec::new()).unwrap();
    std::fs::write(session.join("output/duplicated.omg"), "data Spoofed {}\n").unwrap();
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("duplicate replay ignores physical Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("duplicated.omg") && source.source.as_ref() == "data Duplicated {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn output_set_len_replays_exact_truncation() {
    let (project, profile) = rooted_build_probe_project(
        "resized-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("resized.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data SetLen {}\ngarbage");
    self.code = self.filesystem.set_len(self.descriptor, 15);
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "resized-output-review")
            .expect("successful Output set_len should receipt");
    let summary = checked.build_observation_summary().unwrap();
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 41, 8]
    );
    assert_eq!(
        checked
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("resized.omg"))
            .unwrap()
            .source
            .as_ref(),
        "data SetLen {}\n"
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .unwrap(),
        record
    );
    let package = PackageKeyIdentity::from_digest([105; 32]).unwrap();
    set_canonical_source_tree_permissions(&project, true);
    let source = PackageSourceBinding::new(package, "resized-output", project.clone())
        .with_canonical_source_metadata()
        .unwrap();
    let inputs = PackageCompilationInputs::new_package(package, vec![source], Vec::new()).unwrap();
    std::fs::write(
        rooted_build_session(&project, "resized-output-review").join("output/resized.omg"),
        "data Spoofed {}\n",
    )
    .unwrap();
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("set_len replay ignores physical Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("resized.omg") && source.source.as_ref() == "data SetLen {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "resized-output-review"));
}

#[cfg(unix)]
#[test]
fn output_set_file_permissions_replays_exact_executable_class() {
    use std::os::unix::fs::PermissionsExt;

    let (project, profile) = rooted_build_probe_project(
        "permissioned-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let tool: &[u8] in Path = builder.output.resolve("tool.bin");
    self.descriptor = self.filesystem.create(tool, 438);
    self.result = self.filesystem.write(self.descriptor, "tool");
    self.code = self.filesystem.set_file_permissions(self.descriptor, 493);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("permissioned.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Permissioned {}\n");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let session = rooted_build_session(&project, "permissioned-output-review");
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "permissioned-output-review")
            .expect("successful Output descriptor permission change should receipt");
    let summary = checked.build_observation_summary().unwrap();
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 17, 8, 1, 5, 8]
    );
    let captured_digest = summary.staged_output_tree().unwrap().digest();
    let captured_materialization = session.join("captured-materialization");
    std::fs::create_dir(&captured_materialization).unwrap();
    summary
        .staged_output_tree()
        .unwrap()
        .materialize_into(&captured_materialization)
        .unwrap();
    assert_ne!(
        std::fs::metadata(captured_materialization.join("tool.bin"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .unwrap(),
        record
    );
    let package = PackageKeyIdentity::from_digest([107; 32]).unwrap();
    set_canonical_source_tree_permissions(&project, true);
    let source = PackageSourceBinding::new(package, "permissioned-output", project.clone())
        .with_canonical_source_metadata()
        .unwrap();
    let inputs = PackageCompilationInputs::new_package(package, vec![source], Vec::new()).unwrap();
    let physical_tool = session.join("output/tool.bin");
    std::fs::write(&physical_tool, "spoofed").unwrap();
    std::fs::set_permissions(&physical_tool, std::fs::Permissions::from_mode(0o644)).unwrap();
    std::fs::write(session.join("output/permissioned.omg"), "data Spoofed {}\n").unwrap();
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("permission replay ignores physical Output content and mode drift");
    set_canonical_source_tree_permissions(&project, false);
    let replayed_summary = replayed.build_observation_summary().unwrap();
    assert_eq!(
        replayed_summary.staged_output_tree().unwrap().digest(),
        captured_digest
    );
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("permissioned.omg")
            && source.source.as_ref() == "data Permissioned {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn output_set_file_times_replays_exact_timespec_carrier() {
    let (project, profile) = rooted_build_probe_project(
        "dated-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("dated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Dated {}\n");
    self.times[0] = 11;
    self.times[16] = 29;
    self.code = self.filesystem.set_file_times(self.descriptor, &mut self.times);
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let session = rooted_build_session(&project, "dated-output-review");
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "dated-output-review")
            .expect("successful Output descriptor time change should receipt");
    let summary = checked.build_observation_summary().unwrap();
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 42, 8]
    );
    let time_attempt = &summary.filesystem_operation_attempts()[5];
    let [resolution] = time_attempt.mutable_byte_operand_resolutions() else {
        panic!("set_file_times retains one resolution-time carrier")
    };
    let [carrier] = time_attempt.mutable_byte_operands() else {
        panic!("set_file_times retains one provider carrier")
    };
    assert_eq!(resolution.operand_ordinal(), 1);
    assert_eq!(resolution.bytes().len(), 32);
    assert_eq!(resolution.bytes()[0], 11);
    assert_eq!(resolution.bytes()[16], 29);
    assert_eq!(resolution.bytes(), carrier.pre_bytes());
    assert_eq!(carrier.pre_bytes(), carrier.post_bytes());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .unwrap(),
        record
    );

    let package = PackageKeyIdentity::from_digest([108; 32]).unwrap();
    set_canonical_source_tree_permissions(&project, true);
    let source = PackageSourceBinding::new(package, "dated-output", project.clone())
        .with_canonical_source_metadata()
        .unwrap();
    let inputs = PackageCompilationInputs::new_package(package, vec![source], Vec::new()).unwrap();
    std::fs::write(session.join("output/dated.omg"), "data Spoofed {}\n").unwrap();
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("time replay ignores physical Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("dated.omg") && source.source.as_ref() == "data Dated {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(session);
}

#[test]
fn output_seek_replays_exact_cursor_transition() {
    let (project, profile) = rooted_build_probe_project(
        "seeked-output-file",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(input, 0);
    self.result = self.filesystem.read(self.descriptor, &mut self.buffer, 23);
    self.code = self.filesystem.close(self.descriptor);
    let generated: &[u8] in Path = builder.output.resolve("seeked.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "xxxx Seeked {}\n");
    self.result = self.filesystem.seek(self.descriptor, 0, 0);
    self.result = self.filesystem.write(self.descriptor, "data");
    self.code = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );
    let checked =
        compile_rooted_probe_with_sponsored_output(&project, profile, "seeked-output-review")
            .expect("successful canonical Output seek should receipt");
    let summary = checked.build_observation_summary().unwrap();
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 10, 5, 8]
    );
    assert_eq!(
        checked
            .typed
            .symbols
            .source_files()
            .find(|source| source.path.ends_with("seeked.omg"))
            .unwrap()
            .source
            .as_ref(),
        "data Seeked {}\n"
    );

    let limits = BuildFilesystemReplayRecordLimits::default();
    let record = capture_verified_build_filesystem_replay_record(summary, limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        recover_review_only_build_filesystem_replay_record(record.canonical_bytes(), limits)
            .unwrap(),
        record
    );
    let package = PackageKeyIdentity::from_digest([106; 32]).unwrap();
    set_canonical_source_tree_permissions(&project, true);
    let source = PackageSourceBinding::new(package, "seeked-output", project.clone())
        .with_canonical_source_metadata()
        .unwrap();
    let inputs = PackageCompilationInputs::new_package(package, vec![source], Vec::new()).unwrap();
    std::fs::write(
        rooted_build_session(&project, "seeked-output-review").join("output/seeked.omg"),
        "data Spoofed {}\n",
    )
    .unwrap();
    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        inputs,
        record,
    )
    .expect("seek replay ignores physical Output drift");
    set_canonical_source_tree_permissions(&project, false);
    assert!(replayed.typed.symbols.source_files().any(|source| {
        source.path.ends_with("seeked.omg") && source.source.as_ref() == "data Seeked {}\n"
    }));

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "seeked-output-review"));
}

#[test]
fn staged_omega_source_requires_an_explicit_handoff() {
    let (project, profile) = rooted_build_probe_project(
        "unhanded-source",
        r#"    let generated: &[u8] in Path = builder.output.resolve("unhanded.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Unhanded {}\n");
    self.descriptor = self.filesystem.close(self.descriptor);"#,
    );

    let diagnostics =
        compile_rooted_probe_with_sponsored_output(&project, profile, "unhanded-review")
            .expect_err("an output filename cannot implicitly inject Omega source");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("unhanded.omg")
            && rendered.contains("no explicit include_source handoff"),
        "{rendered}"
    );

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "unhanded-review"));
}

#[test]
fn handed_off_generated_source_must_pass_the_final_frontend() {
    let (project, profile) = rooted_build_probe_project(
        "invalid-generated-source",
        r#"    let generated: &[u8] in Path = builder.output.resolve("invalid.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Invalid {\n");
    self.descriptor = self.filesystem.close(self.descriptor);
    builder.output.include_source(generated);"#,
    );

    let diagnostics =
        compile_rooted_probe_with_sponsored_output(&project, profile, "invalid-review")
            .expect_err("generated source is ordinary candidate code and must parse");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("invalid.omg"), "{rendered}");

    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(rooted_build_session(&project, "invalid-review"));
}

#[test]
fn runtime_console_is_not_a_package_build_service() {
    let profile = omega_target::TargetProfile::host();
    let project = std::env::temp_dir().join(format!(
        "omega-build-config-console-only-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project directory");

    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::console;

target {target} {{}}

machine build(builder: &mut Build)
reaches Console
{{
    builder.application("console-only-build");
    let mut console: Console;
    console.write_line("build: console only");
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").expect("write main.omg");

    let package = PackageKeyIdentity::from_digest([83; 32]).expect("nonzero package identity");
    let package_inputs = PackageCompilationInputs::new_package(
        package,
        vec![PackageSourceBinding::new(
            package,
            "console-build",
            project.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package compiler input");
    let diagnostics = compile_to_checked_with_packages_in_build_dir(
        &project.join("main.omg"),
        &project.join("build"),
        Some(profile.target_name()),
        package_inputs,
    )
    .expect_err("runtime Console must not be admitted as a package build service");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Console"), "{rendered}");
    assert!(rendered.contains("FilesystemHost"), "{rendered}");

    let _ = std::fs::remove_dir_all(&project);
}
