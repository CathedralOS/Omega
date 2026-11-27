// Boundary tests verifying that `terminal-runtime` borrows produced Terminal
// storage without re-decoding and that `compiler` does not depend on runtime crates.

#[test]
fn runtime_borrows_the_produced_terminal_storage() {
    let source = include_str!("execution.rs");
    assert!(source.contains("TerminalProgram::borrow(artifact).map_err("));
    assert!(!source.contains("TerminalProgram::load("));
    assert!(!source.contains("semantic_bytes()"));
}

#[test]
fn compilation_and_cli_do_not_own_runtime_integration() {
    let compiler = include_str!("../../../compiler/compiler.rs");
    assert!(!compiler.contains("pub mod runtime"));
    let manifest = include_str!("../../../compiler/Cargo.toml");
    let production = manifest.split("[dev-dependencies]").next().unwrap();
    assert!(!production.contains("terminal-interpreter ="));
    assert!(!production.contains("terminal-runtime ="));
    let cli = include_str!("../../../../application/Cargo.toml");
    assert!(!cli.contains("terminal-interpreter ="));
    assert!(cli.contains("terminal-runtime ="));
    let runtime = include_str!("../Cargo.toml");
    assert!(!runtime.lines().any(|line| line.starts_with("compiler =")));
    for line in runtime.lines().filter(|line| line.contains("/pipeline/")) {
        assert_eq!(
            line,
            r#"target-vocabulary = { path = "../../compiler/psi-to-native/representations/target-operations/target-vocabulary" }"#,
            "the only shared native pipeline dependency is the target vocabulary"
        );
    }
    assert!(
        !runtime
            .lines()
            .any(|line| line.starts_with("provider-admission ="))
    );
    assert!(!runtime.contains("products ="));
}
