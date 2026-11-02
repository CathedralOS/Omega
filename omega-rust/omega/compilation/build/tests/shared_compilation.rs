use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;

fn check(compiler: &mut compiler::CompilationSession, root: &Path) -> Result<(), String> {
    omega_build::check_source(
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

#[test]
fn second_consumer_reuses_the_dependency_judgment_and_declaration_pages() {
    let workspace = TemporaryDirectory::create("shared-dependency-judgment", false).unwrap();
    let root = workspace.path();
    let dependency = root.join("dependency.omg");
    std::fs::write(
        &dependency,
        "public data Cell [copy] { value: u64; }\n\
         public machine project<'storage>(value: &'storage Cell) -> &'storage u64 {\n\
             let view: &u64 = &value.value;\n\
             view\n\
         }\n\
         public machine count(value: u64) -> u64 { value }\n\
         public machine equal(value: u64) -> bool { value == 19u64 }\n",
    )
    .unwrap();
    let consumer = "use dependency;\n\
         machine require_equal(value: u64) requires equal(value) { }\n\
         machine main() -> u64 {\n\
             let cell: Cell = Cell { value: 19u64 };\n\
             let view: &u64 = project(&cell);\n\
             let observed: u64 = count(19u64);\n\
             let difference: u64 = observed - 19u64;\n\
             require_equal(19u64);\n";
    let first = root.join("first.omg");
    let second = root.join("second.omg");
    std::fs::write(&first, format!("{consumer}    view + difference\n}}\n")).unwrap();
    std::fs::write(&second, format!("{consumer}    difference + view\n}}\n")).unwrap();
    let mut compiler = compiler::CompilationSession::default();
    let forks_before = compiler::declaration_page_fork_totals();
    let entailment_before = compiler::dependency_entailment_totals();
    check(&mut compiler, &first).unwrap();
    let after_first = compiler::dependency_entailment_totals();
    let judged_first = after_first.1 - entailment_before.1;
    assert_eq!(after_first.0, entailment_before.0);
    assert!(
        judged_first > 0,
        "the first consumer did not judge its dependency from source"
    );
    let settled_before_second = compiler::settled_check_totals().0;
    let whole_before_second = compiler::whole_program_reuse_hits();
    let reads_before_second = compiler::dependency_body_read_totals();
    check(&mut compiler, &second).unwrap();
    let after_second = compiler::dependency_entailment_totals();
    let reused_second = after_second.0 - after_first.0;
    let judged_second = after_second.1 - after_first.1;
    assert!(
        reused_second >= 1,
        "second consumer re-judged the dependency from source (reused {reused_second}, judged {judged_second})"
    );
    assert!(
        judged_second < judged_first,
        "second consumer judged {judged_second} bodies after the first judged {judged_first}"
    );
    assert!(
        compiler::settled_check_totals().0 > settled_before_second,
        "a different typed program took the first consumer's whole-program judgment"
    );
    assert_eq!(
        compiler::whole_program_reuse_hits(),
        whole_before_second,
        "whole-program reuse matched a different typed input"
    );
    let reads = compiler::dependency_body_read_totals();
    let frozen_return_origins = reads[0].0 - reads_before_second[0].0;
    let frozen_binding_stability = reads[1].0 - reads_before_second[1].0;
    let frozen_observation = reads[2].0 - reads_before_second[2].0;
    let frozen_return_interval = reads[3].0 - reads_before_second[3].0;
    let source_reads = (
        reads[0].1 - reads_before_second[0].1,
        reads[1].1 - reads_before_second[1].1,
        reads[2].1 - reads_before_second[2].1,
        reads[3].1 - reads_before_second[3].1,
    );
    assert!(
        frozen_return_origins
            + frozen_binding_stability
            + frozen_observation
            + frozen_return_interval
            > 0,
        "frozen body reads {frozen_return_origins} {frozen_binding_stability} {frozen_observation} {frozen_return_interval} source {source_reads:?}"
    );
    assert!(
        frozen_return_origins > 0,
        "returned-reference origins did not read the frozen dependency body; frozen {frozen_return_origins} {frozen_binding_stability} {frozen_observation} {frozen_return_interval} source {source_reads:?}"
    );
    assert!(
        frozen_binding_stability > 0,
        "binding stability did not read the frozen dependency body"
    );
    assert!(
        frozen_observation > 0,
        "denotational admission did not read the frozen dependency body"
    );
    assert!(
        frozen_return_interval > 0,
        "return intervals did not read the frozen dependency body under the caller actual"
    );
    let forks = compiler::declaration_page_fork_totals();
    let shared = forks.0 - forks_before.0;
    let copied = forks.1 - forks_before.1;
    assert!(
        shared > 0,
        "independent flow groups did not share declaration pages"
    );
    assert_eq!(
        copied, 0,
        "independent flow groups copied {copied} declaration baselines"
    );
}
