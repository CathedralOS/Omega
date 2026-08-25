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
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildObservationClass, CompileOptions, FilesystemSponsor, PackageCompilationInputs,
    PackageSourceBinding, compile, compile_to_checked,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir,
};
use psi_core::PackageKeyIdentity;
use std::path::PathBuf;
use std::process::Command;

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    }
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

data Subsystem {{ case Console; case Gui; case EfiApplication; case Unspecified(value: u16); }}
data Build {{ subsystem: Subsystem; freestanding: bool; }}

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
    self.fd = self.fs.open("{source}/inputs/table.txt", 0);
    self.n = self.fs.read(self.fd, &mut self.buffer, 6);
    self.handle = self.fs.get_osfhandle(self.fd);
    self.rc = self.fs.close(self.fd);
    self.fd = self.fs.create("{stage}/asset.tmp", 438);
    transition self.fd >= 0 {{ true -> put(builder) _ -> done(builder) }}
    state put(&mut self, builder: &mut Build) {{
        self.clone_fd = self.fs.duplicate(self.fd);
        self.n = self.fs.write(self.clone_fd, "staged by build\n");
        _ = self.fs.sync(self.clone_fd);
        self.n = self.fs.close(self.clone_fd);
        self.n = self.fs.close(self.fd);
        self.rc = self.fs.rename("{stage}/asset.tmp", "{stage}/asset.bin");
        transition true {{ true -> done(builder) _ -> done(builder) }}
    }}
    state done(&mut self, builder: &mut Build) {{
        builder.freestanding = false;
    }}
}}
"#,
            // Forward slashes so the embedded path lexes on windows too
            // (`C:\Users\...` would read `\U` as an escape sequence); every
            // host fs API accepts them.
            stage = stage.display().to_string().replace('\\', "/"),
            source = project.display().to_string().replace('\\', "/"),
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
    assert_eq!(checked_observations.schema_version(), 10);
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
        9
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
    assert_eq!(read_buffer.operand_ordinal(), 1);
    assert_eq!(read_buffer.pre_bytes(), &[0; 6]);
    assert_eq!(read_buffer.post_bytes(), b"table\n");
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
        staged.exists(),
        "the diagnostic must acknowledge that a prior staged side effect occurred"
    );

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
