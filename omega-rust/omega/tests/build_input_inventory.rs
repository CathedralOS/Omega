//! Caller inventories pass through CLI package preparation and both review
//! passes, without becoming a dependency's inventory or changing the checkout.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-cli-build-inputs-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("root")).unwrap();
        fs::create_dir(path.join("root/templates")).unwrap();
        fs::create_dir(path.join("dependency")).unwrap();
        fs::write(
            path.join("root/main.omg"),
            "data Main {}\nmachine Main::main(&mut self) {}\n",
        )
        .unwrap();
        fs::write(path.join("root/build.omg"), BUILD).unwrap();
        fs::write(path.join("root/templates/banner.txt"), "OK").unwrap();
        fs::write(
            path.join("dependency/main.omg"),
            "pub machine helper() {}\n",
        )
        .unwrap();
        fs::write(path.join("dependency/build.omg"), DEPENDENCY_BUILD).unwrap();
        fs::write(path.join("dependency/child-only.txt"), "child").unwrap();
        Self(path)
    }

    fn omega(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(self.0.join("root"))
            .args(arguments)
            .output()
            .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const INVENTORY: &[&str] = &[
    "--build-input",
    "main.omg",
    "--build-input",
    "build.omg",
    "--build-input",
    "templates",
    "--optional-build-input",
    "absent.cfg",
];

const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("cli-build-inputs");
    builder.depend(Source::Path { location: "../dependency" });
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    let path: BuildPath = builder.source.resolve("templates/banner.txt");
    let descriptor: i32 = builder.source.open(path, 0);
    let mut bytes: [u8; 2];
    let count: i64 = builder.source.read(descriptor, &mut bytes, 2);
    let closed: i32 = builder.source.close(descriptor);
    transition count == 2 {
        true -> first(builder, bytes)
        _ -> rejected(builder)
    }
    state first(builder: &mut Build, bytes: [u8; 2]) {
        transition bytes[0] == 79 {
            true -> second(builder, bytes)
            _ -> rejected(builder)
        }
    }
    state second(builder: &mut Build, bytes: [u8; 2]) {
        transition bytes[1] == 75 {
            true -> probe(builder)
            _ -> rejected(builder)
        }
    }
    state probe(builder: &mut Build) {
        let path: BuildPath = builder.source.resolve("undeclared.txt");
        let descriptor: i32 = builder.source.open(path, 0);
        transition descriptor < 0 {
            true -> admitted()
            _ -> rejected(builder)
        }
    }
    state admitted() {}
    state rejected(builder: &mut Build) {
        let output: RequiredOutput = builder.output.require("inventory-verdict");
        builder.output.fail(output, "wrong bytes or undeclared source access");
    }
}
"#;

const DEPENDENCY_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("inventory-child");
    let path: BuildPath = builder.source.resolve("child-only.txt");
    let descriptor: i32 = builder.source.open(path, 0);
    transition descriptor >= 0 {
        true -> admitted(builder, descriptor)
        _ -> rejected(builder)
    }
    state admitted(builder: &mut Build, descriptor: i32) {
        let closed: i32 = builder.source.close(descriptor);
    }
    state rejected(builder: &mut Build) {
        let output: RequiredOutput = builder.output.require("child-verdict");
        builder.output.fail(output, "root inventory replaced child input custody");
    }
}
"#;

#[test]
fn cli_inventory_narrows_root_reads_without_narrowing_dependencies_and_runs_native() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        eprintln!("skipping native inventory execution: unsupported compiler host");
        return;
    };
    let project = Project::new();
    // Accept the ordinary project before adding an undeclared sibling. Local
    // source edits do not authorize changed risk-bearing policy; native
    // production must still pass the exact accepted-package route.
    assert_success(&project.omega(&["update", "--offline", "--target", host.target_name()]));
    fs::write(project.0.join("root/undeclared.txt"), "not a build input").unwrap();
    let accepted = fs::read(project.0.join("root/omega.lock")).unwrap();
    let broader = project.omega(&[
        "--check",
        "--offline",
        "--target",
        host.target_name(),
        "main.omg",
    ]);
    assert!(!broader.status.success());
    assert!(
        String::from_utf8_lossy(&broader.stderr)
            .contains("wrong bytes or undeclared source access"),
        "{broader:?}"
    );

    for product in [Some("--check"), None] {
        let mut arguments = INVENTORY.to_vec();
        arguments.extend(["--offline", "--target", host.target_name(), "main.omg"]);
        if let Some(product) = product {
            arguments.push(product);
        }
        assert_success(&project.omega(&arguments));
    }
    let executable = project.0.join("root/build").join(if cfg!(windows) {
        "omega-program.exe"
    } else {
        "omega-program"
    });
    assert_success(&Command::new(executable).output().unwrap());
    assert_eq!(
        fs::read(project.0.join("root/omega.lock")).unwrap(),
        accepted
    );
    assert_eq!(
        fs::read(project.0.join("root/undeclared.txt")).unwrap(),
        b"not a build input"
    );
    assert!(!project.0.join("root/inventory-verdict").exists());
}

#[test]
fn cli_inventory_rejects_missing_required_inputs_and_omitted_source_before_execution() {
    let project = Project::new();
    for (inventory, expected) in [
        (
            vec![
                "--build-input",
                "main.omg",
                "--build-input",
                "build.omg",
                "--build-input",
                "missing.txt",
            ],
            "required source capture entry `missing.txt` is absent",
        ),
        (
            vec!["--build-input", "build.omg", "--build-input", "templates"],
            "scoped source inventory omits required source member",
        ),
    ] {
        let mut arguments = inventory;
        arguments.extend([
            "--check",
            "--offline",
            "--target",
            "linux_x86_64",
            "main.omg",
        ]);
        let output = project.omega(&arguments);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
        assert!(!project.0.join("root/omega.lock").exists());
        assert!(!project.0.join("root/build/omega-program").exists());
    }
}
