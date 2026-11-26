//! The syntax-only command reads exactly the named file, including errors at its end.
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::process::Command;

#[test]
fn parses_the_whole_file_without_loading_imports_and_reports_byte_locations() {
    let dir = TemporaryDirectory::create("source-parse", false).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .args(["source", "parse"])
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap()
    };
    let file = dir.path().join("-sample.omg");
    let args = ["--", "-sample.omg"];
    let valid = "use {absent}/unavailable.omg;\n";
    fs::write(&file, valid).unwrap();
    let parsed = run(&args);
    assert!(parsed.status.success(), "{parsed:?}");
    assert!(parsed.stdout.is_empty() && parsed.stderr.is_empty());

    for (bad, line, column, start, end) in [
        ("machine", 2, 8, valid.len() + 7, valid.len() + 7),
        ("/*é*/ `", 2, 8, valid.len() + 7, valid.len() + 8),
    ] {
        fs::write(&file, format!("{valid}{bad}")).unwrap();
        let refused = run(&args);
        assert_eq!(refused.status.code(), Some(1), "{refused:?}");
        let stderr = String::from_utf8(refused.stderr).unwrap();
        assert!(
            stderr.contains(&format!("-sample.omg:{line}:{column}:")),
            "{stderr}"
        );
        assert!(
            stderr.contains(&format!("bytes {start}..{end}")),
            "{stderr}"
        );
    }
    fs::write(&file, b"//\n\xff").unwrap();
    let invalid = run(&args);
    assert_eq!(invalid.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&invalid.stderr)
            .contains("-sample.omg:2:1: invalid UTF-8 (bytes 3..4)")
    );
    fs::remove_file(file).unwrap();
    let missing = run(&args);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("-sample.omg"));
    for invalid in [&[][..], &["-sample.omg"], &["--"], &["a.omg", "b.omg"]] {
        assert_eq!(run(invalid).status.code(), Some(2), "{invalid:?}");
    }
}
