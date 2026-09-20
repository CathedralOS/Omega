//! Ordinary package commands carry settled build files through final publication.
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);
impl Project {
    fn new(artifact_only: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-completed-output-cli-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("main.omg"),
            "data Main {}\nmachine Main::main(&mut self) {}\n",
        )
        .unwrap();
        let selection = if artifact_only {
            "builder.artifact_only();"
        } else {
            r#"
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
"#
        };
        fs::write(
            root.join("build.omg"),
            BUILD.replace("SELECTION", selection),
        )
        .unwrap();
        Self(root)
    }
    fn omega(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }
    fn completed_set(&self) -> PathBuf {
        let entries = fs::read_dir(self.0.join("build/completed"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        entries[0].clone()
    }

    fn accept_build(&self, target: &str) {
        self.accept_build_with_inputs(target, &[]);
    }

    fn accept_build_with_inputs(&self, target: &str, inputs: &[&str]) {
        let mut arguments = vec!["update", "--offline", "--target", target];
        arguments.extend_from_slice(inputs);
        let output = self.omega(&arguments);
        if output.status.success() {
            return;
        }
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let path = self
            .0
            .join(format!("build/package-manager/review-{target}.txt"));
        let review = fs::read_to_string(&path).unwrap();
        let mut decisions = 0;
        let accepted = review
            .lines()
            .map(|line| {
                if line.starts_with("decision ") {
                    decisions += 1;
                    format!("{} accept\n", line.strip_suffix(" pending").unwrap())
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        assert!(
            decisions > 0,
            "the fixture explicitly accepts its build's review rows"
        );
        fs::write(path, accepted).unwrap();
        success(self.omega(&["update", "--resume", "--offline"]));
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("completed-outputs");
    SELECTION
    let scratch: BuildPath = builder.output.resolve("scratch.txt");
    let temporary: i32 = builder.output.create(scratch, 438);
    let ignored: i64 = builder.output.write(temporary, "private scratch");
    let released: i32 = builder.output.close(temporary);
    let required: RequiredOutput = builder.output.require("report.txt");
    let artifact: BuildPath = builder.output.resolve("report.txt");
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "report\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#;

#[test]
fn package_artifact_only_and_executable_companions_publish_exact_completed_files() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        eprintln!("skipping executable companion: unsupported host");
        return;
    };
    for artifact_only in [true, false] {
        let project = Project::new(artifact_only);
        project.accept_build(host.target_name());
        let lock = fs::read(project.0.join("omega.lock")).unwrap();
        if artifact_only {
            let rejected = project.omega(&[
                "--offline",
                "--target",
                host.target_name(),
                "--disable-optimization",
                "ControlFlowCleanup",
                "main.omg",
            ]);
            assert!(!rejected.status.success());
            assert!(
                String::from_utf8_lossy(&rejected.stderr)
                    .contains("no product optimization stages"),
                "{rejected:?}"
            );
            assert!(!project.0.join("build/completed").exists());
        }
        success(project.omega(&[
            "--check",
            "--offline",
            "--target",
            host.target_name(),
            "main.omg",
        ]));
        assert!(
            !project.0.join("build/completed").exists(),
            "checking does not publish files"
        );
        success(project.omega(&["--offline", "--target", host.target_name(), "main.omg"]));
        let set = project.completed_set();
        assert_eq!(fs::read(set.join("files/report.txt")).unwrap(), b"report\n");
        assert!(!set.join("files/scratch.txt").exists());
        assert!(set.join("manifest.bin").is_file());
        assert_eq!(fs::read(project.0.join("omega.lock")).unwrap(), lock);
        if !artifact_only {
            let executable = project.0.join("build").join(if cfg!(windows) {
                "omega-program.exe"
            } else {
                "omega-program"
            });
            success(Command::new(executable).output().unwrap());
        }
    }
}

#[test]
fn failure_after_completing_a_file_publishes_no_output_set() {
    let project = Project::new(true);
    let failing = BUILD.replace("SELECTION", "builder.artifact_only();").replace(
        "let completion: OutputCompletion = builder.output.complete(required, artifact);",
        "let completion: OutputCompletion = builder.output.complete(required, artifact);\n    let pending: RequiredOutput = builder.output.require(\"failure.txt\");\n    builder.output.fail(pending, \"later failure\");"
    );
    fs::write(project.0.join("build.omg"), failing).unwrap();
    let output = project.omega(&["--offline", "--target", "linux_x86_64", "main.omg"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("later failure"),
        "{output:?}"
    );
    assert!(!project.0.join("build/completed").exists());
}

/// The helper is acquired as a build-purpose package. It receives only the
/// consumer's snapshot and output facets; neither a host path nor the whole
/// builder is passed to the downloaded generator.
const ACQUIRED_GENERATOR: &str = r#"pub machine generate(source: &BuildSource, output: &mut BuildOutput) {
    let template: BuildPath = source.resolve("templates/banner.txt");
    let descriptor: i32 = source.open(template, 0);
    let mut bytes: [u8; 7];
    let count: i64 = source.read(descriptor, &mut bytes, 7);
    let closed: i32 = source.close(descriptor);
    transition count == 7 {
        true -> probe(source, output, bytes)
        _ -> failed(output)
    }
    state probe(source: &BuildSource, output: &mut BuildOutput, bytes: [u8; 7]) {
        let outside: BuildPath = source.resolve("undeclared.txt");
        let descriptor: i32 = source.open(outside, 0);
        transition descriptor < 0 {
            true -> publish(output, bytes)
            _ -> failed(output)
        }
    }
    state publish(output: &mut BuildOutput, bytes: [u8; 7]) {
        let required: RequiredOutput = output.require("report.txt");
        let artifact: BuildPath = output.resolve("report.txt");
        let descriptor: i32 = output.create(artifact, 438);
        let written: i64 = output.write(descriptor, &bytes);
        let closed: i32 = output.close(descriptor);
        let completion: OutputCompletion = output.complete(required, artifact);
    }
    state failed(output: &mut BuildOutput) {
        let required: RequiredOutput = output.require("report.txt");
        output.fail(required, "generator snapshot invalid");
    }
}
"#;

const GENERATOR_INPUTS: &[&str] = &[
    "--build-input",
    "main.omg",
    "--build-input",
    "build.omg",
    "--build-input",
    "templates/banner.txt",
];

fn completed_directories(directory: &std::path::Path) -> Vec<PathBuf> {
    let mut paths = if directory.join("completed").exists() {
        fs::read_dir(directory.join("completed"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    paths.sort();
    paths
}

fn completed_contents(directory: &std::path::Path) -> Vec<(PathBuf, Vec<u8>, Vec<u8>)> {
    completed_directories(directory)
        .into_iter()
        .map(|path| {
            let report = fs::read(path.join("files/report.txt")).unwrap();
            let manifest = fs::read(path.join("manifest.bin")).unwrap();
            (path, report, manifest)
        })
        .collect()
}

#[test]
fn acquired_generator_publishes_occurrence_local_files_for_artifact_and_native_products() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        eprintln!("skipping acquired generator native execution: unsupported host");
        return;
    };
    let workspace = Project::new(true);
    let generator = workspace.0.join("generator");
    fs::create_dir(&generator).unwrap();
    fs::write(generator.join("main.omg"), ACQUIRED_GENERATOR).unwrap();
    fs::write(
        generator.join("build.omg"),
        "machine build(builder: &mut Build) { builder.package(\"banner-generator\"); }\n",
    )
    .unwrap();
    let publication = workspace.0.join("published");
    let publication_argument = publication.to_str().unwrap();
    let mut previous: Vec<PathBuf> = Vec::new();
    for (name, content, artifact_only) in
        [("first", "FIRST!\n", true), ("second", "SECOND\n", false)]
    {
        let project = Project(workspace.0.join(name));
        fs::create_dir(&project.0).unwrap();
        fs::create_dir(project.0.join("templates")).unwrap();
        fs::write(
            project.0.join("main.omg"),
            "data Main {}\nmachine Main::main(&mut self) {}\n",
        )
        .unwrap();
        fs::write(project.0.join("templates/banner.txt"), content).unwrap();
        fs::write(project.0.join("undeclared.txt"), "private sibling").unwrap();
        let selection = if artifact_only {
            "builder.artifact_only();"
        } else {
            "builder.roots.bind(macos_arm64::ProgramEntry, Main::main);\n\
             builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n\
             builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);\n\
             builder.roots.bind(linux_arm64::ProgramEntry, Main::main);"
        };
        let build = format!(
            r#"use generator::main;
machine build(builder: &mut Build) {{
    builder.application("{name}");
    builder.build_depend_as("generator", Source::Path {{ location: "../generator" }});
    {selection}
    generate(&builder.source, &mut builder.output);
}}
"#
        );
        fs::write(project.0.join("build.omg"), &build).unwrap();
        project.accept_build_with_inputs(host.target_name(), GENERATOR_INPUTS);
        let lock = fs::read(project.0.join("omega.lock")).unwrap();
        let mut arguments = vec![
            "--offline",
            "--target",
            host.target_name(),
            "--build-dir",
            publication_argument,
            "main.omg",
        ];
        arguments.extend_from_slice(GENERATOR_INPUTS);
        let mut checking = arguments.clone();
        checking.push("--check");
        success(project.omega(&checking));
        assert_eq!(completed_directories(&publication), previous);
        success(project.omega(&arguments));
        let current = completed_directories(&publication);
        let added = current
            .iter()
            .filter(|path| !previous.contains(*path))
            .collect::<Vec<_>>();
        assert_eq!(
            added.len(),
            1,
            "each occurrence publishes its own complete set"
        );
        assert_eq!(
            fs::read(added[0].join("files/report.txt")).unwrap(),
            content.as_bytes()
        );
        assert!(added[0].join("manifest.bin").is_file());
        assert_eq!(fs::read_dir(added[0].join("files")).unwrap().count(), 1);
        if !artifact_only {
            let executable = publication.join(if cfg!(windows) {
                "omega-program.exe"
            } else {
                "omega-program"
            });
            success(Command::new(executable).output().unwrap());
        }
        let committed = completed_contents(&publication);
        // Repeating the same request verifies the existing set, never borrows
        // another occurrence's report.txt or creates another successful set.
        success(project.omega(&arguments));
        assert_eq!(completed_contents(&publication), committed);
        assert_eq!(fs::read(project.0.join("omega.lock")).unwrap(), lock);
        for earlier in &previous {
            assert_eq!(
                fs::read(earlier.join("files/report.txt")).unwrap(),
                b"FIRST!\n"
            );
        }
        // The helper completes its file before the caller fails. Neither that
        // partial attempt nor its retry may alter a previously committed set.
        let failing = build.replace("generate(&builder.source, &mut builder.output);",
            "generate(&builder.source, &mut builder.output);\n    let pending: RequiredOutput = builder.output.require(\"later.txt\");\n    builder.output.fail(pending, \"after generator\");");
        fs::write(project.0.join("build.omg"), failing).unwrap();
        let failed = project.omega(&arguments);
        assert!(!failed.status.success());
        assert!(
            String::from_utf8_lossy(&failed.stderr).contains("after generator"),
            "{failed:?}"
        );
        assert_eq!(completed_contents(&publication), committed);
        fs::write(project.0.join("build.omg"), &build).unwrap();
        success(project.omega(&arguments));
        assert_eq!(completed_contents(&publication), committed);
        assert_eq!(fs::read(project.0.join("omega.lock")).unwrap(), lock);
        previous = current;
        if artifact_only {
            // The same package and logical output name under another product
            // target must retain its own activation identity even when its
            // generated bytes happen to match.
            let other = if host == target::TargetProfile::WindowsX64 {
                target::TargetProfile::LinuxX64
            } else {
                target::TargetProfile::WindowsX64
            };
            project.accept_build_with_inputs(other.target_name(), GENERATOR_INPUTS);
            let accepted = fs::read(project.0.join("omega.lock")).unwrap();
            let before = completed_contents(&publication);
            arguments[2] = other.target_name();
            success(project.omega(&arguments));
            let current = completed_directories(&publication);
            let added = current
                .iter()
                .filter(|path| !previous.contains(*path))
                .collect::<Vec<_>>();
            assert_eq!(
                added.len(),
                1,
                "another target owns a distinct completed set"
            );
            assert_eq!(
                fs::read(added[0].join("files/report.txt")).unwrap(),
                content.as_bytes()
            );
            for retained in before {
                assert!(completed_contents(&publication).contains(&retained));
            }
            assert_eq!(fs::read(project.0.join("omega.lock")).unwrap(), accepted);
            previous = current;
        }
    }
    assert_eq!(previous.len(), 3);
}
