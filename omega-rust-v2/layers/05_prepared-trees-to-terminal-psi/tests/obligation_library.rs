//! `VerifyLibraryImplementation` asks that the toolchain's implementation of a
//! bodyless library machine establish that machine's published guarantees. L3
//! records it on the library's own declaration of `access\new_plan`
//! (`toolchain_declarations/core_interface.rs`, `configure_roles`) and copies that
//! row into each importing package (`import`). Psi lowers only this package's
//! declarations, so the row has no receiver in the package's Psi. Whoever admits
//! and links the toolchain's implementation discharges it.
//!
//! The consumer below reads only the prepared package that `lower_package`
//! receives. In the access policy program, which really calls `new_plan`, it joins
//! that call to the row by declaration identity and finds the bodyless declaration
//! and its guarantees in the library forest. L5 stops that program at `access Rule`
//! (pinned in `formation_handoff.rs` and `domains.rs`), so it is not lowered again
//! here. The swapped identity is a checked `app` machine at the toolchain's exact
//! path `language\core\access\new_plan`. That package lowers. Its Psi contains the
//! local machine and the call to it, the row still names the toolchain's machine,
//! and no Psi declaration has that identity. L3 refuses a bodyless copy of the
//! declaration (`a_same_path_copy_in_another_package_cannot_acquire_library_supply`
//! in L3's `tests/standard.rs`). No Psi property is registered for the kind in
//! `tools/spec-corpus/l5_output_checks.tsv`: the row is never local to the
//! lowered package.
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{CallTarget, LoweredPackage, OperationKind};
use omega_preparation::prepare_source_package;
use omega_typed::{ObligationKind, Site, TypedTrees};

/// The access policy program of `formation_handoff.rs`, byte for byte.
const ACCESS_PLAN: &str = "use omega\\language\\core;
                  use omega\\language\\core\\reflect;
                  use omega\\language\\core\\access;
                  domain i32::Locked access Rule;
                  Rule satisfies AccessPolicy {
                      machine plan<'t>(schema:TypeSchema<'t>)->AccessPlan<'t> reaches none crashes none terminates {
                          new_plan::<'t>(@schema, Issuance::Unrestricted{})
                      }
                  }";

/// What a reader of L5's input learns from one imported implementation obligation.
#[derive(Clone, Debug, PartialEq)]
struct Reading {
    /// Package and path of the declaration the row names.
    subject: (String, String),
    site: Site,
    /// Whether the library declaration at the row's node has a body.
    bodyless: bool,
    /// `ensures` clauses on that declaration.
    guarantees: usize,
    /// Machines in this package whose calls select exactly the subject.
    callers: Vec<String>,
}

fn read(typed: &TypedTrees<'_>) -> Vec<Reading> {
    use omega_parser::NodeKind::{Body, Ensures};
    let library = typed.library.expect("the standard profile admits the library");
    typed
        .obligations
        .iter()
        .filter_map(|row| {
            let ObligationKind::VerifyLibraryImplementation(subject) = &row.kind else {
                return None;
            };
            let tree = library
                .trees()
                .nth(row.node.file)
                .expect("the row's node is in the library forest");
            let file = tree.resolved().file;
            let declared = tree
                .resolved()
                .declaration_nodes()
                .find(|(_, id)| *id == subject)
                .map(|(node, _)| node);
            assert_eq!(declared, Some(row.node.node), "the row's node declares its subject");
            Some(Reading {
                subject: (subject.package.to_string(), subject.path.to_string()),
                site: row.site.clone(),
                bodyless: file.child(row.node.node, Body).is_none(),
                guarantees: file.nodes[row.node.node]
                    .children
                    .iter()
                    .filter(|&&n| file.nodes[n].kind == Ensures)
                    .count(),
                callers: typed
                    .calls
                    .iter()
                    .filter(|call| &call.machine == subject)
                    .map(|call| {
                        call.caller
                            .as_ref()
                            .expect("a call in a machine body")
                            .path
                            .to_string()
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Psi declarations whose identity is `(package, path)`.
fn declarations_named(
    package: &LoweredPackage,
    (owner, path): (&str, &str),
) -> usize {
    let named = |p: &str, q: &str| p == owner && q == path;
    package
        .machines
        .iter()
        .filter(|m| named(m.declaration.package.as_str(), m.declaration.path.as_str()))
        .count()
        + package
            .machine_templates
            .iter()
            .filter(|t| named(t.declaration.package.as_str(), t.declaration.path.as_str()))
            .count()
}

#[test]
fn the_imported_obligation_names_the_selected_toolchain_constructor_not_a_same_path_local_machine() {
    let core = omega_toolchain::core_interface();
    let selected = core.id("access\\new_plan");
    let toolchain = (selected.package.as_str(), selected.path.as_str());
    let library_site = |reading: &Reading| (reading.site.package.clone(), reading.site.file.clone());

    // The real selection: Rule::plan calls the toolchain's bodyless constructor.
    let files = [("policy.omg", ACCESS_PLAN)];
    let package = sources::resolve(&files);
    let prepared = prepare_source_package(&package).expect("the policy program prepares");
    let real = read(prepared.typed());
    let [policy] = real.as_slice() else {
        panic!("expected one implementation obligation, found {real:?}")
    };
    assert_eq!((policy.subject.0.as_str(), policy.subject.1.as_str()), toolchain);
    assert_eq!(
        library_site(policy),
        (omega_toolchain::TOOLCHAIN_PACKAGE.to_owned(), "language/core/access/access.omg".to_owned())
    );
    assert!(policy.bodyless);
    assert_eq!(policy.guarantees, 4);
    drop(prepared);

    // The swapped identity: the same domain without its policy, and an `app` machine
    // at the toolchain's path that a local caller selects.
    let (prefix, _) = ACCESS_PLAN.split_once(" access Rule;").unwrap();
    let plain = format!("{prefix};");
    let local = "public machine new_plan(x: u8) -> u8 { x }";
    let relay = "machine relay(y: u8) -> u8 { language\\core\\access\\new_plan(y) }";
    let files = [
        ("policy.omg", plain.as_str()),
        ("language/core/access/access.omg", local),
        ("relay.omg", relay),
    ];
    let package = sources::resolve(&files);
    let prepared = prepare_source_package(&package).expect("the swapped program prepares");
    let swapped = read(prepared.typed());
    let lowered = sources::lower_prepared(&prepared, &files).expect("the swapped program lowers");
    let [row] = swapped.as_slice() else {
        panic!("expected only the imported obligation, found {swapped:?}")
    };

    // Psi holds the local machine and the call to it, under package `app`.
    let local_id = ("app", selected.path.as_str());
    assert_eq!(declarations_named(&lowered, local_id), 1);
    let target = lowered
        .machines
        .iter()
        .find(|m| m.declaration.package == "app" && m.declaration.path == selected.path.as_str())
        .unwrap();
    let caller = lowered
        .machines
        .iter()
        .find(|m| m.declaration.path == "relay")
        .unwrap();
    assert!(caller.body.operations.iter().any(|op| matches!(&op.kind,
        OperationKind::Call { callee: CallTarget::Machine(callee), .. } if *callee == target.id)));
    let called: Vec<_> = prepared
        .typed()
        .calls
        .iter()
        .map(|call| (call.machine.package.to_string(), call.machine.path.to_string()))
        .collect();
    assert_eq!(called, [("app".to_owned(), selected.path.to_string())]);

    spec_corpus::observe_handoff!(
        "library-implementation-obligation/consumer",
        assert_eq!(
            (
                &row.subject,
                library_site(row),
                row.callers.as_slice(),
                policy.callers.as_slice(),
                declarations_named(&lowered, toolchain)
            ),
            (&policy.subject, library_site(policy), &[][..], &["Rule::plan".to_owned()][..], 0)
        ),
    );
    assert_eq!(
        row,
        &Reading {
            callers: vec![],
            ..policy.clone()
        }
    );
}
