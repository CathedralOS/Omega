// Boundary tests verifying that `terminal-runtime` borrows produced Terminal
// storage without re-decoding and that `compiler` does not depend on runtime crates.

#[test]
fn runtime_borrows_the_produced_terminal_storage() {
    let source = include_str!("execution.rs");
    assert!(source.contains("TerminalProgram::borrow(artifact).map_err("));
    assert!(!source.contains("TerminalProgram::load("));
    assert!(!source.contains("semantic_bytes()"));
}

fn production_depends_on(manifest: &str, package: &str) -> bool {
    let mut in_dependencies = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]" || line.ends_with(".dependencies]");
        } else if in_dependencies
            && line
                .split_once('=')
                .is_some_and(|(key, _)| key.trim() == package)
        {
            return true;
        }
    }
    false
}

#[test]
fn compilation_and_cli_do_not_own_runtime_integration() {
    let compiler = include_str!("../../../compiler/compiler.rs");
    assert!(!compiler.contains("pub mod runtime"));
    let manifest = include_str!("../../../compiler/Cargo.toml");
    assert!(!production_depends_on(manifest, "terminal-interpreter"));
    assert!(!production_depends_on(manifest, "terminal-runtime"));
    let cli = include_str!("../../../../application/Cargo.toml");
    assert!(!production_depends_on(cli, "terminal-interpreter"));
    assert!(production_depends_on(cli, "terminal-runtime"));
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
