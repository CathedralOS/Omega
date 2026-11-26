//! Run an ordinary portable filesystem client against two independent Product grants.
mod support;

#[cfg(target_os = "linux")]
#[test]
fn a_psi_product_can_write_only_under_its_writable_guest_directory() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::{fs, process::Command};

    let scratch = TemporaryDirectory::create("product-directory-mappings", false).unwrap();
    let sample = support::repository_root().join("samples/cli/systems/file_journal");
    let library = std::env::var_os("OMEGA_LIBRARY_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| support::repository_root().join("source/library"));
    let package = scratch.path().join("package");
    let mapped = scratch.path().join("mapped");
    fs::create_dir(&package).unwrap();
    fs::create_dir(&mapped).unwrap();
    fs::write(
        package.join("build.omg"),
        fs::read_to_string(sample.join("build.omg"))
            .unwrap()
            .replace(
                "../../../../source/library/system",
                library.join("system").to_str().unwrap(),
            ),
    )
    .unwrap();
    fs::write(
        package.join("main.omg"),
        fs::read_to_string(sample.join("main.omg"))
            .unwrap()
            .replace("/tmp/omega_journal_dir", "/journal/work"),
    )
    .unwrap();

    // The first operation cleans up the previous journal through its guest path.
    let marker = mapped.join("work/note.txt");
    fs::create_dir(mapped.join("work")).unwrap();
    fs::write(&marker, b"existing").unwrap();
    let run = |mode: &str, expected: i32| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&package)
            .args(["run", "--engine", "psi", mode])
            .arg(format!("{}::/journal", mapped.display()))
            .arg("build.omg")
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(expected),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run("--run-directories-readonly", 0);
    assert_eq!(fs::read(&marker).unwrap(), b"existing");
    run("--run-directories-readwrite", 7);
    assert!(!mapped.join("work").exists());
}
