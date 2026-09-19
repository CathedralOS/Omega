use super::{Sources, compile, identity, selections};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::path::Path;
use terminal_interpreter::{TerminalExecutionResult, interpret_terminal_artifact};

fn package_inputs(root: &Path, library: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.to_path_buf()),
            PackageSourceBinding::new(identity(2), "library", library.to_path_buf()),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "library",
            identity(2),
        )],
    )
    .expect("direct dependency")
}

#[test]
fn qualified_domain_selection_reaches_the_separate_duplicate_validation_boundary() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain<const N: u64> u64::Below<N> requires self < N;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use bounds; pub const VALUE: u64 in bounds::Below<8> = 7;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; domain<const N: u64> u64::Below<N> requires self > N;
         machine read() -> u64 { settings::VALUE }",
    );
    // Selection must not falsely pool the root's generic leaf with the exact
    // qualified declaration. Validation still has a separate leaf-name-based
    // duplicate check; retain this customer until that owner-aware check lands.
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(
        error.contains("declared more than once with different normalized semantics"),
        "{error}"
    );
    assert!(
        !error.contains("declaration-site proof checking"),
        "{error}"
    );
}

#[test]
fn nested_indexed_domain_facts_use_the_declaring_files_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain<const N: u64> u64::Below<N> requires self < N;",
    );
    Sources::write(
        library.join("policy.omg"),
        "module policy; use bounds::Below;
         pub domain<const Limit: u64> u64::Window<Limit>
         requires self in Below<Limit>, self in Below<9>;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use policy; pub const VALUE: u64 in policy::Window<8> = 1 + 6;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; const Below: u64 = 91;
         machine read() -> u64 { settings::VALUE }",
    );
    let checked = compile(&root, package_inputs(&root, &library));
    assert!(!selections(&checked, "bounds::Below", identity(2)).is_empty());
    assert!(!selections(&checked, "policy::Window", identity(2)).is_empty());
    assert_source_free_seven(checked);
}

fn rejection(root: &Path, inputs: PackageCompilationInputs) -> String {
    compiler::compile_to_checked(compiler::CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..compiler::CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .map(|_| ())
    .expect_err("invalid constrained constant must reject")
    .iter()
    .map(|diagnostic| diagnostic.message.as_str())
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn unused_nested_indexed_constants_reject_false_facts() {
    let tree = Sources::new();
    let root = tree.package("root");
    for value in ["8", "9", "4 + 4"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const N: u64> u64::Below<N> requires self < N;
             domain<const Limit: u64> u64::Window<Limit> requires self in Below<Limit>;
             const UNUSED: u64 in Window<8> = {value};
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains("for const `UNUSED` is false"), "{error}");
    }
}

#[test]
fn nested_index_arguments_preserve_declared_carrier_and_range() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (outer_type, argument) in [("u64", "7"), ("u64", "256"), ("u8", "256")] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const N: u8> u64::Below<N> requires self < N;
             domain<const Limit: {outer_type}> u64::Window<Limit> requires self in Below<Limit>;
             const UNUSED: u64 in Window<{argument}> = 1;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains("expected `u8`") || error.contains("does not fit `u8`"),
            "{outer_type} {argument}: {error}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        "domain<const N: u8> u64::Below<N> requires self < N;
         domain u64::Window requires self in Below<256>;
         const UNUSED: u64 in Window = 1; machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(error.contains("does not fit `u8`"), "{error}");
}

#[test]
fn private_range_and_arithmetic_qualifications_keep_their_existing_route() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "const RANGE: u64[0..=9] = 7;
         const POLICY: u64 in Wrapping = 7;
         machine read() -> u64 { 7 }",
    );
    compile(&root, super::root_inputs(&root));
}

#[test]
fn private_indexed_domain_is_not_exposed_by_qualified_spelling() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; domain<const N: u64> u64::Below<N> requires self < N;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::bounds; const UNUSED: u64 in bounds::Below<8> = 7;
         machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("private"), "{error}");
}

#[test]
fn competing_indexed_domain_imports_reject_in_both_orders() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub domain<const N: u64> u64::Below<N> requires self < N;"),
        );
    }
    for imports in [
        "use first::Below; use second::Below;",
        "use second::Below; use first::Below;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{imports} const UNUSED: u64 in Below<8> = 7; machine read() -> u64 {{ 7 }}"),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains("declaration-site proof checking"), "{error}");
    }
}

#[test]
fn transitive_loading_does_not_expose_indexed_domains() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain<const N: u64> u64::Below<N> requires self < N;",
    );
    Sources::write(
        middle.join("bridge.omg"),
        "module bridge; use library::bounds;",
    );
    Sources::write(
        root.join("main.omg"),
        "use middle::bridge; use library::bounds;
         const UNUSED: u64 in bounds::Below<8> = 7; machine read() -> u64 { 7 }",
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "library", library),
            PackageSourceBinding::new(identity(3), "middle", middle),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(3)),
            PackageDependencyBinding::new(identity(3), "library", identity(2)),
        ],
    )
    .expect("transitive package graph");
    let error = rejection(&root, inputs);
    assert!(
        error.contains("names package `library`") && error.contains("builder.depend_as"),
        "{error}"
    );
}

#[test]
fn nested_indexed_facts_cannot_use_sibling_or_consumer_imports() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain<const N: u64> u64::Below<N> requires self < N;",
    );
    Sources::write(
        library.join("relay.omg"),
        "module policy; use bounds::Below;",
    );
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<const N: u64> u64::Window<N> requires self in Below<N>;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use policy; use relay;
         pub const VALUE: u64 in policy::Window<8> = 7;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; use library::bounds::Below;
         machine read() -> u64 { settings::VALUE }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("declaration-site proof checking"), "{error}");
}

#[test]
fn nested_indexed_recursion_remains_fenced() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain<const N: u64> u64::First<N> requires self in Second<N>;
         domain<const M: u64> u64::Second<M> requires self in First<M>;
         const UNUSED: u64 in First<8> = 7; machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(
        error.contains("declaration-site proof checking")
            || error.contains("domain membership cycle"),
        "{error}"
    );
}

fn assert_source_free_seven(checked: compiler::CheckedCompilation) {
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("indexed constrained constant reaches Terminal");
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("independent source-free replay"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(7, 64))
    );
}
