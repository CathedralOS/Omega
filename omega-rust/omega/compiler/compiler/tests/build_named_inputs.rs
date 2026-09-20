//! Named build slots are caller-supplied immutable capabilities, not paths.

use compiler::{CheckedCompileRequest, compile_to_checked};

#[test]
fn missing_named_input_fails_before_a_build_can_publish() {
    let root =
        std::env::temp_dir().join(format!("omega-missing-build-input-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(
        root.join("main.omg"),
        "data Main {}\nmachine Main::main(&mut self) {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: &mut Build) {
        builder.application("missing-input");
        builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
        let input: BuildSource = builder.inputs.get("template");
    }
"#,
    )
    .unwrap();
    let result = compile_to_checked(CheckedCompileRequest::new(
        &root.join("main.omg"),
        Some("linux_x86_64"),
    ));
    std::fs::remove_dir_all(root).unwrap();
    let diagnostics = result.expect_err("a missing input cannot manufacture a capability");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("named build input")
                && diagnostic.message.contains("not assigned")
        ),
        "{diagnostics:?}"
    );
}

#[cfg(unix)]
fn input_permissions(root: &std::path::Path, sealed: bool) {
    use std::os::unix::fs::PermissionsExt;
    if !sealed {
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    for entry in std::fs::read_dir(root).unwrap() {
        std::fs::set_permissions(
            entry.unwrap().path(),
            std::fs::Permissions::from_mode(if sealed { 0o444 } else { 0o644 }),
        )
        .unwrap();
    }
    if sealed {
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
}
#[cfg(not(unix))]
fn input_permissions(_root: &std::path::Path, _sealed: bool) {}

#[test]
fn captured_named_input_reaches_ordinary_completed_file_publication() {
    use package_compilation::{
        BuildSourceCaptureObligation, BuildSourceCaptureRequest, capture_scoped_source_input,
    };
    let root =
        std::env::temp_dir().join(format!("omega-captured-build-input-{}", std::process::id()));
    let source = root.join("source");
    let extra = root.join("extra");
    let publication = root.join("published");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir(&extra).unwrap();
    std::fs::write(extra.join("data.txt"), b"captured\n").unwrap();
    input_permissions(&extra, true);
    let captured = capture_scoped_source_input(
        &extra,
        &BuildSourceCaptureRequest::new([(
            b"data.txt".to_vec(),
            BuildSourceCaptureObligation::Required,
        )])
        .unwrap(),
        Vec::<std::path::PathBuf>::new(),
    )
    .unwrap();
    let invalid_name = vec![0xff];
    let error = build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        .with_inputs([(invalid_name.clone(), captured.clone())])
        .expect_err("non-UTF8 local slots must reject without panicking");
    assert!(!error.is_empty());
    let occurrence = package_compilation::BuildDependencyOccurrence::new(
        semantic_vocabulary::PackageKeyIdentity::from_digest([31; 32]).unwrap(),
        build_declarations::DependencyPurpose::Build,
        "dependency",
        semantic_vocabulary::PackageKeyIdentity::from_digest([32; 32]).unwrap(),
    );
    let error = build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
        .with_dependency_inputs([(occurrence, invalid_name, captured.clone())])
        .expect_err("non-UTF8 dependency slots must reject without panicking");
    assert!(!error.is_empty());
    input_permissions(&extra, false);
    std::fs::write(extra.join("data.txt"), b"changed!\n").unwrap();
    std::fs::write(source.join("main.omg"), "data Main {}\n").unwrap();
    std::fs::write(
        source.join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.application("captured-input");
    builder.artifact_only();
    let input: BuildSource = builder.inputs.get("template");
    let input_path: BuildPath = input.resolve("data.txt");
    let reader: i32 = input.open(input_path, 0);
    let mut bytes: [u8; 9];
    let count: i64 = input.read(reader, &mut bytes, 9);
    let reader_closed: i32 = input.close(reader);
    let required: RequiredOutput = builder.output.require("report.txt");
    let output_path: BuildPath = builder.output.resolve("report.txt");
    let writer: i32 = builder.output.create(output_path, 438);
    let written: i64 = builder.output.write(writer, &bytes);
    let writer_closed: i32 = builder.output.close(writer);
    let completion: OutputCompletion = builder.output.complete(required, output_path);
}
"#,
    )
    .unwrap();
    let snapshot = build_evaluation::BuildSnapshotRequest::scoped(
        std::iter::empty::<Vec<u8>>(),
        BuildSourceCaptureRequest::new([
            (b"main.omg".to_vec(), BuildSourceCaptureObligation::Required),
            (
                b"build.omg".to_vec(),
                BuildSourceCaptureObligation::Required,
            ),
        ])
        .unwrap(),
    )
    .with_inputs([(b"template".to_vec(), captured)])
    .unwrap();
    input_permissions(&source, true);
    let result = compiler::compile(
        compiler::CompileRequest::new(compiler::CompileOptions {
            root_path: source.join("main.omg"),
            build_dir: Some(publication.clone()),
            target_name: Some("linux_x86_64".to_owned()),
        })
        .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
        .with_build_snapshot(snapshot),
    )
    .and_then(compiler::CompileOutcomes::into_single_report);
    input_permissions(&source, false);
    let report = result.expect("captured input must reach ordinary artifact-only compilation");
    assert_eq!(
        report.output_kind(),
        compiler::CompileOutputKind::BuildArtifacts
    );
    assert!(!publication.join("completed").exists());
    let report = report
        .publish_completed_build_outputs(&publication)
        .unwrap();
    let outputs = report.build_outputs().unwrap();
    let directory = outputs.published_directory().unwrap();
    assert_eq!(
        std::fs::read(directory.join("files/report.txt")).unwrap(),
        b"captured\n"
    );
    assert_eq!(
        std::fs::read(directory.join("manifest.bin")).unwrap(),
        outputs.manifest_bytes()
    );
    assert_eq!(
        std::fs::read_dir(directory.join("files")).unwrap().count(),
        1
    );
    assert!(report.has_consistent_executable_publication_custody());
    std::fs::remove_dir_all(root).unwrap();
}
