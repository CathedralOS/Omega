//! The caller selects the first Build by naming a file. Its Product receives a checked entry.
#[path = "support/project.rs"]
mod project;

use platform_custody::temporary_directory::TemporaryDirectory;

const BUILD: &str = r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "named_entry" };
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::main);
}
"#;
const MAIN: &str = "machine main() {}\n";

#[test]
fn default_build_selects_only_the_named_files_main() {
    let scratch = TemporaryDirectory::create("named-default", false).unwrap();
    let root = scratch.path();
    std::fs::write(root.join("build.omg"), "machine build() {}").unwrap();
    std::fs::write(root.join("chosen.omg"), MAIN).unwrap();
    let built = project::omega(root, &["build", "chosen.omg"]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = project::omega(root, &["run", "--engine", "psi", "chosen.omg"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    std::fs::write(root.join("chosen.omg"), "machine other() {}").unwrap();
    std::fs::write(root.join("main.omg"), MAIN).unwrap();
    let missing = project::omega(root, &["build", "chosen.omg"]);
    assert!(!missing.status.success());
    let reason = String::from_utf8_lossy(&missing.stderr);
    assert!(
        reason.contains("entry") || reason.contains("main"),
        "{reason}"
    );
}

#[test]
fn a_renamed_build_can_select_a_product_from_the_same_file() {
    let scratch = TemporaryDirectory::create("named-authored", false).unwrap();
    let root = scratch.path();
    std::fs::write(root.join("build.omg"), "machine build() {}").unwrap();
    std::fs::write(root.join("release.omg"), format!("{BUILD}{MAIN}")).unwrap();
    let run = project::omega(
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
    let alternate = project::omega(
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
    let invalid = project::omega(root, &["build", "release.omg"]);
    assert!(!invalid.status.success());
    let reason = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        reason.contains("one original BuildPlan parameter"),
        "{reason}"
    );

    std::fs::write(
        root.join("release.omg"),
        r#"machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "first" };
    builder.product = BuildProduct::Application { name: "second" };
}
machine main(arg: UnavailableSchema) {}"#,
    )
    .unwrap();
    let incomplete = project::omega(root, &["build", "release.omg"]);
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
    let scratch = TemporaryDirectory::create("hosted-private-entry-ram", false).unwrap();
    let root = scratch.path();
    std::fs::write(root.join("build.omg"), BUILD).unwrap();
    std::fs::write(root.join("main.omg"), MAIN).unwrap();
    let run = |ram: bool| {
        let mut args = vec!["run", "--engine", "psi", "--target", "linux_x86_64"];
        if ram {
            args.extend(["--entry-ram", "0=29000000"]);
        }
        args.push("build.omg");
        project::omega(root, &args)
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
