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
fn generic_carrier_aliases_retain_constituent_bindings_through_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    for declaration in [
        "domain<U> U::Marked; domain<T> T::Alias = T::Marked;",
        "domain<U> U::T; domain<T> T::Alias = T::T;",
        "domain<U> U::Marked requires true; domain<V> V::Both = V::Marked;
         domain<T> T::Alias = T::Both & T::Marked;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{declaration} const VALUE: u64 in Alias = 7;
                      machine read() -> u64 {{ VALUE as u64 }}"
            ),
        );
        assert_source_free_seven(compile(&root, super::root_inputs(&root)));
    }
}

#[test]
fn generic_carrier_aliases_select_constituents_in_the_authors_package() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<U> U::Marked requires true;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use policy;
         pub domain<T> T::Alias = T::Marked;
         pub const VALUE: u64 in Alias = 7;",
    );
    Sources::write(
        root.join("policy.omg"),
        "module policy; pub domain<U> U::Marked requires false;",
    );
    Sources::write(
        root.join("main.omg"),
        "use policy; use library::settings;
         machine read() -> u64 { settings::VALUE as u64 }",
    );
    assert_source_free_seven(compile(&root, package_inputs(&root, &library)));

    // An import by another file is not exposure for the alias author.
    Sources::write(
        library.join("settings.omg"),
        "module settings; pub domain<T> T::Alias = T::Marked;
         pub const VALUE: u64 in Alias = 7;",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("does not select one exposed"), "{error}");
}

#[test]
fn generic_carrier_aliases_do_not_erase_application_obligations() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declaration, expected) in [
        (
            "domain<U [copy]> U::Marked; domain<T> T::Alias = T::Marked;",
            "carrier requires `copy`",
        ),
        (
            "domain<U> U::Marked; domain<T [copy]> T::Alias = T::Marked;",
            "typed carrier-bound application evidence",
        ),
        (
            "domain<U> U::Marked; domain<T, const N: u64> T::Alias<N> = T::Marked;",
            "typed index-application evidence",
        ),
        (
            "domain<U, const N: u64> U::Marked<N>; domain<T> T::Alias = T::Marked;",
            "supplies no index arguments",
        ),
        (
            "domain<U> U::Marked; domain<T, V> T::Alias = V::Marked;",
            "must name this alias's carrier type binder",
        ),
        (
            "domain<U> U::Marked; domain<const T: u64> u64::Alias<T> = T::Marked;",
            "must name this alias's carrier type binder",
        ),
        (
            "domain<U> U::Marked; pub domain<T> T::Alias = T::Marked;",
            "private constituent",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{declaration} machine read() -> u64 {{ 7 }}"),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains(expected), "{declaration}: {error}");
    }

    Sources::write(
        root.join("main.omg"),
        "domain<U> U::Marked requires false; domain<T> T::Alias = T::Marked;
         const UNUSED: u64 in Alias = 7; machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(error.contains("failed domain `Marked`"), "{error}");
}

#[test]
fn generic_alias_carrier_prefix_cannot_select_a_same_named_module() {
    let tree = Sources::new();
    let root = tree.package("root");
    std::fs::create_dir(root.join("T")).unwrap();
    Sources::write(
        root.join("T/nested.omg"),
        "module T::nested; pub domain<U> U::Marked;",
    );
    Sources::write(
        root.join("main.omg"),
        "use T::nested; domain<T> T::Alias = T::nested::Marked;
         const VALUE: u64 in Alias = 7; machine read() -> u64 { VALUE as u64 }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(error.contains("carrier type binder"), "{error}");
}

#[test]
fn scalar_constant_aliases_discharge_every_constituent_through_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy;
         pub domain u64::Positive requires self > 0;
         pub domain u64::Small requires self < 10;
         pub domain u64::Both = u64::Positive & u64::Small;
         pub domain u64::Alias = u64::Both & u64::Positive;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use policy; pub const VALUE: u64 in policy::u64::Alias = 7;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; machine read() -> u64 { settings::VALUE as u64 }",
    );
    assert_source_free_seven(compile(&root, package_inputs(&root, &library)));

    // Equal logical alias paths in two packages are distinct declarations,
    // not a cycle. The root alias selects the library's entire conjunction.
    Sources::write(
        root.join("policy.omg"),
        "module policy; use library::policy;
         pub domain u64::Alias = library::policy::u64::Alias;
         pub const VALUE: u64 in Alias = 7;",
    );
    Sources::write(
        root.join("main.omg"),
        "use policy; machine read() -> u64 { policy::VALUE as u64 }",
    );
    assert_source_free_seven(compile(&root, package_inputs(&root, &library)));
}

#[test]
fn scalar_constant_aliases_reject_false_and_unestablished_constituents() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declaration, expected) in [
        (
            "domain<T> T::Alias = u64::Atom; domain u64::Atom;",
            "classify different types",
        ),
        (
            "domain u64::Atom; pub domain u64::Alias = u64::Atom;",
            "private constituent",
        ),
        (
            "domain u64::Atom requires self < 7; domain u64::Alias = u64::Atom;",
            "Atom",
        ),
        (
            "domain u32::Atom; domain u64::Alias = u32::Atom;",
            "carrier",
        ),
        (
            "domain u64::Atom established by Issuer::issue; domain u64::Alias = u64::Atom;
          trait Issuer { machine issue() -> u64 in Atom; }",
            "declaration-site proof checking",
        ),
        (
            "domain u64::Atom = u64::Alias; domain u64::Alias = u64::Atom;",
            "declaration-site proof checking",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{declaration} const UNUSED: u64 in Alias = 7; machine read() -> u64 {{ 7 }}"),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains(expected), "{declaration}: {error}");
    }
}

#[test]
fn scalar_constant_aliases_cannot_select_private_foreign_constituents() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; domain u64::Atom;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::policy; domain u64::Alias = library::policy::u64::Atom;
         const UNUSED: u64 in Alias = 7; machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(
        error.contains("selects private domain `policy::u64::Atom`"),
        "{error}"
    );
}

#[test]
fn false_alias_membership_retains_ordinary_boolean_meaning() {
    let tree = Sources::new();
    let root = tree.package("root");
    for fact in ["(self in Alias) == false", "(self in Alias) || true"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Never requires false; domain u64::Alias = u64::Never;
             domain u64::Gate requires {fact}; const VALUE: u64 in Gate = 7;
             machine read() -> u64 {{ VALUE as u64 }}"
            ),
        );
        assert_source_free_seven(compile(&root, super::root_inputs(&root)));
    }
}

#[test]
fn scalar_constants_in_carrier_polymorphic_domains_reach_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    for (declaration, qualification, selected) in [
        (
            "pub domain<T> T::Marked;",
            "policy::Marked",
            "policy::Marked",
        ),
        (
            "pub domain<T, const Enabled: bool> T::Gate<Enabled> requires Enabled;",
            "policy::Gate<true>",
            "policy::Gate",
        ),
    ] {
        Sources::write(
            library.join("policy.omg"),
            &format!("module policy; {declaration}"),
        );
        Sources::write(
            library.join("settings.omg"),
            &format!("module settings; use policy; pub const VALUE: u64 in {qualification} = 7;"),
        );
        Sources::write(
            root.join("main.omg"),
            "use library::settings; machine read() -> u64 { settings::VALUE as u64 }",
        );
        let checked = compile(&root, package_inputs(&root, &library));
        assert!(!selections(&checked, selected, identity(2)).is_empty());
        assert_source_free_seven(checked);
    }
}

#[test]
fn unused_carrier_polymorphic_constants_check_the_complete_application() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declaration, qualification, expected) in [
        (
            "domain<T, const Enabled: bool> T::Gate<Enabled> requires Enabled;",
            "Gate<false>",
            "for const `UNUSED` is false",
        ),
        (
            "domain<T, const Enabled: bool> T::Gate<Enabled> requires !Enabled;",
            "Gate<true>",
            "for const `UNUSED` is false",
        ),
        (
            "domain<T, const Enabled: bool> T::Gate<Enabled>;",
            "Gate",
            "requires 1 closed index argument(s), but 0 were supplied",
        ),
        (
            "domain<T, const Enabled: bool> T::Gate<Enabled>;",
            "Gate<true, false>",
            "requires 1 closed index argument(s), but 2 were supplied",
        ),
        (
            "domain<T, const Count: u8> T::Gate<Count>;",
            "Gate<256>",
            "does not fit `u8`",
        ),
        (
            "domain<T, const Count: u8> T::Gate<Count>;",
            "Gate<true>",
            "canonical type `bool`, expected `u8`",
        ),
        (
            "domain<T [linear]> T::Gate;",
            "Gate",
            "declaration-site proof checking",
        ),
        (
            "domain<const Enabled: bool> u32::Gate<Enabled> requires Enabled;",
            "u32::Gate<true>",
            "const value has carrier `u64`",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{declaration} const UNUSED: u64 in {qualification} = 7;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains(expected),
            "{declaration} {qualification}: {error}"
        );
    }
}

#[test]
fn carrier_polymorphic_integer_facts_keep_declared_width_and_nested_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (fact, argument, expected) in [
        ("N + 1 > N", "254", None),
        ("N + 1u8 > N", "254", None),
        ("ROOT_N + 1 > ROOT_N", "254", None),
        (
            "ROOT_N + 1 > ROOT_N",
            "255",
            Some("outside the declared `u8` range"),
        ),
        ("N + 1 > N", "255", Some("outside the declared `u8` range")),
        ("N - 1 < N", "0", Some("outside the declared `u8` range")),
        ("(N << 7) == 128", "1", None),
        ("(N << 1u64) == 2u8", "1", None),
        ("(N << 8) == 256", "1", Some("declared `u8` width")),
        ("N in u8::Allowed", "7", None),
        (
            "N in u64::Allowed",
            "7",
            Some("const value has carrier `u8`"),
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "const ROOT_N: u8 = {argument};
             domain u8::Allowed; domain u64::Allowed;
             domain<T, const N: u8> T::Gate<N> requires {fact};
             const VALUE: u64 in Gate<{argument}> = 7;
             machine read() -> u64 {{ VALUE as u64 }}"
            ),
        );
        if let Some(expected) = expected {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(error.contains(expected), "{fact}, N={argument}: {error}");
        } else {
            assert_source_free_seven(compile(&root, super::root_inputs(&root)));
        }
    }
}

#[test]
fn domain_membership_subjects_retain_literal_and_computed_types() {
    let tree = Sources::new();
    let root = tree.package("root");
    for fact in [
        "7u8 in u8::Allowed",
        "(7u8 + 1) in u8::Allowed",
        "true in bool::Allowed",
        "(7u8 < 8) in bool::Allowed",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u8::Allowed; domain bool::Allowed;
             domain u8::Gate requires {fact};
             machine read() -> u64 {{ 7 }}"
            ),
        );
        assert_source_free_seven(compile(&root, super::root_inputs(&root)));
    }
}

#[test]
fn scalar_constant_predicates_cannot_mint_routed_or_aliased_authority() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declaration, expected) in [
        (
            "domain<T> T::Issued established by Issuer::issue;",
            "declaration-site proof checking",
        ),
        (
            "domain<T> T::Issued requires true\n established by Issuer::issue;",
            "declaration-site proof checking",
        ),
        (
            "domain u64::Issued established by Issuer::issue;",
            "declaration-site proof checking",
        ),
        (
            "domain<T> T::Never requires false; domain<T> T::Issued = T::Never;",
            "failed domain `Never`",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{declaration}
             trait Issuer {{ machine issue() -> u64 in Issued; }}
             const UNUSED: u64 in Issued = 7;
             machine read() -> u64 {{ 7 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(error.contains(expected), "{declaration}: {error}");
    }
}

#[test]
fn carrier_polymorphic_constant_domain_selection_stays_source_and_package_owned() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<T> T::Gate requires true;",
    );
    Sources::write(
        root.join("policy.omg"),
        "module policy; pub domain<T> T::Gate requires false;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use policy; pub const VALUE: u64 in policy::Gate = 7;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; use policy;
         machine read() -> u64 { settings::VALUE as u64 }",
    );
    let checked = compile(&root, package_inputs(&root, &library));
    assert!(!selections(&checked, "policy::Gate", identity(2)).is_empty());
    assert_source_free_seven(checked);

    Sources::write(
        library.join("policy.omg"),
        "module policy; domain<T> T::Gate requires true;",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("private"), "{error}");

    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<T> T::Gate requires true;",
    );
    Sources::write(
        library.join("settings.omg"),
        "module settings; use policy; pub machine value() -> u64 { 7 }",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings;
         const UNUSED: u64 in u64::Gate = 7;
         machine read() -> u64 { 7 }",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(error.contains("declaration-site proof checking"), "{error}");
}

#[test]
fn generic_carrier_qualified_fields_keep_their_owner_after_specialization() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("tags.omg"),
        "module tags; pub domain<T> T::Marked;",
    );
    Sources::write(
        library.join("holders.omg"),
        "module holders; use tags; pub data Holder<T [copy]> [copy] { value: T in T::Marked; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::holders; use library::tags;
         machine read() -> u64 {
             let holder: holders::Holder<u64> = holders::Holder {
                 value: 7
             };
             holder.value as u64
         }",
    );
    let checked = compile(&root, package_inputs(&root, &library));
    assert!(!selections(&checked, "tags::Marked", identity(2)).is_empty());
    // The runtime record-local route additionally needs structural lowering.
    // Constant field projection already has a source-independent scalar route.
    Sources::write(
        library.join("settings.omg"),
        "module settings; use holders;
         pub const SELECTED: holders::Holder<u64> = holders::Holder { value: 7 };",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; machine read() -> u64 { settings::SELECTED.value as u64 }",
    );
    assert_source_free_seven(compile(&root, package_inputs(&root, &library)));
}

#[test]
fn generic_carrier_domain_keeps_the_exposed_package_when_logical_paths_collide() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    for package in [&root, &library] {
        Sources::write(
            package.join("tags.omg"),
            "module tags; pub domain<Element> Element::Marked;",
        );
    }
    Sources::write(
        root.join("main.omg"),
        "use holders; use tags; machine read() -> u64 { 7 }",
    );
    for import in ["library::tags", "library::tags::Marked"] {
        Sources::write(
            root.join("holders.omg"),
            &format!(
                "module holders; use {import};
             pub data Holder<T> {{ value: T in T::Marked; }}"
            ),
        );
        let checked = compile(&root, package_inputs(&root, &library));
        assert!(!selections(&checked, "tags::Marked", identity(2)).is_empty());
        assert!(selections(&checked, "tags::Marked", identity(1)).is_empty());
    }
    Sources::write(
        library.join("tags.omg"),
        "module tags; domain<Element> Element::Marked;",
    );
    let error = rejection(&root, package_inputs(&root, &library));
    assert!(
        error.contains("public interface selects private domain `tags::Marked`"),
        "{error}"
    );
}

#[test]
fn generic_carrier_qualification_survives_array_specialization() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain<T> T::Marked;
         data Holder<T [copy]> [copy] { value: T in T::Marked; }
         const SELECTED: Holder<[u64; 1]> = Holder { value: [7] };
         machine read() -> u64 { 7 }",
    );
    let checked = compile(&root, super::root_inputs(&root));
    let instance = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str().starts_with("Holder<"))
        .expect("closed array instance");
    let typed_trees::data::DataMember::Field(field) = &checked.typed.data_members(instance)[0]
    else {
        panic!("qualified field");
    };
    let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } = checked
        .typed
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        panic!("field retains qualification");
    };
    assert!(matches!(
        checked
            .typed
            .type_reference_table
            .type_reference(*base_type),
        typed_trees::types::TypeReferenceNode::FixedArray { .. }
    ));
}

#[test]
fn qualified_record_projection_rejects_unproven_constructor_membership() {
    let tree = Sources::new();
    let root = tree.package("root");
    for selected_field in ["value", "other"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<T> T::Marked requires false;
             data Config<T [copy]> [copy] {{ value: T in T::Marked; other: u64; }}
             const CONFIG: Config<u64> = Config {{ value: 9, other: 7 }};
             machine read() -> u64 {{ CONFIG.{selected_field} as u64 }}"
            ),
        );
        let error = rejection(&root, super::root_inputs(&root));
        assert!(
            error.contains("Marked") && error.contains("not proven"),
            "{error}"
        );
    }
}

#[test]
fn generic_carrier_domain_selection_is_lexical_and_requires_exposure() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("tags.omg"),
        "module tags; pub domain<Element> Element::Marked;",
    );
    Sources::write(
        library.join("other.omg"),
        "module other; pub domain<Element> Element::Marked;",
    );
    // Loading a module named T must never reinterpret a lexical T prefix.
    Sources::write(
        library.join("T.omg"),
        "module T; pub domain<Element> Element::Marked;",
    );
    Sources::write(library.join("relay.omg"), "module relay; use tags;");
    // A sibling file's imports cannot expose a domain in holders.omg.
    Sources::write(library.join("sibling.omg"), "module holders; use tags;");
    Sources::write(
        root.join("main.omg"),
        "use library::holders; use library::tags; use library::other; use library::T;
         machine read() -> u64 { 7 }",
    );
    for (imports, parameters, carrier, expected) in [
        (
            "",
            "T",
            "T",
            "does not select one exposed generic-carrier family",
        ),
        (
            "use relay;",
            "T",
            "T",
            "does not select one exposed generic-carrier family",
        ),
        (
            "use tags; use other;",
            "T",
            "T",
            "does not select one exposed generic-carrier family",
        ),
        (
            "use tags; pub domain<Element> Element::Marked;",
            "T",
            "T",
            "does not select one exposed generic-carrier family",
        ),
        (
            "use tags;",
            "T, U",
            "U",
            "must name this field's carrier type binder",
        ),
        (
            "use tags;",
            "const T: u64",
            "u64",
            "must name this field's carrier type binder",
        ),
    ] {
        Sources::write(
            library.join("holders.omg"),
            &format!(
                "module holders; {imports} pub data Holder<{parameters}> {{ value: {carrier} in T::Marked; }}"
            ),
        );
        let error = rejection(&root, package_inputs(&root, &library));
        assert!(
            error.contains(expected),
            "{imports}, {parameters}, {carrier}: {error}"
        );
    }
    // Both exact and declaring-module imports select the same family despite
    // unrelated same-leaf declarations, and binder names are not identities.
    for imports in ["use tags;", "use tags::Marked;"] {
        Sources::write(
            library.join("holders.omg"),
            &format!("module holders; {imports} pub data Holder<T> {{ value: T in T::Marked; }}"),
        );
        let checked = compile(&root, package_inputs(&root, &library));
        assert!(!selections(&checked, "tags::Marked", identity(2)).is_empty());
        assert!(selections(&checked, "other::Marked", identity(2)).is_empty());
        assert!(selections(&checked, "T::Marked", identity(2)).is_empty());
    }
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
        let error = rejection(&root, package_inputs(&root, &library));
        assert!(
            error.contains("cannot prove scalar result domain"),
            "{error}"
        );
    }
}

#[test]
fn scalar_domain_returns_preserve_membership_and_prove_stronger_predicates() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u64::Positive requires self > 0;
         domain u64::Large requires self > 100;
         machine preserve(value: u64 in Positive) -> u64 in Positive { value }
         machine strengthen(value: u64 in Positive) -> u64 in Large
             requires value > 100
         { value }
         machine weaken(value: u64 in Large) -> u64 in Positive { value }
         machine literal() -> u64 in Large { 101 }",
    );
    compile(&root, super::root_inputs(&root));
}

#[test]
fn scalar_domain_returns_keep_exact_indexed_membership() {
    let tree = Sources::new();
    let root = tree.package("root");
    for result_limit in [8, 4] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Limit: u64> u64::Below<Limit> requires self < Limit;
                 machine forward(value: u64 in Below<8>) -> u64 in Below<{result_limit}> {{ value }}"
            ),
        );
        if result_limit == 8 {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(error.contains("distinct normalized instances"), "{error}");
        }
    }
}

#[test]
fn scalar_domain_returns_establish_exact_indexed_predicates() {
    let tree = Sources::new();
    let root = tree.package("root");
    for limit in [8, 4] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Limit: u64> u64::Below<Limit> requires self < Limit;
                 machine literal() -> u64 in Below<{limit}> {{ 7 }}
                 machine bounded(value: u64) -> u64 in Below<{limit}>
                     requires value < 8
                 {{ value }}"
            ),
        );
        if limit == 8 {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_keep_index_binders_in_their_declaration_scope() {
    let tree = Sources::new();
    let root = tree.package("root");
    for lower in [6, 8] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain<const Limit: u64> u64::Below<Limit> requires self < Limit;
                 domain<const Limit: u64> u64::Above<Limit> requires self > Limit;
                 machine below() -> u64 in Below<8> {{ 7 }}
                 machine above() -> u64 in Above<{lower}> {{ 7 }}"
            ),
        );
        if lower == 6 {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_check_the_body_behind_a_call_result_promise() {
    let tree = Sources::new();
    let root = tree.package("root");
    for value in [7, 0] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Positive requires self > 0;
                 machine producer() -> u64 in Positive {{ {value} }}
                 machine forward() -> u64 in Positive {{ producer() }}"
            ),
        );
        if value == 7 {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
            assert!(error.contains("producer"), "{error}");
        }
    }
}

#[test]
fn scalar_domain_returns_check_each_state_and_branch_exit() {
    let tree = Sources::new();
    let root = tree.package("root");
    for fallback in [7, 0] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Positive requires self > 0;
                 machine choose(selected: bool) -> u64 in Positive {{
                     transition selected {{ true -> later() false -> {fallback} }}
                     state later() -> u64 in Positive {{ 7 }}
                 }}"
            ),
        );
        if fallback == 7 {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_preserve_wrapping_arithmetic_meaning() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u8::Positive requires self > 0;
         machine keep(value: u8 in Wrapping) -> u8 in Wrapping & Positive
             requires value > 0
         { value }",
    );
    compile(&root, super::root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        "domain u8::Positive requires self > 0;
         machine wrap(value: u8 in Wrapping) -> u8 in Wrapping & Positive {
             value + 1
         }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(
        error.contains("cannot prove scalar result domain"),
        "{error}"
    );
}

#[test]
fn scalar_domain_returns_check_a_states_own_result_qualification() {
    let tree = Sources::new();
    let root = tree.package("root");
    for value in [7, 0] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Positive requires self > 0;
                 machine outer() -> u64 {{
                     transition {{ _ -> inner() }}
                     state inner() -> u64 in Positive {{ {value} }}
                 }}"
            ),
        );
        if value == 7 {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_do_not_negate_one_part_of_a_denied_conjunction() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u64::Positive requires self > 0;
         machine choose(value: u64) -> u64 in Positive requires value > 0 {
             transition value > 0 && false { true -> 1 false -> 0 }
         }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(
        error.contains("cannot prove scalar result domain"),
        "{error}"
    );
}

#[test]
fn scalar_domain_returns_require_membership_from_named_tail_calls() {
    let tree = Sources::new();
    let root = tree.package("root");
    for qualified in [true, false] {
        let result = if qualified { "u64 in Positive" } else { "u64" };
        let value = if qualified { 7 } else { 0 };
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Positive requires self > 0;
                 machine producer() -> {result} {{ {value} }}
                 machine outer() -> u64 in Positive {{ transition {{ _ -> producer() }} }}"
            ),
        );
        if qualified {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_preserve_state_obligations_across_internal_transfers() {
    let tree = Sources::new();
    let root = tree.package("root");
    // The common machine promise is proved at the eventual exit; internal
    // destinations need not repeat it on every state signature.
    Sources::write(
        root.join("main.omg"),
        "domain u64::Positive requires self > 0;
         machine outer() -> u64 in Positive {
             transition { _ -> inner() }
             state inner() -> u64 { 7 }
         }",
    );
    compile(&root, super::root_inputs(&root));
    for qualified in [true, false] {
        let result = if qualified { "u64 in Positive" } else { "u64" };
        let value = if qualified { 7 } else { 0 };
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Positive requires self > 0;
                 machine outer() -> u64 {{
                     transition {{ _ -> inner() }}
                     state inner() -> u64 in Positive {{ transition {{ _ -> last() }} }}
                     state last() -> {result} {{ {value} }}
                 }}"
            ),
        );
        if qualified {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_preserve_independently_checked_casts() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u64::Positive requires self > 0;
         machine literal() -> u64 in Positive { 7 as u64 in Positive }
         machine bounded(value: u64) -> u64 in Positive
             requires value > 0
         { value as u64 in Positive }",
    );
    compile(&root, super::root_inputs(&root));
}

#[test]
fn scalar_domain_returns_require_routed_provenance() {
    let tree = Sources::new();
    let root = tree.package("root");
    for body in ["value", "7"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain u64::Issued established by Issuer::issue;
                 trait Issuer {{ machine issue() -> u64 in Issued; }}
                 machine forward(value: u64 in Issued) -> u64 in Issued {{ {body} }}"
            ),
        );
        if body == "value" {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
    }
}

#[test]
fn scalar_domain_returns_preserve_authorized_issuance_and_call_results() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u64::Issued established by Issuer::issue;
         trait Issuer { machine issue() -> u64 in Issued; }
         data Factory {}
         machine Factory::issue() -> u64 in Issued satisfies Issuer::issue { 7 }
         machine forward() -> u64 in Issued { Factory::issue() }",
    );
    compile(&root, super::root_inputs(&root));
}

#[test]
fn scalar_domain_returns_do_not_reuse_invalidated_predicates() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u64::Positive requires self > 0;
         machine reset(mut value: u64) -> u64 in Positive
             requires value > 0
         { value = 0; value }",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(
        error.contains("cannot prove scalar result domain"),
        "{error}"
    );
}

#[test]
fn scalar_domain_returns_require_a_returned_subject() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "domain u64::Positive requires self > 0;
         machine empty() -> u64 in Positive {}",
    );
    let error = rejection(&root, super::root_inputs(&root));
    assert!(error.contains("its body is empty"), "{error}");
}

#[test]
fn scalar_domain_returns_prove_boolean_predicates() {
    let tree = Sources::new();
    let root = tree.package("root");
    for value in ["true", "false"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "domain bool::Truthy requires self;
                 machine boolean() -> bool in Truthy {{ {value} }}"
            ),
        );
        if value == "true" {
            compile(&root, super::root_inputs(&root));
        } else {
            let error = rejection(&root, super::root_inputs(&root));
            assert!(
                error.contains("cannot prove scalar result domain"),
                "{error}"
            );
        }
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
    .expect_err("invalid domain-qualified source must reject")
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
        "module flags; const ENABLED: bool = !false;",
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
fn computed_boolean_domain_indices_reach_source_free_terminal() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings;
         domain<const Enabled: u8> u64::Gate<Enabled>;
         machine read() -> u64 { settings::VALUE }",
    );
    for (argument, initializer) in [
        ("(!false)", "1 == 1"),
        ("(1 == 1)", "1 == 1"),
        ("ENABLED", "1 == 1"),
        ("ENABLED", "!false"),
        ("ENABLED", "!(1u8 != 1u8)"),
    ] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; use policy; const ENABLED: bool = {initializer};
             pub const VALUE: u64 in policy::Gate<{argument}> = 7;"
            ),
        );
        assert_source_free_seven(compile(&root, package_inputs(&root, &library)));
    }
}

#[test]
fn computed_boolean_indices_compose_with_other_const_qualifications() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;
         pub domain u64::Small requires self < 8;
         pub domain<const Limit: u64> u64::Below<Limit> requires self < Limit;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; machine read() -> u64 { settings::RESULT }",
    );
    // Nongeneric attachments keep the carrier in their exact address; the
    // indexed family has a leaf-named declaration. An invalid Small address
    // would reject selection before testing either predicate.
    for qualification in [
        "policy::u64::Small",
        "policy::Gate<true> & policy::u64::Small",
        "policy::Gate<ENABLED> & policy::u64::Small",
        "policy::u64::Small & policy::Gate<(!false)>",
        "policy::Gate<ENABLED> & policy::Below<(4 + 4)>",
    ] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; use policy; const ENABLED: bool = !false;
             pub const VALUE: u64 in {qualification} = 7;
             pub const RESULT: u64 = VALUE + 0;"
            ),
        );
        assert_source_free_seven(compile(&root, package_inputs(&root, &library)));
    }
}

#[test]
fn computed_boolean_domain_indices_do_not_publish_placeholder_membership() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("policy.omg"),
        "module policy; pub domain<const Enabled: bool> u64::Gate<Enabled> requires Enabled;
         pub domain u64::Small requires self < 3;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; machine read() -> u64 { settings::RESULT }",
    );
    for (initializer, argument, extra, expected) in [
        ("!true", "ENABLED", "", "for const `VALUE` is false"),
        ("!false", "(!true)", "", "for const `VALUE` is false"),
        (
            "!false",
            "ENABLED",
            " & policy::u64::Small",
            "domain constraint `policy::u64::Small` for const `VALUE` is false",
        ),
        (
            "1u8 == 1u64",
            "ENABLED",
            "",
            "incompatible landed integer carriers",
        ),
        ("(255u8 + 1u8) == 0u8", "ENABLED", "", "overflow"),
    ] {
        Sources::write(
            library.join("settings.omg"),
            &format!(
                "module settings; use policy; const ENABLED: bool = {initializer};
                 pub const VALUE: u64 in policy::Gate<{argument}>{extra} = 7;
                 pub const RESULT: u64 = VALUE + 0;"
            ),
        );
        let error = rejection(&root, package_inputs(&root, &library));
        assert!(
            error.contains(expected),
            "{initializer}, {argument}, {extra}: {error}"
        );
    }
}
