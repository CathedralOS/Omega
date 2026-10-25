//! Guard the owning entrance and direct compiler/engine dependency boundaries.

#[test]
fn runtime_borrows_the_produced_terminal_storage() {
    let source = include_str!("execution.rs");
    assert!(source.contains("TerminalProgram::borrow(artifact)"));
    assert!(!source.contains("TerminalProgram::load("));
    assert!(!source.contains("semantic_bytes()"));
}

#[test]
fn compilation_and_cli_do_not_own_runtime_integration() {
    let compiler = include_str!("../../../compilation/driver/src/lib.rs");
    assert!(!compiler.contains("pub mod runtime"));
    let manifest = include_str!("../../../compilation/driver/Cargo.toml");
    let production = manifest.split("[dev-dependencies]").next().unwrap();
    assert!(!production.contains("terminal-interpreter ="));
    assert!(!production.contains("terminal-runtime ="));
    let cli = include_str!("../../cli/Cargo.toml");
    assert!(!cli.contains("terminal-interpreter ="));
    assert!(cli.contains("terminal-runtime ="));
    let runtime = include_str!("../Cargo.toml");
    assert!(!runtime.lines().any(|line| line.starts_with("compiler =")));
    assert!(!runtime.contains("/pipeline/"));
}
