//! The caller selects the first Build by naming a file. Its Product receives a checked entry.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const BUILD: &str = r#"machine build(builder: @mutable Build) {
    Build::application(builder, "named_entry");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
"#;
const MAIN: &str = "machine main() {}\n";

fn invoke(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn default_build_selects_only_the_named_files_main() {
    let project = TemporaryDirectory::create("named-default", false).unwrap();
    let root = project.path();
    std::fs::write(root.join("build.omg"), "machine build() {}").unwrap();
    std::fs::write(root.join("chosen.omg"), MAIN).unwrap();
    let built = invoke(root, &["build", "chosen.omg"]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = invoke(root, &["run", "--engine", "psi", "chosen.omg"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    std::fs::write(root.join("chosen.omg"), "machine other() {}").unwrap();
    std::fs::write(root.join("main.omg"), MAIN).unwrap();
    let missing = invoke(root, &["build", "chosen.omg"]);
    assert!(!missing.status.success());
    let reason = String::from_utf8_lossy(&missing.stderr);
    assert!(
        reason.contains("entry") || reason.contains("main"),
        "{reason}"
    );
}

#[test]
fn a_renamed_build_can_select_a_product_from_the_same_file() {
    let project = TemporaryDirectory::create("named-authored", false).unwrap();
    let root = project.path();
    std::fs::write(root.join("build.omg"), "machine build() {}").unwrap();
    std::fs::write(root.join("release.omg"), format!("{BUILD}{MAIN}")).unwrap();
    let run = invoke(
        root,
        &[
            "run",
            "--engine",
            "psi",
            "--target",
            "linux_x86_64",
            "release.omg",
        ],
    );
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    std::fs::write(
        root.join("release.omg"),
        format!("{BUILD}{MAIN}").replace("main", "launch"),
    )
    .unwrap();
    let alternate = invoke(
        root,
        &[
            "run",
            "--engine",
            "psi",
            "--target",
            "linux_x86_64",
            "release.omg",
        ],
    );
    assert!(
        alternate.status.success(),
        "{}",
        String::from_utf8_lossy(&alternate.stderr)
    );

    std::fs::write(
        root.join("release.omg"),
        format!("machine build() {{}}\n{MAIN}"),
    )
    .unwrap();
    let invalid = invoke(root, &["build", "release.omg"]);
    assert!(!invalid.status.success());
    let reason = String::from_utf8_lossy(&invalid.stderr);
    assert!(reason.contains("one original BuildPlan parameter"), "{reason}");

    std::fs::write(
        root.join("release.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::application(builder, "first");
    Build::application(builder, "second");
}
machine main(arg: UnavailableSchema) {}"#,
    )
    .unwrap();
    let incomplete = invoke(root, &["build", "release.omg"]);
    assert!(!incomplete.status.success());
    let reason = String::from_utf8_lossy(&incomplete.stderr);
    assert!(
        reason.contains("UnavailableSchema")
            || reason.contains("while reading the signature of `main`"),
        "{reason}"
    );
}

#[test]
fn private_ram_can_only_initialize_an_installed_argument() {
    let project = TemporaryDirectory::create("hosted-private-entry-ram", false).unwrap();
    let root = project.path();
    std::fs::write(root.join("build.omg"), BUILD).unwrap();
    std::fs::write(root.join("main.omg"), MAIN).unwrap();
    let run = |ram: bool| {
        let mut args = vec!["run", "--engine", "psi", "--target", "linux_x86_64"];
        if ram {
            args.extend(["--entry-ram", "0=29000000"]);
        }
        args.push("build.omg");
        invoke(root, &args)
    };
    let empty = run(false);
    assert!(empty.status.success(), "{empty:?}");
    let extra = run(true);
    assert!(!extra.status.success());
    let reason = String::from_utf8_lossy(&extra.stderr);
    assert!(
        reason.contains("private entry RAM initializer answers no declared entry position"),
        "{reason}"
    );

    std::fs::write(
        root.join("main.omg"),
        "data Registers { value: u32; }\nmachine main(view: @mutable Registers) {}\n",
    )
    .unwrap();
    for ram in [false, true] {
        let undeclared = run(ram);
        assert!(!undeclared.status.success());
        let reason = String::from_utf8_lossy(&undeclared.stderr);
        assert!(
            reason.contains("hosted `ProgramEntry` exposes no arrival parameters")
                && reason.contains("`main` declares 1 visible parameters"),
            "{reason}"
        );
    }
}
