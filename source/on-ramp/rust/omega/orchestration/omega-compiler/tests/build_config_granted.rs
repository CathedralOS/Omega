//! Granted build-host staging round trip: a build machine with a declared exact
//! toolchain `FilesystemHost` service ceiling runs at compile time through the
//! granted interpreter entry, scoped to source reads and build-output writes,
//! and stages an asset itself while its ordinary `Build` image facts flow into
//! the pipeline. A declared exact toolchain `Console` boundary write is served
//! without incidentally supplying filesystem authority. Fail canaries cover
//! undeclared services and package-authored boundary lookalikes.

use omega_compiler::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemMetadataObservationKind,
    BuildFilesystemObservedByteRegionKind, BuildFilesystemOperationResult, BuildFilesystemProvider,
    BuildFilesystemReplayRecordLimits, BuildFilesystemReturnedPathCompleteness,
    BuildFilesystemReturnedPathKind, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildObservationClass, CheckedCompilation, CompileOptions, FilesystemSponsor,
    PackageCompilationInputs, PackageSourceBinding,
    capture_verified_build_filesystem_replay_record, compile_to_checked,
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir, compile_to_checked_with_replay_record,
    recover_review_only_build_filesystem_replay_record,
};

fn compile(
    options: CompileOptions,
) -> Result<omega_compiler::CompileReport, Vec<psi_diagnostics::Diagnostic>> {
    omega_compiler::compile(omega_compiler::CompileRequest::new(options))
}

use psi_core::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::process::Command;

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    }
}

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

fn rooted_build_probe_project(label: &str, body: &str) -> (PathBuf, omega_target::TargetProfile) {
    let profile = omega_target::TargetProfile::host();
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

data FakeRoot {{}}
machine FakeRoot::resolve<'path>(&self, relative: &'path [u8] in Path) -> &'path [u8] in Path {{
    relative
}}

data RootProbe {{
    filesystem: FilesystemHost;
    fake: FakeRoot;
    descriptor: i32;
    code: i32;
    result: i64;
    position: i64;
    buffer: [u8; 4096];
    small_buffer: [u8; 1];
}}

machine RootProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
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

fn compile_rooted_probe_with_sponsored_output(
    project: &std::path::Path,
    profile: omega_target::TargetProfile,
    label: &str,
) -> Result<CheckedCompilation, Vec<psi_diagnostics::Diagnostic>> {
    let session = project.join(label);
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

    let package = PackageKeyIdentity::from_digest([97; 32]).expect("nonzero package identity");
    let package_inputs = PackageCompilationInputs::new(
        package,
        vec![PackageSourceBinding::new(
            package,
            "generated-source",
            project.to_path_buf(),
        )],
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
            r#"use omega::language::std::console;
use omega::language::std::filesystem_host;

target {target} {{}}

data Stager {{
    fs: FilesystemHost;
    log: Console;
    fd: i32;
    clone_fd: i32;
    handle: i64;
    buffer: [u8; 6];
    n: i64;
    rc: i32;
}}

machine Stager::build(&mut self, builder: &mut Build)
reaches
    FilesystemHost + Console
{{
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    self.log.write_line("build: staging");
    let source_path: &[u8] in Path = builder.source.resolve("inputs/table.txt");
    self.fd = self.fs.open(source_path, 0);
    self.n = self.fs.read(self.fd, &mut self.buffer, 6);
    self.handle = self.fs.get_osfhandle(self.fd);
    self.rc = self.fs.close(self.fd);
    let staged_path: &[u8] in Path = builder.output.resolve("stage/asset.tmp");
    self.fd = self.fs.create(staged_path, 438);
    transition self.fd >= 0 {{ true -> put(builder) _ -> done(builder) }}
    state put(&mut self, builder: &mut Build) {{
        self.clone_fd = self.fs.duplicate(self.fd);
        self.n = self.fs.write(self.clone_fd, "staged by build\n");
        _ = self.fs.sync(self.clone_fd);
        self.n = self.fs.close(self.clone_fd);
        self.n = self.fs.close(self.fd);
        let staged_path: &[u8] in Path = builder.output.resolve("stage/asset.tmp");
        let final_path: &[u8] in Path = builder.output.resolve("stage/asset.bin");
        self.rc = self.fs.rename(staged_path, final_path);
        transition true {{ true -> done(builder) _ -> done(builder) }}
    }}
    state done(&mut self, builder: &mut Build) {{
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
    assert_eq!(checked_usage.usage_schema_version, 1);
    assert_eq!(checked_usage.step_schedule_marker, 1);
    assert!(checked_usage.fuel_units > 0);
    assert!(checked_usage.result_cells > 0);
    let checked_observations = checked
        .build_observation_summary()
        .expect("build machine evaluation must publish observation evidence");
    assert_eq!(checked_observations.schema_version(), 24);
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
        18
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

    let report = compile(CompileOptions {
        root_path: PathBuf::from(project.join("main.omg")),
        build_dir: Some(build_dir.clone()),
        target_name: Some(profile.target_name().to_owned()),
        write_output: true,
    })
    .expect("declared filesystem+console build.omg should compile (console rows are SERVED, not backstopped)");
    assert!(report.wrote_output());
    assert_eq!(report.build_evaluation_usage, Some(checked_usage));
    assert_eq!(
        report.build_observation_summary.as_ref(),
        Some(checked_observations)
    );

    let staged = std::fs::read_to_string(stage.join("asset.bin"))
        .expect("the build machine should have staged stage/asset.bin at compile time");
    assert_eq!(staged, "staged by build\n");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("compiled program should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the staged program to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let sponsored_session = project.join("sponsored-review");
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
    let package = PackageKeyIdentity::from_digest([91; 32]).expect("nonzero package identity");
    let package_inputs = PackageCompilationInputs::new(
        package,
        vec![PackageSourceBinding::new(
            package,
            "sponsored-build",
            project.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package compiler input");
    let sponsored = compile_to_checked_with_packages_in_sponsored_build_dir(
        &project.join("main.omg"),
        &sponsored_build,
        Some(profile.target_name()),
        package_inputs,
        sponsor,
    )
    .expect("sponsored package build must retain staged-output custody");
    let sponsored_tree = sponsored
        .build_observation_summary()
        .and_then(|summary| summary.staged_output_tree())
        .expect("sponsored filesystem build commits its complete staged tree");
    assert_eq!(sponsored_tree.entry_count(), 2);
    assert_eq!(sponsored_tree.file_bytes(), 16);

    let _ = std::fs::remove_dir_all(&project);
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
    let absent_output = project.join("build/absent.bin");
    let mixed_from = project.join("build/mixed-from.bin");
    let mixed_to = project.join("stage/mixed-to.bin");
    let rename_from = project.join("stage/rename-from.bin");
    let rename_to = project.join("stage/rename-to.bin");
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target {target} {{}}

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

data SourceWriter {{
    fs: FilesystemHost;
    fd: i32;
    rc: i32;
}}

machine SourceWriter::build(&mut self, builder: &mut Build)
reaches
    FilesystemHost
{{
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    self.fd = self.fs.create("{forbidden}", 438);
    self.fd = self.fs.create("{unresolvable}", 438);
    self.rc = self.fs.close(self.fd);
    self.rc = self.fs.remove("{absent_output}");
    self.rc = self.fs.rename("{mixed_from}", "{mixed_to}");
    self.rc = self.fs.rename("{rename_from}", "{rename_to}");
    builder.freestanding = false;
}}
"#,
            // Forward slashes so the embedded path lexes on windows too.
            forbidden = forbidden.display().to_string().replace('\\', "/"),
            unresolvable = unresolvable.display().to_string().replace('\\', "/"),
            absent_output = absent_output.display().to_string().replace('\\', "/"),
            mixed_from = mixed_from.display().to_string().replace('\\', "/"),
            mixed_to = mixed_to.display().to_string().replace('\\', "/"),
            rename_from = rename_from.display().to_string().replace('\\', "/"),
            rename_to = rename_to.display().to_string().replace('\\', "/"),
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

    let report = compile(CompileOptions {
        root_path: PathBuf::from(project.join("main.omg")),
        build_dir: Some(project.join("build")),
        target_name: Some(profile.target_name().to_owned()),
        write_output: false,
    })
    .expect(
        "declared filesystem build.omg should compile while denied source write returns fd < 0",
    );
    let observations = report
        .build_observation_summary
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

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

data SourceMetadataWriter {{
    fs: FilesystemHost;
    descriptor: i32;
    result: i32;
}}

machine SourceMetadataWriter::build(&mut self, builder: &mut Build)
reaches
    FilesystemHost
{{
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    self.descriptor = self.fs.open("{source_file}", 0);
    self.result = self.fs.set_file_permissions(self.descriptor, 511);
    self.result = self.fs.lock_file(self.descriptor, 6);
    self.result = self.fs.close(self.descriptor);
    builder.freestanding = false;
}}
"#,
            source_file = source_file.display().to_string().replace('\\', "/"),
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

    let report = compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(project.join("build")),
        target_name: Some(profile.target_name().to_owned()),
        write_output: false,
    })
    .expect("a denied descriptor metadata mutation is an ordinary build result");
    let observations = report
        .build_observation_summary
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

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

data FailingStager {{
    fs: FilesystemHost;
    fd: i32;
    buffer: [u8; 1];
    n: i64;
}}

machine FailingStager::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
    builder.roots.bind({root_owner}::ProgramEntry, Main::main);
    self.fd = self.fs.create("{staged}", 438);
    self.n = self.fs.read(self.fd, &mut self.buffer, 16777217);
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
            root_owner = profile.root_slot_owner_name(),
            staged = staged.display().to_string().replace('\\', "/"),
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
fn failed_filesystem_preparation_retains_the_completed_path_like_operand_prefix() {
    let (project, profile) = rooted_build_probe_project(
        "path-like-preparation-prefix",
        r#"    self.result = self.filesystem.find_first("missing/*", &mut self.small_buffer);"#,
    );
    let diagnostics = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("undersized find-data output must reject build evaluation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("filesystem output requires 320 bytes"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1 path-like operand(s)"),
        "the completed find pattern must survive the later buffer-capacity failure: {rendered}"
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
            "linux_x64",
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
            "windows_x64",
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

data MetadataProbe {{
    filesystem: FilesystemHost;
    buffer: [u8; 144];
    descriptor: i32;
    result: i32;
}}

machine MetadataProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{{
    self.buffer[36] = 255;
    self.buffer[143] = 255;
    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.result = self.filesystem.read_metadata(path, &mut self.buffer);
    self.descriptor = self.filesystem.open(path, 0);
    self.result = self.filesystem.read_file_metadata(self.descriptor, &mut self.buffer);
    self.result = self.filesystem.close(self.descriptor);
    self.result = self.filesystem.read_symlink_metadata(path, &mut self.buffer);
    let missing: &[u8] in Path = builder.source.resolve("missing.omg");
    self.result = self.filesystem.read_metadata(missing, &mut self.buffer);
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
    assert!(summary.source_inputs_replay_verified());
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
    assert!(replayed_summary.source_inputs_replay_verified());
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts()
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read metadata replay build");
    let changed_build = original_build.replacen(
        "self.filesystem.read_metadata(link, &mut self.buffer)",
        "self.filesystem.read_symlink_metadata(link, &mut self.buffer)",
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
    assert!(summary.source_inputs_replay_verified());
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
fn failed_or_descriptor_metadata_does_not_claim_source_input_replay() {
    for (label, body) in [
        (
            "failed-source-metadata-no-replay",
            r#"    let path: &[u8] in Path = builder.source.resolve("missing.omg");
    self.code = self.filesystem.read_metadata(path, &mut self.buffer);"#,
        ),
        (
            "descriptor-metadata-no-replay",
            r#"    let path: &[u8] in Path = builder.source.resolve("main.omg");
    self.descriptor = self.filesystem.open(path, 0);
    self.code = self.filesystem.read_file_metadata(self.descriptor, &mut self.buffer);
    self.code = self.filesystem.close(self.descriptor);"#,
        ),
    ] {
        let (project, profile) = rooted_build_probe_project(label, body);
        let compilation =
            compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
                .expect("out-of-rung metadata remains an ordinary observed build");
        let summary = compilation
            .build_observation_summary()
            .expect("metadata build retains observations");
        assert!(!summary.source_inputs_replay_verified(), "{label}");
        assert!(
            capture_verified_build_filesystem_replay_record(
                summary,
                BuildFilesystemReplayRecordLimits::default(),
            )
            .expect("non-replayed metadata summary is not a codec error")
            .is_none(),
            "{label}"
        );
        let _ = std::fs::remove_dir_all(&project);
    }
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
    assert!(summary.source_inputs_replay_verified());
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
    let record_header_bytes = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0".len() + 2 + 4 + 4 + 8;
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
    assert!(replayed_summary.source_inputs_replay_verified());
    assert_eq!(replayed_summary.ceiling(), BuildObservationClass::Volatile);
    assert_eq!(replayed_summary.realized(), BuildObservationClass::Volatile);
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
        "self.filesystem.read(self.descriptor, &mut self.buffer, 23)",
        "self.filesystem.read(self.descriptor, &mut self.buffer, 22)",
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
    assert!(summary.source_inputs_replay_verified());
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
    read_attempt_prefix.extend_from_slice(&[2, 0]);
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
        "self.filesystem.read_at(self.descriptor, &mut self.buffer, 7, 5)",
        "self.filesystem.read_at(self.descriptor, &mut self.buffer, 7, 6)",
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
    assert!(summary.source_inputs_replay_verified());
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
    assert!(replayed_summary.source_inputs_replay_verified());
    assert_eq!(
        replayed_summary.filesystem_operation_attempts(),
        summary.filesystem_operation_attempts(),
        "reopened replay must reproduce every ordered read lane exactly"
    );

    let build_path = project.join("build.omg");
    let original_build = std::fs::read_to_string(&build_path).expect("read build probe");
    let changed_build = original_build.replace(
        "self.filesystem.read_at(self.descriptor, &mut self.buffer, 4, 10)",
        "self.filesystem.read_at(self.descriptor, &mut self.buffer, 4, 11)",
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
    assert!(summary.source_inputs_replay_verified());
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
    assert!(replayed_summary.source_inputs_replay_verified());
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
    assert!(!summary.source_inputs_replay_verified());
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
        assert!(!summary.source_inputs_replay_verified(), "{label}");
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
    assert!(!summary.source_inputs_replay_verified());
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
fn failed_filesystem_preparation_retains_the_completed_logical_handle_prefix() {
    let (project, profile) = rooted_build_probe_project(
        "logical-handle-preparation-prefix",
        r#"    self.code = self.filesystem.find_next(7, &mut self.small_buffer);"#,
    );
    let diagnostics = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect_err("undersized find-data output must reject build evaluation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("filesystem output requires 320 bytes"),
        "{rendered}"
    );
    assert!(
        rendered.contains("1 logical-handle operand(s)"),
        "the completed find handle must survive the later buffer-capacity failure: {rendered}"
    );
    assert!(
        rendered.contains("1 mutable-carrier operand(s)"),
        "the completed find-data carrier must survive its capacity failure: {rendered}"
    );
    let _ = std::fs::remove_dir_all(&project);
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
    let report = compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(project.join("build")),
        target_name: Some(profile.target_name().to_owned()),
        write_output: false,
    })
    .expect("a denied source-root write is an observed build result");
    let observations = report
        .build_observation_summary
        .expect("source-root denial remains observable");
    let [create] = observations.filesystem_operation_attempts() else {
        panic!("source-root write must retain one attempted create")
    };
    assert_eq!(create.result(), BuildFilesystemOperationResult::Scalar(-1));
    assert_eq!(create.post_error(), 13);
    assert!(create.authorized_paths().is_empty());
    let [refusal] = create.grant_refusals() else {
        panic!("source-root write must retain its refused path")
    };
    assert_eq!(refusal.access(), BuildFilesystemGrantAccess::Write);
    assert_eq!(
        refusal.reason(),
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots
    );
    assert!(!project.join("blocked.bin").exists());
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

    let report = compile(CompileOptions {
        root_path: project.join("main.omg"),
        build_dir: Some(build_dir),
        target_name: Some(profile.target_name().to_owned()),
        write_output: false,
    })
    .expect("a denied output symlink escape remains an observed build result");
    let observations = report
        .build_observation_summary
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
}

#[cfg(unix)]
#[test]
fn returned_read_link_bytes_survive_compiler_projection() {
    use std::os::unix::fs::symlink;

    let (project, profile) = rooted_build_probe_project(
        "returned-read-link",
        r#"    let link: &[u8] in Path = builder.source.resolve("fixture-link");
    self.result = self.filesystem.read_link(link, &mut self.buffer, 4096);"#,
    );
    symlink("returned-target", project.join("fixture-link"))
        .expect("create source symlink fixture");
    let checked = compile_to_checked(&project.join("main.omg"), Some(profile.target_name()))
        .expect("read_link of a granted source symlink succeeds");
    let observations = checked
        .build_observation_summary()
        .expect("filesystem build publishes observation evidence");
    let [read_link] = observations.filesystem_operation_attempts() else {
        panic!("fixture performs one read_link operation")
    };
    let [returned] = read_link.returned_paths() else {
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
        read_link.result(),
        BuildFilesystemOperationResult::Scalar(15)
    );
    let _ = std::fs::remove_dir_all(&project);
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
            r#"    self.result = self.filesystem.final_path_name_by_handle(0, &mut self.buffer, 4096, 0);"#,
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
fn generated_source_handoff_requires_custody_and_enters_one_frozen_final_pass() {
    let (project, profile) = rooted_build_probe_project(
        "included-source",
        r#"    let generated: &[u8] in Path = builder.output.resolve("generated.omg");
    self.descriptor = self.filesystem.create(generated, 438);
    self.result = self.filesystem.write(self.descriptor, "data Generated {}\n");
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
    assert_eq!(generated.source.as_ref(), "data Generated {}\n");
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
        std::fs::read_to_string(project.join("sponsored-review/output/generated.omg")).unwrap(),
        "data Generated {}\n"
    );

    let _ = std::fs::remove_dir_all(&project);
}

#[test]
fn receipted_generated_source_reopens_without_host_source_or_output() {
    let (project, profile) = rooted_build_probe_project(
        "receipted-generated-source",
        r#"    let input: &[u8] in Path = builder.source.resolve("main.omg");
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
    assert!(summary.source_inputs_replay_verified());
    assert!(summary.operation_replay_verified());
    assert_eq!(summary.ceiling(), BuildObservationClass::Volatile);
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);
    assert_eq!(
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 5, 8]
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
    let package_inputs = || {
        PackageCompilationInputs::new(
            package,
            vec![PackageSourceBinding::new(
                package,
                "generated-source",
                project.clone(),
            )],
            Vec::new(),
        )
        .expect("single-package generated-source input")
    };
    let changed_diagnostics = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs(),
        changed_record,
    )
    .expect_err("changed retained output bytes must disagree with authored replay");
    assert!(changed_diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("replay") && diagnostic.message.contains("changed")
    }));

    std::fs::write(project.join("main.omg"), "data Main { value: u16; }\n")
        .expect("change host source after receipt capture");
    std::fs::write(
        project.join("receipted-review/output/generated.omg"),
        "data Spoofed {}\n",
    )
    .expect("change host output after receipt capture");

    let replayed = compile_to_checked_with_packages_and_replay_record(
        &project.join("main.omg"),
        Some(profile.target_name()),
        package_inputs(),
        record,
    )
    .expect("reopened receipt should reproduce build input and generated output");
    let replayed_summary = replayed
        .build_observation_summary()
        .expect("reopened receipt retains observations");
    assert!(replayed_summary.operation_replay_verified());
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

    let _ = std::fs::remove_dir_all(&project);
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
}

#[test]
fn console_only_build_machine_receives_no_real_filesystem_provider() {
    let profile = omega_target::TargetProfile::host();
    let project = std::env::temp_dir().join(format!(
        "omega-build-config-console-only-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create project directory");

    let non_directory = project.join("not-a-directory");
    std::fs::write(&non_directory, "blocks accidental build-root creation")
        .expect("create build-root blocker");
    let unavailable_build_root = non_directory.join("nested-build-root");

    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::console;

target {target} {{}}

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

data BuildLogger {{ console: Console; }}

machine BuildLogger::build(&mut self, builder: &mut Build)
reaches Console
{{
    self.console.write_line("build: console only");
    builder.freestanding = false;
}}
"#,
            target = profile.target_name(),
        ),
    )
    .expect("write build.omg");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").expect("write main.omg");

    let package = PackageKeyIdentity::from_digest([83; 32]).expect("nonzero package identity");
    let package_inputs = PackageCompilationInputs::new(
        package,
        vec![PackageSourceBinding::new(
            package,
            "console-build",
            project.clone(),
        )],
        Vec::new(),
    )
    .expect("single-package compiler input");
    let checked = compile_to_checked_with_packages_in_build_dir(
        &project.join("main.omg"),
        &unavailable_build_root,
        Some(profile.target_name()),
        package_inputs,
    )
    .expect("console-only build must not attempt to install real filesystem authority");

    let observations = checked
        .build_observation_summary()
        .expect("console-only build publishes observation evidence");
    assert_eq!(observations.ceiling(), BuildObservationClass::Hermetic);
    assert_eq!(observations.realized(), BuildObservationClass::Hermetic);
    assert!(observations.filesystem_operation_attempts().is_empty());
    assert!(observations.staged_output_tree().is_none());
    assert!(!unavailable_build_root.exists());

    let _ = std::fs::remove_dir_all(&project);
}
