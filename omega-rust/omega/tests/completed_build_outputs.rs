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
        let output = self.omega(&["update", "--offline", "--target", target]);
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
