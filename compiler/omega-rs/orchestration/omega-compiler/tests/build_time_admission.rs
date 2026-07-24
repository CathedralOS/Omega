use omega_compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("omega-build-time-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create temporary program directory");
    let main_path = directory.join("main.omg");
    fs::write(&main_path, source).expect("write build-time admission program");
    main_path
}

fn compile_error(name: &str, source: &str) -> String {
    let main_path = write_program(name, source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("the build-time contract violation must reject compilation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program has a parent"));
    rendered
}

#[test]
fn const_evaluation_rejects_possible_suspension_independently() {
    let error = compile_error(
        "suspends",
        r#"
boundary machine wait_for_length() -> u64 suspends;

machine length() -> u64 {
    let value: u64 = wait_for_length();
    transition { _ -> value }
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(error.contains("may suspend"), "{error}");
    assert!(!error.contains("may block"), "{error}");
    assert!(!error.contains("service reach ["), "{error}");
}

#[test]
fn const_evaluation_rejects_possible_blocking_independently() {
    let error = compile_error(
        "blocks",
        r#"
boundary machine blocking_length() -> u64 blocks;

machine helper() -> u64 {
    let value: u64 = blocking_length();
    transition { _ -> value }
}

machine length() -> u64 {
    let value: u64 = helper();
    transition { _ -> value }
}

data Buffer { bytes: [u8; length()]; }
data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(
        error.contains("machine `length` is not build-time admissible"),
        "{error}"
    );
    assert!(error.contains("may block"), "{error}");
    assert!(!error.contains("may suspend"), "{error}");
    assert!(!error.contains("service reach ["), "{error}");
}
