//! A shared session must refuse a changed dependency contract or proof context.

use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;

fn check(compiler: &mut compiler::CompilationSession, root: &Path) -> Result<(), String> {
    orchestration::check_source(
        compiler,
        compiler::CheckedCompileRequest::new(root, Some("linux_x86_64")),
    )
    .result
    .map(|_| ())
    .map_err(|failure| format!("{:?}", failure.into_diagnostics()))
}

#[test]
fn shared_compiler_rechecks_changed_dependency_contract_and_recovers_after_refusal() {
    let workspace = TemporaryDirectory::create("shared-project-compilation", false).unwrap();
    let root = workspace.path();
    let dependency = root.join("dependency.omg");
    let valid = "public machine bounded(value: u64) -> u64 requires value < 10u64 { value }\n";
    std::fs::write(&dependency, valid).unwrap();
    let first = root.join("first.omg");
    let second = root.join("second.omg");
    std::fs::write(
        &first,
        "use dependency; machine main() -> u64 { bounded(5u64) }\n",
    )
    .unwrap();
    std::fs::write(
        &second,
        "use dependency; machine main() -> u64 { bounded(6u64) }\n",
    )
    .unwrap();
    let mut compiler = compiler::CompilationSession::default();
    check(&mut compiler, &first).unwrap();
    check(&mut compiler, &second).unwrap();
    std::fs::write(&dependency, valid.replace("10u64", "5u64")).unwrap();
    for source in [&first, &second] {
        let error = check(&mut compiler, source)
            .expect_err("a previous check cannot satisfy the changed contract");
        assert!(
            error.contains("violates required fact `value < 5`"),
            "{error}"
        );
    }
    std::fs::write(&dependency, valid).unwrap();
    check(&mut compiler, &second).unwrap();
    check(&mut compiler, &first).unwrap();
}

#[test]
fn matching_body_hash_with_different_dependency_context_is_refused() {
    let workspace = TemporaryDirectory::create("dependency-context-refusal", false).unwrap();
    let root = workspace.path();
    let wrapper = "public machine seven() -> u64 ensures result == 7 { supplied() }\n";
    let good = root.join("good.omg");
    let bad = root.join("bad.omg");
    std::fs::write(
        &good,
        format!("public machine supplied() -> u64 {{ 7 }}\n{wrapper}machine main() -> u64 {{ seven() }}\n"),
    )
    .unwrap();
    std::fs::write(
        &bad,
        format!("public machine supplied() -> u64 {{ 8 }}\n{wrapper}machine main() -> u64 {{ seven() }}\n"),
    )
    .unwrap();
    let mut compiler = compiler::CompilationSession::default();
    check(&mut compiler, &good).unwrap();
    let error = check(&mut compiler, &bad)
        .expect_err("a matching body hash from another dependency must not admit seven");
    assert!(
        error.contains("result == 7")
            || error.contains("cannot prove")
            || error.contains("ensures"),
        "{error}"
    );
}
