use super::{Sources, compile, identity, root_inputs};
use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::path::Path;
use terminal_interpreter::{TerminalExecutionResult, interpret_terminal_artifact};

#[test]
fn imported_trait_default_keeps_its_declaring_module_through_terminal_execution() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("defaults.omg"),
        "module defaults; const VALUE: u64 = 7;
         pub trait Reader { machine read(&self) -> u64 { VALUE } }",
    );
    Sources::write(
        root.join("decoy.omg"),
        "module decoy; const VALUE: u64 = 91;
         pub trait Reader { machine read(&self) -> u64 { VALUE } }",
    );
    Sources::write(
        root.join("main.omg"),
        "use defaults::Reader; const VALUE: u64 = 42;
         data Worker {} membership: Worker satisfies Reader;
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
    );
    assert_source_free_result(compile(&root, root_inputs(&root)));
}

fn assert_source_free_result(checked: compiler::CheckedCompilation) {
    let implementation = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == "Worker::read")
        .expect("selected default implementation");
    assert_eq!(
        checked
            .symbols
            .symbol_package_identity(implementation.symbol),
        Some(identity(1)),
    );
    for state in checked.typed.machine_states(implementation) {
        assert_eq!(
            checked.symbols.symbol_package_identity(state.symbol),
            Some(identity(1)),
            "implementation states belong to the conforming package, not the template",
        );
    }
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("imported default reaches Terminal");
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("selected default executes without source"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(7, 64)),
    );
}

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
    .expect("package graph")
}

#[test]
fn foreign_generic_default_retains_its_own_private_import_after_specialization() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("values.omg"),
        "module values; const VALUE: u64 = 7;",
    );
    Sources::write(
        library.join("defaults.omg"),
        "module defaults; use values::VALUE;
         pub trait Reader<T> { machine read(&self, value: T) -> u64 { VALUE } }",
    );
    Sources::write(
        root.join("decoy.omg"),
        "module decoy; pub trait Reader<T> { machine read(&self, value: T) -> u64 { 91 } }",
    );
    for carrier in ["u32", "u64"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use library::defaults::Reader; const VALUE: u64 = 42;
                 data Worker {{}} membership: Worker satisfies Reader<{carrier}>;
                 machine read() -> u64 {{ let worker: Worker = Worker {{}}; worker.read(3{carrier}) }}"
            ),
        );
        assert_source_free_result(compile(&root, package_inputs(&root, &library)));
    }
}

#[test]
fn foreign_default_requires_public_trait_and_direct_dependency() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        root.join("main.omg"),
        "use library::defaults::Reader;
         data Worker {} membership: Worker satisfies Reader;
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
    );
    for (visibility, expose_library) in [("", true), ("pub ", false)] {
        Sources::write(
            library.join("defaults.omg"),
            &format!(
                "module defaults; {visibility}trait Reader {{ machine read(&self) -> u64 {{ 7 }} }}"
            ),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(if expose_library {
                package_inputs(&root, &library)
            } else {
                root_inputs(&root)
            }),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("a default body cannot grant selection authority over its trait");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(if expose_library {
                    "private"
                } else {
                    "failed to resolve"
                })),
            "{diagnostics:?}",
        );
    }
}

#[test]
fn transitive_package_loading_does_not_expose_a_default_trait() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let library = tree.package("library");
    Sources::write(
        library.join("defaults.omg"),
        "module defaults; pub trait Reader { machine read(&self) -> u64 { 7 } }",
    );
    Sources::write(
        middle.join("bridge.omg"),
        "module bridge; use library::defaults::Reader;",
    );
    Sources::write(
        root.join("main.omg"),
        "use middle::bridge; use library::defaults::Reader;
         data Worker {} membership: Worker satisfies Reader;
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
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
    .expect("reachable transitive package graph");
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("loading a dependency's default does not expose it transitively");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("names package `library`")
                && diagnostic.message.contains("builder.depend_as")
        ),
        "{diagnostics:?}",
    );
}

#[test]
fn default_body_cannot_borrow_another_files_import() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("values.omg"),
        "module values; const VALUE: u64 = 7;",
    );
    Sources::write(
        root.join("relay.omg"),
        "module defaults; use values::VALUE;",
    );
    Sources::write(
        root.join("defaults.omg"),
        "module defaults; pub trait Reader { machine read(&self) -> u64 { VALUE } }",
    );
    Sources::write(
        root.join("main.omg"),
        "use defaults::Reader; use values::VALUE;
         data Worker {} membership: Worker satisfies Reader;
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("sibling and consumer imports cannot authorize a trait's default body");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("VALUE")),
        "{diagnostics:?}",
    );
}

#[test]
fn foreign_default_cannot_select_consumer_private_names() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("defaults.omg"),
        "module defaults; pub trait Reader { machine read(&self) -> u64 { CONSUMER_ONLY } }",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::defaults::Reader; const CONSUMER_ONLY: u64 = 7;
         data Worker {} membership: Worker satisfies Reader;
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&root, &library)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("local implementation ownership cannot grant body access to consumer names");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("CONSUMER_ONLY")),
        "{diagnostics:?}",
    );
}

#[test]
fn inherited_foreign_default_keeps_its_private_helper_and_local_override_wins() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("defaults.omg"),
        "module defaults;
         machine answer() -> u64 { 7 }
         pub trait Base { machine read(&self) -> u64 { answer() } }
         pub trait Reader { requires Base; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::defaults::Reader;
         machine answer() -> u64 { 91 }
         data Worker {} membership: Worker satisfies Reader;
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
    );
    assert_source_free_result(compile(&root, package_inputs(&root, &library)));
    Sources::write(
        root.join("main.omg"),
        "use library::defaults::Reader;
         data Worker {} membership: Worker satisfies Reader;
         machine Worker::read(&self) -> u64 { 7 }
         machine read() -> u64 { let worker: Worker = Worker {}; worker.read() }",
    );
    Sources::write(
        library.join("defaults.omg"),
        "module defaults;
         pub trait Base { machine read(&self) -> u64 { 91 } }
         pub trait Reader { requires Base; }",
    );
    assert_source_free_result(compile(&root, package_inputs(&root, &library)));
}

#[test]
fn synthesized_equatable_keeps_implementation_state_ownership() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
         data Point { value: u64; }
         membership: Point satisfies Equatable;
         machine read(left: &Point, right: &Point) -> bool { left.equals(right) }",
    );
    let checked = compile(&root, root_inputs(&root));
    let implementation = checked
        .typed
        .machines()
        .iter()
        .find(|machine| {
            checked.symbols.display_path(machine.symbol, "::")
                == "__omega_synthesized_equatable::Point::equals"
        })
        .expect("structural equality implementation");
    for state in checked.typed.machine_states(implementation) {
        assert_eq!(
            checked.symbols.symbol_package_identity(state.symbol),
            Some(identity(1)),
        );
    }
}
