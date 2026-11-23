use platform_custody::temporary_directory::TemporaryDirectory;
use std::process::{Command, Output, Stdio};

/// Run `omega` on a copy of the fixture's one source file, so nothing the
/// compiler writes lands in the checkout.
pub fn invoke(source: &str, arguments: &[&str]) -> (Output, String) {
    let scratch = TemporaryDirectory::create("omega-test-program", false).unwrap();
    std::fs::write(scratch.path().join("main.omg"), source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .arg("main.omg")
        .current_dir(scratch.path())
        .stdin(Stdio::null())
        .output()
        .expect("the compiler runs on the fixture");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output, report)
}
