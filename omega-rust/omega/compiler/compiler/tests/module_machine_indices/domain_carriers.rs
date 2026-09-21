//! Foreign domains select carrier identity independently of its authored spelling.

use super::{Sources, compile, identity, selections};
use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use std::path::Path;

fn inputs(root: &Path, library: &Path, other: Option<&Path>) -> PackageCompilationInputs {
    let mut packages = vec![
        PackageSourceBinding::new(identity(1), "root", root.to_path_buf()),
        PackageSourceBinding::new(identity(2), "library", library.to_path_buf()),
    ];
    let mut dependencies = vec![PackageDependencyBinding::new(
        identity(1),
        "library",
        identity(2),
    )];
    if let Some(other) = other {
        packages.push(PackageSourceBinding::new(
            identity(3),
            "other",
            other.to_path_buf(),
        ));
        dependencies.push(PackageDependencyBinding::new(
            identity(1),
            "other",
            identity(3),
        ));
    }
    PackageCompilationInputs::new_package(identity(1), packages, dependencies).unwrap()
}

#[test]
fn foreign_case_domain_retains_its_exact_carrier_and_predicate_on_formals() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    Sources::write(
        library.join("settings.omg"),
        "module settings; pub data Choice [copy] { case Empty; case Some(value: u32); }",
    );
    Sources::write(
        library.join("policy.omg"),
        "module policy; use settings::Choice;
         pub domain Choice::NonEmpty requires self in settings::Choice::Some;",
    );
    Sources::write(
        root.join("main.omg"),
        "use library::settings; use library::policy;
         data Choice [copy] { case Some(value: bool); case Empty; }
         machine payload(value: settings::Choice in policy::Choice::NonEmpty) -> u32 { value.value }",
    );
    let checked = compile(&root, inputs(&root, &library, None));
    assert!(!selections(&checked, "policy::Choice::NonEmpty", identity(2)).is_empty());
    let domain = checked
        .typed
        .domain_definitions()
        .iter()
        .find(|domain| {
            checked.symbols.display_path(domain.symbol, "::") == "policy::Choice::NonEmpty"
        })
        .expect("the exact foreign domain remains declared");
    assert_eq!(checked.typed.proof_facts(domain).len(), 1);
    assert!(domain.predicate_body.is_present());
    // This checks the hypothetical formal contract, not establishment by a
    // caller or a local annotation. Those obligations must not be inferred
    // merely from the successful carrier selection tested here.
}

#[test]
fn equal_module_paths_in_distinct_packages_do_not_share_a_domain_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    let library = tree.package("library");
    let other = tree.package("other");
    for package in [&library, &other] {
        Sources::write(
            package.join("settings.omg"),
            "module settings; pub data Point [copy] { value: u32; }",
        );
    }
    Sources::write(
        library.join("policy.omg"),
        "module policy; use settings::Point;
         pub domain Point::Selected;",
    );
    for (carrier, accepted) in [("library", true), ("other", false)] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use library::settings; use library::policy; use other::settings;
                 machine read(value: {carrier}::settings::Point in library::policy::Point::Selected) -> u32 {{ value.value }}"
            ),
        );
        if accepted {
            compile(&root, inputs(&root, &library, Some(&other)));
        } else {
            let diagnostics = compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(inputs(&root, &library, Some(&other))),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .expect_err("the other package's identically named carrier is not the domain target");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("domains are bound to their storage type")),
                "{diagnostics:?}"
            );
        }
    }
}
