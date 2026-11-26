//! Package review and compilation share acquired sources. Compiler working state ends with the invocation.
#![cfg(target_os = "linux")]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

#[test]
fn builds_and_reviews_ignore_the_old_compiler_directories() {
    let scratch = TemporaryDirectory::create("compiler-storage", false).unwrap();
    let app = scratch.path().join("app");
    let kit = scratch.path().join("kit");
    let user_cache = scratch.path().join("cache");
    fs::create_dir(&app).unwrap();
    fs::create_dir(&kit).unwrap();
    fs::write(
        app.join("build.omg"),
        r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::application(builder, "cache_probe");
    let registered: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../kit" }));
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
}"#,
    )
    .unwrap();
    fs::write(
        app.join("main.omg"),
        r#"use {kit}/math.omg as lib;
machine main() crashes Trap {
    transition |lib|::answer() == 7 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}"#,
    )
    .unwrap();
    fs::write(
        kit.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "kit"); }"#,
    )
    .unwrap();
    fs::write(kit.join("math.omg"), "public machine answer() -> i32 { 7 }").unwrap();

    let omega = |arguments: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .env("XDG_CACHE_HOME", &user_cache)
            .args(arguments)
            .current_dir(&app)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.status.success(), "{report}");
        report
    };
    let run = || {
        let report = omega(&["run", "--both", "build.omg"]);
        for expected in ["native exit 0", "interp exit: 0"] {
            assert!(report.lines().any(|line| line == expected), "{report}");
        }
    };
    let update = || {
        omega(&[
            "packages",
            "update",
            "--project",
            ".",
            "--target",
            "linux_x86_64",
            "--offline",
            "--dangerously-accept-all",
        ]);
        fs::read(app.join("omega.lock")).unwrap()
    };

    run();
    let lock = update();
    let sources = user_cache.join("CathedralOS/Omega/source/v1/external-local-sources");
    assert!(sources.is_dir());
    let old_directories =
        ["checked-source-cache", "compiled-review-cache"].map(|name| sources.join(name));
    for old in &old_directories {
        assert!(!old.exists());
        fs::create_dir(old).unwrap();
        fs::write(old.join("previous.rec"), b"damaged compiler state").unwrap();
        fs::set_permissions(old, fs::Permissions::from_mode(0o500)).unwrap();
    }
    // Force a fresh review under the same package inputs and stale per-user store.
    fs::remove_file(app.join("omega.lock")).unwrap();
    assert_eq!(update(), lock);
    run();
    for old in &old_directories {
        fs::set_permissions(old, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            fs::read(old.join("previous.rec")).unwrap(),
            b"damaged compiler state"
        );
        assert_eq!(fs::read_dir(old).unwrap().count(), 1);
    }
}
