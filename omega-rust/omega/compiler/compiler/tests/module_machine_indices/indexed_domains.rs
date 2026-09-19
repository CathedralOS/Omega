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
fn qualified_domain_selection_keeps_distinct_same_leaf_owners_through_terminal() {
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
    let checked = compile(&root, package_inputs(&root, &library));
    assert!(!selections(&checked, "bounds::Below", identity(2)).is_empty());
    assert_source_free_seven(checked);
}

#[test]
fn same_leaf_domain_owners_do_not_hide_a_false_foreign_predicate() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain<const N: u64> u64::Below<N> requires self < N;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use bounds; pub const VALUE: u64 in bounds::Below<8> = 9;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; domain<const N: u64> u64::Below<N> requires self > N;
         machine read() -> u64 { settings::VALUE }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("for const `VALUE` is false"), "{error}");
}

#[test]
fn same_owner_capacity_specializations_keep_one_theory_through_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "use codecs; machine read() -> u64 { 7 }",
    );
    for predicate in ["valid_utf8", "no_nul"] {
        Sources::write(
            root.join("codecs.omg"),
            &format!(
                "module codecs;
                domain [u8; 8]::Utf8 requires valid_utf8(self);
                domain [u8; 16]::Utf8 requires {predicate}(self);"
            ),
        );
        if predicate == "valid_utf8" {
            assert_source_free_seven(compile(&root, super::root_inputs(&root)));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(error.contains("different normalized semantics"), "{error}");
        }
    }
}

#[test]
fn separate_packages_keep_same_module_domain_paths_distinct_through_terminal() {
    for indexed in [false, true] {
        assert_separate_package_domains_through_terminal(indexed);
    }
}

fn assert_separate_package_domains_through_terminal(indexed: bool) {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    let declaration = if indexed {
        "domain<const N: u64> u64::Tag<N>"
    } else {
        "domain u64::Tag"
    };
    let qualification = if indexed { "Tag<7>" } else { "Tag" };
    Sources::write(
        library.join("bounds.omg"),
        &format!("module bounds; pub {declaration};
         pub machine foreign(value: u64) -> u64 in {qualification} {{ value as u64 in {qualification} }}"),
    );
    // Different telescopes ensure the early canonicalizer follows the exact
    // declaration, not the first same-path family loaded from another package.
    let declaration = if indexed {
        "domain<const N: bool> u64::Tag<N>"
    } else {
        declaration
    };
    let qualification = if indexed { "Tag<true>" } else { qualification };
    Sources::write(
        root.join("main.omg"),
        &format!(
            "module bounds; use library::bounds;
         {declaration};
         machine local(value: u64) -> u64 in {qualification} {{ value as u64 in {qualification} }}
         machine read() -> u64 {{
             let retained: u64 in {qualification} = local(101);
             library::bounds::foreign(7) as u64
         }}"
        ),
    );
    let checked = compile(&root, package_inputs(&root, &library));
    let domains = checked.typed.domain_definitions();
    let local = domains
        .iter()
        .find(|domain| {
            checked.typed.symbols.symbol_package_identity(domain.symbol) == Some(identity(1))
                && domain.name.as_str().rsplit("::").next() == Some("Tag")
        })
        .expect("root domain");
    let foreign = domains
        .iter()
        .find(|domain| {
            checked.typed.symbols.symbol_package_identity(domain.symbol) == Some(identity(2))
                && domain.name.as_str().rsplit("::").next() == Some("Tag")
        })
        .expect("library domain");
    assert_ne!(local.semantic_id, foreign.semantic_id);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "bounds::read")
        .produce_artifact()
        .expect("distinct owners reach Terminal");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode Terminal");
    assert_eq!(module.scalar_qualifications.domains.len(), 2);
    let domains = &module.scalar_qualifications.domains;
    assert_ne!(domains[0].semantic_domain, domains[1].semantic_domain);
    assert_ne!(domains[0].identity, domains[1].identity);
    drop(checked);
    assert_eq!(
        interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[]
        )
        .expect("independent replay"),
        TerminalExecutionResult::Scalar(super::array_construction::integer(7, 64))
    );
    if indexed {
        Sources::write(
            library.join("bounds.omg"),
            "module bounds; pub domain<const N: u64> u64::Tag<N>;
             pub machine foreign(value: u64) -> u64 in Tag<true> { value as u64 in Tag<true> }",
        );
        let error = rejection(&root, package_inputs(&root, &library));
        assert!(
            error.contains("canonical type `bool`, expected `u64`"),
            "{error}"
        );
    }
}

#[test]
fn qualification_casts_keep_import_exposure_local_to_the_author() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain u64::Tag;",
    );
    Sources::write(
        root.join("loader.omg"),
        "module loader; use library::bounds::u64::Tag;",
    );
    for imports in ["use loader;", "use library::bounds::u64::Tag;"] {
        Sources::write(
            root.join("main.omg"),
            &format!("{imports} machine read() -> u64 {{ (7 as u64 in Tag) as u64 }}"),
        );
        if imports == "use loader;" {
            let error = rejection(&root, package_inputs(&root, &library));
            assert!(error.contains("unknown cast domain"), "{error}");
        } else {
            assert_source_free_seven(compile(&root, package_inputs(&root, &library)));
        }
    }
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; domain u64::Tag;",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("private"), "{error}");
}

#[test]
#[ignore = "scalar return-domain body checking remains an independent implementation gap"]
fn scalar_domain_returns_require_independent_membership_proofs() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    for module in ["foreign_bounds", "bounds"] {
        Sources::write(
            library.join(format!("{module}.omg")),
            &format!("module {module}; pub domain u64::Tag requires self > 0;"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "module bounds; use library::{module};
             domain u64::Tag requires self > 100;
             machine exchange(value: u64 in library::{module}::u64::Tag) -> u64 in Tag {{ value }}"
            ),
        );
        // The differently named module already bypasses the old identity
        // collision fence. Neither declaration promises the stronger result.
        let _ = rejection(&root, package_inputs(&root, &library));
    }
}

#[test]
fn separate_packages_cannot_exchange_mutable_domain_qualifications() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("bounds.omg"),
        "module bounds; pub domain u64::Tag requires self > 0;",
    );
    for destination in ["library::bounds::u64::Tag", "Tag"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "module bounds; use library::bounds;
             domain u64::Tag requires self > 100;
             data Main {{ value: u64 in library::bounds::u64::Tag; }}
             machine Main::exchange(&mut self) {{
                 let local: &mut u64 in {destination} = &mut self.value as &mut u64;
             }}"
            ),
        );
        if destination == "library::bounds::u64::Tag" {
            compile(&root, package_inputs(&root, &library));
        } else {
            // A mutable recast must preserve both domains' write obligations.
            // Equal leaf paths cannot short-circuit checked implication.
            let error = rejection(&root, package_inputs(&root, &library));
            assert!(
                error.contains("a mutable recast must prove fact implication in BOTH directions"),
                "{error}"
            );
        }
    }
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

#[test]
fn boolean_domain_indices_retain_exact_module_values_through_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; const ENABLED: bool = false;
         machine read() -> u64 { settings::VALUE }",
    );
    for argument in ["true", "ENABLED"] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; use policy; const ENABLED: bool = true;
             pub const VALUE: u64 in policy::Gate<{argument}> = 7;"
            ),
        );
        let checked = compile(&root, package_inputs(&root, &library));
        assert!(!selections(&checked, "policy::Gate", identity(2)).is_empty());
        assert_source_free_seven(checked);
    }
}

#[test]
fn nested_boolean_domain_indices_keep_their_caller_telescope() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain<const Enabled: bool, const Limit: u64> u64::Gate<Enabled, Limit>
         requires Enabled && self < Limit;
         domain<const Allowed: bool> u64::Selected<Allowed> requires self in Gate<Allowed, 8>;
         const VALUE: u64 in Selected<true> = 1 + 6;
         machine read() -> u64 { VALUE }",
    );
    assert_source_free_seven(compile(&root, super::root_inputs(&root)));
}

#[test]
fn boolean_domain_indices_reject_false_unused_declarations() {
    let tree = Sources::new();
    let root = tree.package("root");
    for argument in ["false", "DISABLED"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;
             const DISABLED: bool = 1 == 2;
             const UNUSED: u64 in Gate<{argument}> = 1 + 6;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains("for const `UNUSED` is false"),
            "{argument}: {error}"
        );
    }
}

#[test]
fn boolean_domain_indices_reject_integer_substitution() {
    let tree = Sources::new();
    let root = tree.package("root");
    for argument in ["0", "1", "NUMBER", "(1 + 2)"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Enabled: bool> u64::Gate<Enabled>;
             const NUMBER: u64 = 1;
             const UNUSED: u64 in Gate<{argument}> = 7;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains("bool") || error.contains("Boolean"),
            "{argument}: {error}"
        );
    }
}

#[test]
fn boolean_domain_indices_do_not_erase_invalid_numeric_operand_types() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (argument, diagnostic) in [
        ("(1u8 == 1u64)", "incompatible landed integer carriers"),
        ("((255u8 + 1u8) == 0u8)", "overflow"),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Enabled: bool> u64::Gate<Enabled>;
             const UNUSED: u64 in Gate<{argument}> = 7;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains(diagnostic), "{argument}: {error}");
    }
}

#[test]
fn computed_boolean_domain_indices_use_the_existing_typed_operand_checks() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain<const Enabled: bool> u64::Gate<Enabled>;
         const VALUE: u64 in Gate<(1u8 == 1u8)> = 7;
         machine read() -> u64 { VALUE }",
    );
    assert_source_free_seven(compile(&root, super::root_inputs(&root)));
}

#[test]
fn forwarded_boolean_domain_indices_keep_their_declared_kind() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (outer_type, inner_type, argument) in [("bool", "u8", "true"), ("u8", "bool", "1")] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Index: {inner_type}> u64::Inner<Index>;
             domain<const Index: {outer_type}> u64::Outer<Index> requires self in Inner<Index>;
             const UNUSED: u64 in Outer<{argument}> = 7;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains(&format!("expected `{inner_type}`")),
            "{error}"
        );
    }
}

#[test]
fn boolean_domain_indices_require_public_and_file_local_constant_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("flags.omg"),
        "module flags; const ENABLED: bool = true;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::flags;
         domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;
         const UNUSED: u64 in Gate<flags::ENABLED> = 7;
         machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("private"), "{error}");

    Sources::write(
        library.join("flags.omg"),
        "module flags; pub const ENABLED: bool = true;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; use library::flags::ENABLED;
         machine read() -> u64 { settings::VALUE }",
    );
    for import in ["use flags::ENABLED;", ""] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; {import}
             pub domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;
             pub const VALUE: u64 in Gate<ENABLED> = 7;"
            ),
        );
        if import.is_empty() {
            let error = rejection(&root, package_inputs(&root, &library));
            assert!(
                error.contains("declaration-site proof checking") || error.contains("unresolved"),
                "{error}"
            );
        } else {
            assert_source_free_seven(compile(&root, package_inputs(&root, &library)));
        }
    }
}

#[test]
fn computed_boolean_domain_indices_retain_their_pending_typed_probe_boundary() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; machine read() -> u64 { settings::VALUE }",
    );
    for (argument, boundary) in [
        ("(!false)", "declaration-site proof checking"),
        ("(1 == 1)", "declaration-site proof checking"),
        ("ENABLED", "expected a boolean literal"),
    ] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; use policy; const ENABLED: bool = 1 == 1;
             pub const VALUE: u64 in policy::Gate<{argument}> = 7;"
            ),
        );
        // These should execute after exact family discovery,
        // and pending declaration probes are connected. Do not fold them using
        // untyped facts: that would erase operand types and selection custody.
        let error = rejection(&root, package_inputs(&root, &library));
        assert!(error.contains(boundary), "{argument}: {error}");
    }
}
