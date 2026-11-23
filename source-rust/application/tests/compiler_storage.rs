//! A build can acquire a dependency without retaining the compiler's source metadata.
#![cfg(target_os = "linux")]

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

#[test]
fn a_build_ignores_the_old_checked_source_directory() {
    let scratch = TemporaryDirectory::create("compiler-storage", false).unwrap();
    let app = scratch.path().join("app");
    let kit = scratch.path().join("kit");
    let user_cache = scratch.path().join("cache");
    fs::create_dir(&app).unwrap();
    fs::create_dir(&kit).unwrap();
    fs::write(
        app.join("build.omg"),
        r#"machine build(builder: @mutable Build) {
    Build::application(builder, "cache_probe");
    let registered: DependencyRegistration = Build::dependency(builder, dependency\from_source(Source::Path { location: "../kit" }));
    Build::bind(builder, linux_x86_64\ProgramEntry, main);
    Build::bind(builder, linux_arm64\ProgramEntry, main);
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

    let run = || {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .env("XDG_CACHE_HOME", &user_cache)
            .args(["run", "--both", "build.omg"])
            .current_dir(&app)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let report = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{report}");
        for expected in ["native exit 0", "interp exit: 0"] {
            assert!(report.lines().any(|line| line == expected), "{report}");
        }
    };
    run();
    let sources = user_cache.join("CathedralOS/Omega/source/v1/external-local-sources");
    assert!(sources.is_dir());
    let old_cache = sources.join("checked-source-cache");
    assert!(!old_cache.exists());

    fs::create_dir(&old_cache).unwrap();
    let record = old_cache.join("previous.rec");
    fs::write(&record, b"damaged metadata").unwrap();
    fs::set_permissions(&old_cache, fs::Permissions::from_mode(0o500)).unwrap();
    run();
    fs::set_permissions(&old_cache, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(fs::read(record).unwrap(), b"damaged metadata");
    assert_eq!(fs::read_dir(old_cache).unwrap().count(), 1);
}
