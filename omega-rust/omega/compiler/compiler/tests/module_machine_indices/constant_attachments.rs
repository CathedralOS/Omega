use super::{Sources, compile, identity, selections};
use compiler::{CheckedCompileRequest, compile_to_checked};
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
    .expect("direct library dependency")
}

#[test]
fn foreign_nominal_constant_retains_declaring_package_and_exact_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("geometry.omg"),
        "module geometry; pub data Point [copy] { value: u64; }",
    );
    Sources::write(
        library.join("values.omg"),
        "module values; const VALUE: u64 = 7;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use geometry::Point; use values::VALUE;
         pub const SELECTED: Point = Point { value: VALUE };",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; data Point [copy] { wrong: bool; }
         const VALUE: u64 = 91;
         machine read() -> u64 { settings::SELECTED.value }",
    );
    let checked = compile(&root, package_inputs(&root, &library));
    assert!(!selections(&checked, "settings::SELECTED", identity(2)).is_empty());
    assert!(!selections(&checked, "geometry::Point", identity(2)).is_empty());
    assert_source_free_result(checked);
}

#[test]
fn specialized_foreign_template_constant_reaches_source_free_execution() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("geometry.omg"),
        "module geometry; pub data Box<T [copy]> [copy] { value: T; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; data Box<T [copy]> [copy] { wrong: T; }
         machine read() -> u64 { settings::SELECTED.value }",
    );
    for value in ["7", "1 + 6"] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; use geometry::Box;
             pub const SELECTED: Box<u64> = Box {{ value: {value} }};"
            ),
        );
        let checked = compile(&root, package_inputs(&root, &library));
        assert!(!selections(&checked, "settings::SELECTED", identity(2)).is_empty());
        assert_source_free_result(checked);
    }
}

fn assert_source_free_result(checked: compiler::CheckedCompilation) {
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
        .produce_artifact()
        .expect("foreign nominal constant reaches Terminal");
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("constant executes without source"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(7, 64))
    );
}

fn rejection(root: &Path, inputs: PackageCompilationInputs) -> String {
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .map(|_| ())
    .expect_err("invalid constant selection must reject")
    .iter()
    .map(|diagnostic| diagnostic.message.as_str())
    .collect::<Vec<_>>()
    .join("\n")
}

#[test]
fn equal_shaped_constructor_cannot_replace_the_selected_constant_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    for generic in [false, true] {
        let parameters = if generic { "<T [copy]>" } else { "" };
        let field = if generic { "T" } else { "u64" };
        let arguments = if generic { "<u64>" } else { "" };
        Sources::write(
            root.join("geometry.omg"),
            &format!("module geometry; pub data Box{parameters} [copy] {{ value: {field}; }}"),
        );
        Sources::write(
            root.join("decoy.omg"),
            &format!("module decoy; pub data Box{parameters} [copy] {{ value: {field}; }}"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use geometry; use decoy;
             const BAD: geometry::Box{arguments} = decoy::Box {{ value: 7 }};
             machine read() -> u64 {{ 0 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains("different nominal carrier"), "{error}");
    }
}

#[test]
fn foreign_constant_and_its_carrier_need_independent_public_visibility() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        root.join("main.omg"),
        "use library::settings; machine read() -> u64 { settings::SELECTED.value }",
    );
    for (carrier_visibility, constant_visibility) in [("pub ", ""), ("", "pub ")] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; {carrier_visibility}data Point [copy] {{ value: u64; }}
             {constant_visibility}const SELECTED: Point = Point {{ value: 7 }};"
            ),
        );
        let error = rejection(&root, package_inputs(&root, &library));
        assert!(error.contains("private"), "{error}");
    }
}

#[test]
fn constant_initializer_cannot_borrow_a_sibling_files_import() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("geometry.omg"),
        "module geometry; pub data Box<T [copy]> [copy] { value: T; }",
    );
    Sources::write(
        root.join("relay.omg"),
        "module settings; use geometry::Box;",
    );
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub const SELECTED: geometry::Box<u64> = Box { value: 7 };",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; use geometry::Box; machine read() -> u64 { settings::SELECTED.value }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(error.contains("Box"), "{error}");
}

#[test]
fn transitive_package_loading_does_not_expose_foreign_constants() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let library = tree.package("library");
    Sources::write(
        library.join("settings.omg"),
        "module settings; pub data Point [copy] { value: u64; }
         pub const SELECTED: Point = Point { value: 7 };",
    );
    Sources::write(
        middle.join("bridge.omg"),
        "module bridge; use library::settings;",
    );
    Sources::write(
        root.join("main.omg"),
        "use middle::bridge; use library::settings;
         machine read() -> u64 { settings::SELECTED.value }",
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
    let error = rejection(&root, inputs);
    assert!(
        error.contains("names package `library`") && error.contains("builder.depend_as"),
        "{error}"
    );
}

#[test]
fn nested_generic_constant_closes_each_declared_field_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("geometry.omg"),
        "module geometry; pub data Box<T [copy]> [copy] { value: T; }",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use geometry::Box;
         pub const SELECTED: Box<Box<u64>> = Box { value: Box { value: 1 + 6 } };",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings;
         machine read() -> u64 { settings::SELECTED.value.value }",
    );
    assert_source_free_result(compile(&root, package_inputs(&root, &library)));
}

#[test]
fn unused_constant_cannot_change_a_closed_generic_argument_tuple() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Box<T [copy]> [copy] { value: T; }
         const SOURCE: Box<u8> = Box { value: 7 };
         const BAD: Box<u64> = SOURCE;
         machine read() -> u64 { 0 }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(
        error.contains("expected record `Box<u64>`, found record `Box<u8>`"),
        "{error}"
    );
}

#[test]
fn competing_imported_constant_carriers_reject_in_both_orders() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub data Box<T [copy]> [copy] {{ value: T; }}"),
        );
    }
    for imports in [
        "use first::Box; use second::Box;",
        "use second::Box; use first::Box;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} const UNUSED: Box<u64> = Box {{ value: 7 }}; machine read() -> u64 {{ 0 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains("Box") && (error.contains("ambiguous") || error.contains("conflict")),
            "{error}"
        );
    }
}

#[test]
fn nested_generic_argument_keeps_its_qualified_nominal_sibling() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("geometry.omg"),
        "module geometry; pub data Point [copy] { value: u64; }
         pub data Box<T [copy]> [copy] { value: T; }
         pub data Pair<A [copy], B [copy]> [copy] { first: A; second: B; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use geometry;
         const SELECTED: geometry::Pair<geometry::Point, geometry::Box<u64>> = geometry::Pair {
             first: geometry::Point { value: 1 + 6 }, second: geometry::Box { value: 9 }
         };
         machine read() -> u64 { SELECTED.first.value }",
    );
    assert_source_free_result(compile(&root, super::root_inputs(&root)));
}

#[test]
fn generic_constant_array_elements_keep_their_declared_constructor_owner() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Box<T [copy]> [copy] { value: T; }
         const VALUES: [Box<u64>; 2] = [Box { value: 7 }, Box { value: 9 }];
         machine read() -> u64 { VALUES[0].value }",
    );
    assert_source_free_result(compile(&root, super::root_inputs(&root)));
}

#[test]
fn foreign_generic_record_tables_compose_field_and_index_projection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("settings.omg"),
        "module settings;
         pub data Cell<T [copy]> [copy] { value: T; }
         pub data Row<T [copy]> [copy] { cells: [Cell<T>; 2]; }
         pub const TABLE: [Row<u64>; 2] = [
             Row { cells: [Cell { value: 91 }, Cell { value: 93 }] },
             Row { cells: [Cell { value: 7 }, Cell { value: 95 }] }
         ];",
    );
    for (declaration, table) in [
        ("", "settings::TABLE"),
        (
            "const COPIED: [settings::Row<u64>; 2] = settings::TABLE;",
            "COPIED",
        ),
        (
            "const COPIED: [settings::Row<u64>; 2] = settings::TABLE;
             const SECOND: [settings::Row<u64>; 2] = COPIED;",
            "SECOND",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use library::settings;
         data Cell<T [copy]> [copy] {{ wrong: T; }}
         data Row<T [copy]> [copy] {{ wrong: Cell<T>; }}
         {declaration}
         machine read() -> u64 {{ {table}[1].cells[0].value }}"
            ),
        );
        let checked = compile(&root, package_inputs(&root, &library));
        assert!(!selections(&checked, "settings::TABLE", identity(2)).is_empty());
        assert_source_free_result(checked);
    }
}

#[test]
fn unmoduled_same_leaf_records_keep_collision_rejection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("types.omg"),
        "data Cell [copy] { value: bool; } pub const PRESENT: u64 = 1;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::types;
         data Cell [copy] { value: u64; }
         machine make() -> Cell { Cell { value: 7 } }
         const SELECTED: Cell = make();
         machine read() -> u64 { SELECTED.value }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("duplicate data `Cell`"), "{error}");
}

#[test]
fn copied_record_tables_preserve_unused_generic_arguments() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         pub data Cell<const TAG: u64> [copy] { value: u64; }
         pub const TABLE: [Cell<1>; 1] = [Cell { value: 7 }];",
    );
    for argument in [1, 2] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings;
                 data Cell<const TAG: u64> [copy] {{ value: u64; }}
                 const COPIED: [settings::Cell<{argument}>; 1] = settings::TABLE;
                 machine read() -> u64 {{ COPIED[0].value }}"
            ),
        );
        if argument == 1 {
            assert_source_free_result(compile(&root, super::root_inputs(&root)));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(error.contains("Cell"), "{error}");
        }
    }
}

#[test]
fn record_table_projections_preserve_bounds_types_and_nonaddressability() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, expression) in [
        ("u64", "VALUES[2].value"),
        ("u64", "VALUES[-1].value"),
        ("bool", "VALUES[0].value"),
        ("u8", "VALUES[0].value"),
        ("&u64", "&VALUES[0].value"),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "data Cell<T [copy]> [copy] {{ value: T; }}
             const VALUES: [Cell<u64>; 2] = [Cell {{ value: 7 }}, Cell {{ value: 9 }}];
             machine read() -> {carrier} {{ {expression} }}"
            ),
        );
        assert!(
            compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(super::root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .is_err(),
            "{expression} must not produce {carrier}"
        );
    }
}
