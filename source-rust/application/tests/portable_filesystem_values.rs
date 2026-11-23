//! Native count fields and filename encodings must fit the portable API while
//! preserving valid buffers and complete names. Check the actual adapter
//! operations, then the selected filesystem service on this host.

mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

struct Project(TemporaryDirectory);

impl Project {
    fn new(name: &str) -> Self {
        Self(TemporaryDirectory::create(&format!("filesystem-{name}"), false).unwrap())
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn observe(&self, arguments: &[&str], build: impl AsRef<Path>) -> Output {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(arguments)
            .arg(build.as_ref())
            .current_dir(self.0.path())
            .env(
                "OMEGA_LIBRARY_ROOT",
                support::repository_root().join("source/library"),
            )
            .stdin(Stdio::null())
            .output()
            .expect("run the filesystem program")
    }
}

fn report(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn both_zero(output: Output) {
    let text = report(&output);
    assert!(output.status.success(), "{text}");
    for receipt in ["native exit 0", "interp exit: 0"] {
        assert!(text.lines().any(|line| line == receipt), "{text}");
    }
}

// An adapter edit must change the code this test runs.
fn declaration(source: &str, heading: &str) -> String {
    let start = source.find(heading).unwrap();
    let body = start + source[start..].find('{').unwrap();
    let mut depth = 0;
    for (offset, byte) in source[body..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return source[start..=body + offset].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("unterminated declaration {heading}")
}

// A declaration's file determines which siblings it can call and what its
// import aliases mean. Keep each operation at its standard-library filename.
fn stage_adapter_operations(project: &Project, library: &Path) {
    let files = [
        (
            "filesystem.omg",
            &[
                "public data ErrorKind ",
                "public data DirEntry ",
                "public data DirEntryResult ",
            ][..],
        ),
        (
            "windows_filesystem_host.omg",
            &[
                "public data WindowsDirectoryEntry ",
                "public data WindowsDirectoryEntryResult ",
            ][..],
        ),
        (
            "targets/windows_x86_64/filesystem/handles.omg",
            &["machine windows_filesystem_transfer_size("][..],
        ),
        (
            "targets/windows_x86_64/filesystem/metadata.omg",
            &[
                "data WindowsTimestampEncoding ",
                "machine windows_filesystem_encode_timestamp(",
            ][..],
        ),
        (
            "targets/windows_x86_64/windows_filesystem_directory_entry.omg",
            &[
                "machine windows_directory_copy_entry(",
                "machine windows_directory_copy_name(",
            ][..],
        ),
        (
            "targets/windows_x86_64/filesystem/directory.omg",
            &["machine windows_filesystem_directory_entry("][..],
        ),
    ];
    for (file, headings) in files {
        let source = std::fs::read_to_string(library.join(file)).unwrap();
        let mut staged = String::new();
        for line in source.lines() {
            let Some(import) = line.strip_prefix("use ") else {
                continue;
            };
            let path = import.trim_end_matches(';').split_whitespace().next();
            if files.iter().any(|(name, _)| Some(*name) == path) {
                staged.push_str(line);
                staged.push('\n');
            }
        }
        for heading in headings {
            staged.push_str(&declaration(&source, heading));
            staged.push('\n');
        }
        project.write(file, &staged);
    }
}

fn build(dependency: bool) -> String {
    let std = support::repository_root().join("source/library/std");
    let dependency = if dependency {
        format!(
            "let std_registration: DependencyRegistration = Build::dependency(builder, dependency\\from_source(Source::Path {{ location: \"{}\" }}));",
            std.to_string_lossy().replace('\\', "/")
        )
    } else {
        String::new()
    };
    format!(
        "machine build(builder: @mutable Build) {{\nBuild::application(builder, \"portable_filesystem_values\");\n{dependency}\nBuild::bind(builder, windows_x86_64\\ProgramEntry, main);\nBuild::bind(builder, linux_x86_64\\ProgramEntry, main);\nBuild::bind(builder, linux_arm64\\ProgramEntry, main);\nBuild::bind(builder, macos_arm64\\ProgramEntry, main);\n}}\n"
    )
}

#[test]
fn windows_adapter_values_preserve_counts_timestamps_and_whole_names() {
    let project = Project::new("values");
    let library = support::repository_root().join("source/library/std");
    let main = include_str!("fixtures/filesystem/run/windows_adapter_values/main.omg");
    project.write("main.omg", main);
    stage_adapter_operations(&project, &library);
    project.write("build.omg", &build(false));
    both_zero(project.observe(&["run", "--both"], "build.omg"));

    // Only the requested extent changes: the same copy must refuse byte 766.
    let invalid = main.replacen(
        "windows_directory_copy_entry(@source, 765, 16)",
        "windows_directory_copy_entry(@source, 766, 16)",
        1,
    );
    assert_ne!(invalid, main);
    project.write("main.omg", &invalid);
    let output = project.observe(&["--check"], "build.omg");
    let text = report(&output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.contains("windows_directory_copy_entry") && text.contains("766 <= 765"),
        "{text}"
    );
}

#[test]
fn portable_filesystem_public_calls_preserve_neighboring_bytes() {
    let project = Project::new("public");
    project.write(
        "main.omg",
        include_str!("fixtures/filesystem/run/public_values/main.omg"),
    );
    project.write("build.omg", &build(true));
    both_zero(project.observe(
        &[
            "run",
            "--both",
            "--build-input",
            "main.omg",
            "--build-input",
            "build.omg",
        ],
        "build.omg",
    ));
}

// This rule checks POSIX mode bits and consumes its input directory.
#[cfg(unix)]
#[test]
fn portable_filesystem_rules_hold_on_each_engine() {
    let build = support::repository_root()
        .join("source-rust/application/tests/fixtures/filesystem/run/portable_rules/build.omg");
    for (engine, expected) in [("native", "native exit 42"), ("psi", "Psi exit 42")] {
        let project = Project::new(engine);
        let wide = project.0.path().join("omega_fs_rules_wide");
        std::fs::create_dir(&wide).unwrap();
        for index in 0..4200 {
            std::fs::File::create(wide.join(index.to_string())).unwrap();
        }

        let output = project.observe(&["run", "--engine", engine], &build);
        let text = report(&output);
        assert_eq!(output.status.code(), Some(42), "{engine}: {text}");
        assert!(
            text.lines().any(|line| line == expected),
            "{engine}: {text}"
        );
        for tree in [wide, project.0.path().join("omega_fs_rules")] {
            assert!(
                !tree.try_exists().unwrap(),
                "{engine} left {}",
                tree.display()
            );
        }
    }
}
