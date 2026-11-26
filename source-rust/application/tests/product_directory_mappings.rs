//! The Build and its interpreted Product receive independent guest directories.
mod support;

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn a_psi_product_reads_its_own_mount_and_writes_only_to_its_output() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::{fs, process::Command};

    let scratch = TemporaryDirectory::create("product-directory-mappings", false).unwrap();
    let package = scratch.path().join("package");
    let build = scratch.path().join("build-input");
    let input = scratch.path().join("product-input");
    let output = scratch.path().join("product-output");
    for directory in [&package, &build, &input, &output] {
        fs::create_dir(directory).unwrap();
    }
    let fixture = support::repository_root()
        .join("source-rust/application/tests/fixtures/cli/run/mapped_filesystem");
    for file in ["build.omg", "main.omg"] {
        fs::copy(fixture.join(file), package.join(file)).unwrap();
    }
    fs::write(build.join("x"), b"B").unwrap();
    fs::write(input.join("x"), b"R").unwrap();

    let run = Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(&package)
        .args(["run", "--engine", "psi", "--build-directories-readwrite"])
        .arg(format!("{}::/r", build.display()))
        .arg("--run-directories-readonly")
        .arg(format!("{}::/r", input.display()))
        .arg("--run-directories-readwrite")
        .arg(format!("{}::/w", output.display()))
        .arg("build.omg")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    // The Product only creates this file after reading R and observing EACCES on a write-open.
    assert!(output.join("x").is_file());
    assert_eq!(fs::read(input.join("x")).unwrap(), b"R");
    assert_eq!(fs::read(build.join("x")).unwrap(), b"B");
}
