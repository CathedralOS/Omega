//! Regression tests for canonical build-observation commitments.

use super::build_observation_commitment;
use omega_build_evaluation::{
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemOperationResult,
    BuildFilesystemRoot, BuildFilesystemScalarOperandValue, BuildObservationSummary,
};
use omega_compiler::compile_to_checked;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_OBSERVATION_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn compiled_observation(
    relative_output: &str,
    mode: i32,
    payload: &str,
) -> BuildObservationSummary {
    let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-observation-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    let build_dir = project.join("build");
    let output = build_dir.join(relative_output);
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data RootedWriter {{ filesystem: FilesystemHost; descriptor: i32; written: i64; result: i32; }}

machine RootedWriter::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
let output: &[u8] in Path = builder.output.resolve("{relative_output}");
self.descriptor = self.filesystem.create(output, {mode});
self.written = self.filesystem.write(self.descriptor, "{payload}");
self.result = self.filesystem.close(self.descriptor);
}}
"#,
            relative_output = relative_output,
            mode = mode,
            payload = payload,
        ),
    )
    .unwrap();
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
    let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
        .unwrap()
        .build_observation_summary()
        .expect("filesystem build publishes observations")
        .clone();
    std::fs::remove_dir_all(project).unwrap();
    summary
}

fn compiled_handle_order_observation(reverse_close_order: bool) -> BuildObservationSummary {
    let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-handle-observation-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    let input = project.join("input.txt");
    std::fs::write(&input, "input\n").unwrap();
    let close_order = if reverse_close_order {
        "self.result = self.filesystem.close(self.second);\n    self.result = self.filesystem.close(self.first);"
    } else {
        "self.result = self.filesystem.close(self.first);\n    self.result = self.filesystem.close(self.second);"
    };
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data HandleOrder {{ filesystem: FilesystemHost; first: i32; second: i32; result: i32; }}

machine HandleOrder::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
let input: &[u8] in Path = builder.source.resolve("input.txt");
self.first = self.filesystem.open(input, 0);
self.second = self.filesystem.open(input, 0);
{close_order}
}}
"#,
        ),
    )
    .unwrap();
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
    let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
        .unwrap()
        .build_observation_summary()
        .expect("filesystem build publishes observations")
        .clone();
    std::fs::remove_dir_all(project).unwrap();
    summary
}

fn compiled_read_observation(input_bytes: &str) -> BuildObservationSummary {
    let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-read-observation-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    let input = project.join("input.txt");
    std::fs::write(&input, input_bytes).unwrap();
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data RootedReader {{ filesystem: FilesystemHost; descriptor: i32; buffer: [u8; 6]; read: i64; result: i32; }}

machine RootedReader::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
let input: &[u8] in Path = builder.source.resolve("input.txt");
self.descriptor = self.filesystem.open(input, 0);
self.read = self.filesystem.read(self.descriptor, &mut self.buffer, 6);
self.result = self.filesystem.close(self.descriptor);
}}
"#,
        ),
    )
    .unwrap();
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
    let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
        .unwrap()
        .build_observation_summary()
        .expect("filesystem build publishes observations")
        .clone();
    std::fs::remove_dir_all(project).unwrap();
    summary
}

fn compiled_path_like_observation(target: &str) -> BuildObservationSummary {
    let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-path-like-observation-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target windows_x64 {{}}

data SymlinkProbe {{ filesystem: FilesystemHost; result: i32; }}

machine SymlinkProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{{
let link: &[u8] in Path = builder.output.resolve("missing-parent/link");
self.result = self.filesystem.symlink("{target}", link);
}}
"#,
        ),
    )
    .unwrap();
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
    let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
        .unwrap()
        .build_observation_summary()
        .expect("filesystem build publishes observations")
        .clone();
    std::fs::remove_dir_all(project).unwrap();
    summary
}

#[cfg(unix)]
fn compiled_read_link_observation(target: &str) -> BuildObservationSummary {
    use std::os::unix::fs::symlink;

    let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-read-link-observation-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    symlink(target, project.join("fixture-link")).unwrap();
    std::fs::write(
        project.join("build.omg"),
        r#"use omega::language::std::filesystem_host;

target windows_x64 {}

data ReadLinkProbe { filesystem: FilesystemHost; buffer: [u8; 32]; result: i64; }

machine ReadLinkProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{
let link: &[u8] in Path = builder.source.resolve("fixture-link");
self.result = self.filesystem.read_link(link, &mut self.buffer, 32);
}
"#,
    )
    .unwrap();
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n").unwrap();
    let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
        .unwrap()
        .build_observation_summary()
        .expect("read_link build publishes observations")
        .clone();
    std::fs::remove_dir_all(project).unwrap();
    summary
}

fn compiled_metadata_observation(main_source: &str) -> BuildObservationSummary {
    let sequence = NEXT_OBSERVATION_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-metadata-observation-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).unwrap();
    std::fs::write(
        project.join("build.omg"),
        r#"use omega::language::std::filesystem_host;

target windows_x64 {}

data MetadataProbe { filesystem: FilesystemHost; buffer: [u8; 144]; result: i32; }

machine MetadataProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{
let input: &[u8] in Path = builder.source.resolve("main.omg");
self.result = self.filesystem.read_metadata(input, &mut self.buffer);
}
"#,
    )
    .unwrap();
    std::fs::write(project.join("main.omg"), main_source).unwrap();
    let summary = compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
        .unwrap()
        .build_observation_summary()
        .expect("metadata build publishes observations")
        .clone();
    std::fs::remove_dir_all(project).unwrap();
    summary
}

#[test]
fn rooted_observation_commitment_is_relocation_stable_and_path_sensitive() {
    let first = compiled_observation("stage/artifact.bin", 438, "payload-a");
    let relocated = compiled_observation("stage/artifact.bin", 438, "payload-a");
    assert_eq!(first, relocated);
    assert_eq!(
        build_observation_commitment(&first),
        build_observation_commitment(&relocated)
    );

    let changed = compiled_observation("stage/changed.bin", 438, "payload-a");
    assert_ne!(first, changed);
    assert_ne!(
        build_observation_commitment(&first),
        build_observation_commitment(&changed)
    );
    let scalar_changed = compiled_observation("stage/artifact.bin", 420, "payload-a");
    assert_ne!(
        build_observation_commitment(&first),
        build_observation_commitment(&scalar_changed),
        "one changed scalar operand changes observation identity"
    );
    let bytes_changed = compiled_observation("stage/artifact.bin", 438, "payload-b");
    assert_ne!(
        build_observation_commitment(&first),
        build_observation_commitment(&bytes_changed),
        "one changed immutable byte operand changes observation identity"
    );
    assert_eq!(first.schema_version(), 33);
    assert_eq!(first.filesystem_operation_schema_version(), 19);
    assert!(first.staged_output_tree().is_none());
    assert!(relocated.staged_output_tree().is_none());
    assert!(bytes_changed.staged_output_tree().is_none());
    let [create, write, close] = first.filesystem_operation_attempts() else {
        panic!("fixture performs create, write, and close")
    };
    let [path] = create.authorized_paths() else {
        panic!("create retains one rooted output path")
    };
    let [resolution] = create.rooted_path_operand_resolutions() else {
        panic!("create retains one rooted input resolution")
    };
    assert_eq!(resolution.operand_ordinal(), 0);
    assert_eq!(resolution.root(), BuildFilesystemRoot::Output);
    assert_eq!(resolution.relative_path(), b"stage/artifact.bin");
    assert_eq!(path.root(), BuildFilesystemRoot::Output);
    assert_eq!(path.relative_path(), b"stage/artifact.bin");
    let created_identity = create
        .logical_handle_output()
        .expect("successful create retains its logical descriptor")
        .identity();
    assert_eq!(
        create.result(),
        BuildFilesystemOperationResult::LogicalHandle(created_identity)
    );
    assert_eq!(write.result(), BuildFilesystemOperationResult::Scalar(9));
    assert_eq!(
        create.scalar_operands()[0].value(),
        BuildFilesystemScalarOperandValue::I32(438)
    );
    assert_eq!(write.byte_operands()[0].bytes(), b"payload-a");
    assert!(close.authorized_paths().is_empty());
}

#[cfg(unix)]
#[test]
fn observation_commitment_binds_exact_returned_path_bytes() {
    let alpha = compiled_read_link_observation("alpha");
    let bravo = compiled_read_link_observation("bravo");
    let [alpha_attempt] = alpha.filesystem_operation_attempts() else {
        panic!("fixture performs one read_link")
    };
    let [bravo_attempt] = bravo.filesystem_operation_attempts() else {
        panic!("fixture performs one read_link")
    };
    assert_eq!(alpha_attempt.returned_paths()[0].bytes(), b"alpha");
    assert_eq!(bravo_attempt.returned_paths()[0].bytes(), b"bravo");
    assert_ne!(
        build_observation_commitment(&alpha),
        build_observation_commitment(&bravo),
        "changed returned-path bytes change observation identity"
    );
}

#[test]
fn observation_commitment_binds_logical_handle_lifetimes() {
    let forward = compiled_handle_order_observation(false);
    let reverse = compiled_handle_order_observation(true);
    let without_handles = |summary: &BuildObservationSummary| {
        summary
            .filesystem_operation_attempts()
            .iter()
            .map(|attempt| {
                (
                    attempt.operation_tag(),
                    attempt.provider(),
                    attempt.result(),
                    attempt.post_error(),
                    attempt.authorized_paths().to_vec(),
                    attempt.grant_refusals().to_vec(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        without_handles(&forward),
        without_handles(&reverse),
        "the fixture pair differs only in logical descriptor use"
    );
    assert_ne!(forward, reverse);
    assert_ne!(
        build_observation_commitment(&forward),
        build_observation_commitment(&reverse),
        "package review must bind which live descriptor each close consumed"
    );

    let [_, _, forward_first_close, forward_second_close] = forward.filesystem_operation_attempts()
    else {
        panic!("forward fixture performs two opens and two closes")
    };
    let [first_input] = forward_first_close.logical_handle_inputs() else {
        panic!("first close retains one logical descriptor")
    };
    let [second_input] = forward_second_close.logical_handle_inputs() else {
        panic!("second close retains one logical descriptor")
    };
    assert_eq!(
        first_input.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Resolved(
            forward_first_close.retired_logical_handles()[0]
        )
    );
    assert_eq!(
        second_input.resolution(),
        BuildFilesystemLogicalHandleInputResolution::Resolved(
            forward_second_close.retired_logical_handles()[0]
        )
    );
    assert_ne!(first_input.resolution(), second_input.resolution());
}

#[test]
fn observation_commitment_binds_mutable_pre_and_post_state() {
    let first = compiled_read_observation("alpha\n");
    let changed = compiled_read_observation("bravo\n");
    let first_read = &first.filesystem_operation_attempts()[1];
    let changed_read = &changed.filesystem_operation_attempts()[1];
    assert_eq!(
        first_read.mutable_byte_operand_resolutions()[0].bytes(),
        &[0; 6]
    );
    assert_eq!(
        changed_read.mutable_byte_operand_resolutions()[0].bytes(),
        &[0; 6]
    );
    assert_eq!(first_read.mutable_byte_operands()[0].pre_bytes(), &[0; 6]);
    assert_eq!(changed_read.mutable_byte_operands()[0].pre_bytes(), &[0; 6]);
    assert_eq!(
        first_read.mutable_byte_operands()[0].post_bytes(),
        b"alpha\n"
    );
    assert_eq!(
        changed_read.mutable_byte_operands()[0].post_bytes(),
        b"bravo\n"
    );
    let [first_region] = first_read.observed_byte_regions() else {
        panic!("successful read retains one semantic observed-byte region")
    };
    let [changed_region] = changed_read.observed_byte_regions() else {
        panic!("changed read retains one semantic observed-byte region")
    };
    assert_eq!(first_region, changed_region);
    assert_eq!(
        first_read.observed_bytes(first_region),
        Some(b"alpha\n".as_slice())
    );
    assert_eq!(
        changed_read.observed_bytes(changed_region),
        Some(b"bravo\n".as_slice())
    );
    assert_ne!(
        build_observation_commitment(&first),
        build_observation_commitment(&changed),
        "one changed observed file-content region changes observation identity"
    );
}

#[test]
fn observation_commitment_binds_canonical_metadata() {
    let short = compiled_metadata_observation("const VALUE: u8 = 1;\n");
    let long = compiled_metadata_observation("const VALUE: u64 = 123456789;\n");
    let [short_attempt] = short.filesystem_operation_attempts() else {
        panic!("metadata fixture performs one stat")
    };
    let [long_attempt] = long.filesystem_operation_attempts() else {
        panic!("metadata fixture performs one stat")
    };
    let [short_metadata] = short_attempt.metadata_observations() else {
        panic!("successful short stat retains canonical metadata")
    };
    let [long_metadata] = long_attempt.metadata_observations() else {
        panic!("successful long stat retains canonical metadata")
    };
    assert_ne!(short_metadata.size(), long_metadata.size());
    assert_ne!(
        build_observation_commitment(&short),
        build_observation_commitment(&long),
        "changed canonical metadata changes package observation identity"
    );
}

#[test]
fn observation_commitment_binds_path_like_operands() {
    let first = compiled_path_like_observation("missing-alpha");
    let changed = compiled_path_like_observation("missing-bravo");
    let [first_symlink] = first.filesystem_operation_attempts() else {
        panic!("fixture performs one symlink operation")
    };
    let [changed_symlink] = changed.filesystem_operation_attempts() else {
        panic!("fixture performs one symlink operation")
    };
    assert_eq!(first_symlink.result(), changed_symlink.result());
    assert_eq!(first_symlink.post_error(), changed_symlink.post_error());
    assert_eq!(
        first_symlink.authorized_paths(),
        changed_symlink.authorized_paths()
    );
    assert_eq!(
        first_symlink.grant_refusals(),
        changed_symlink.grant_refusals()
    );
    assert_eq!(
        first_symlink.path_like_operands()[0].bytes(),
        b"missing-alpha"
    );
    assert_eq!(
        changed_symlink.path_like_operands()[0].bytes(),
        b"missing-bravo"
    );
    assert_ne!(
        build_observation_commitment(&first),
        build_observation_commitment(&changed),
        "one changed path-like operand changes observation identity"
    );
}
