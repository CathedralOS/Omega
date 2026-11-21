//! A library machine's definition obligations stay in the library's own typed forest.
//! `access\new_plan` is an ordinary checked core body. L3 records its four guarantees as
//! `ProveGuarantee` rows of that body, sited in the toolchain file and indexed in the
//! library forest (`toolchain_declarations/core_interface.rs`). No importing package
//! receives them, whether it calls the machine or not. A package that calls `new_plan`
//! depends on the definition through its call, which names the library declaration
//! (language/machines.md#call-preconditions: the defining machine establishes its
//! `ensures`, the caller establishes the `requires`).
//!
//! The consumer reads only the prepared package that `lower_package` receives. In the
//! access policy program, which really calls `new_plan`, it joins the call to the library
//! forest by declaration identity and finds the body and its open guarantees there. L5
//! publishes the constructor call when the domain omits its policy selection. A package
//! that never calls `new_plan`, and a checked `app` machine at the toolchain's
//! exact path, both lower with no foreign row, so the obligation counts the test reporter
//! publishes are their own rows, counted once. No Psi property is registered: none of these
//! rows is local to a lowered package. Checking the library body itself needs the toolchain
//! package to be prepared, lowered and checked, which no stage does yet.
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{CallTarget, LoweredPackage, OperationKind};
use omega_preparation::prepare_source_package;
use omega_typed::{ObligationKind, TypedTrees, TypingObligation};

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

/// Rows in the package's obligation list whose site is not one of its own files.
fn foreign_rows<'a>(typed: &'a TypedTrees<'_>) -> Vec<&'a TypingObligation> {
    foreign_in(typed, typed.obligations.iter())
}

fn foreign_in<'a>(
    typed: &TypedTrees<'_>,
    rows: impl Iterator<Item = &'a TypingObligation>,
) -> Vec<&'a TypingObligation> {
    let files: Vec<_> = typed
        .trees()
        .map(|tree| (tree.resolved().file.package, tree.resolved().file.path))
        .collect();
    rows.filter(|row| !files.contains(&(row.site.package.as_str(), row.site.file.as_str())))
        .collect()
}

/// What the package learns about one library machine it selects: the callers, whether the
/// declaration in the library forest has a body, and its open guarantee rows there.
#[derive(Debug, PartialEq)]
struct Selection {
    callers: Vec<String>,
    body: bool,
    guarantees: usize,
}

fn select(
    typed: &TypedTrees<'_>,
    machine: &omega_resolver::DeclarationId,
) -> Selection {
    use omega_parser::NodeKind::Body;
    let library = typed.library.expect("the standard profile admits the library");
    let signature = library
        .signatures
        .iter()
        .find(|s| &s.machine == machine)
        .unwrap();
    let tree = library.trees().nth(signature.node.file).unwrap();
    let guarantees: Vec<_> = library
        .obligations
        .iter()
        .filter(|o| matches!(&o.kind, ObligationKind::ProveGuarantee { machine: owner, .. } if owner == machine))
        .collect();
    for row in &guarantees {
        assert!(
            tree.semantics_at(row.node.node)
                .any(|fact| matches!(fact, omega_typed::NodeSemantics::Obligation(found) if found == *row))
        );
    }
    Selection {
        callers: typed
            .calls
            .iter()
            .filter(|call| &call.machine == machine)
            .map(|call| {
                call.caller
                    .as_ref()
                    .expect("a call in a machine body")
                    .path
                    .to_string()
            })
            .collect(),
        body: tree.resolved().file.child(signature.node.node, Body).is_some(),
        guarantees: guarantees.len(),
    }
}

/// Obligation kind counts the test reporter publishes, and the same counts over local rows.
fn counts(prepared: &omega_prepared::PreparedTrees<'_>) -> (std::collections::BTreeMap<&'static str, usize>, std::collections::BTreeMap<&'static str, usize>) {
    let typed = prepared.typed();
    let foreign = foreign_rows(typed);
    let mut local = std::collections::BTreeMap::new();
    for row in typed.obligations.iter().filter(|row| !foreign.contains(row)) {
        *local
            .entry(tree_handoff_consumer::obligation_report::name(&row.kind))
            .or_insert(0) += 1;
    }
    (tree_handoff_consumer::obligation_report::counts(prepared), local)
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
fn library_definition_rows_stay_in_the_library_forest_for_callers_and_non_callers() {
    let core = omega_toolchain::core_interface();
    let selected = core.id("access\\new_plan");
    let toolchain = (selected.package.as_str(), selected.path.as_str());

    // Selecting the constructor in the body is independent of selecting a policy for a domain.
    let implementation = ACCESS_PLAN.replace(" access Rule;", ";");
    let files = [("policy.omg", implementation.as_str())];
    let prepared = prepare_source_package(&sources::resolve(&files)).expect("the policy program prepares");
    let real = select(prepared.typed(), selected);
    assert!(foreign_rows(prepared.typed()).is_empty());
    let (reported, local) = counts(&prepared);
    assert_eq!(reported, local);
    let psi = sources::lower_prepared(&prepared, &files).expect("the ordinary imported call lowers");
    let plan = psi.machine_templates.iter().find(|template| template.declaration.path == "Rule::plan").unwrap();
    let call = psi_handoff_consumer::returned_template_call(&psi, plan).unwrap();
    assert_eq!(call.callee, psi_handoff_consumer::TemplateCallee::Provided { package: toolchain.0, path: toolchain.1 });
    drop(prepared);

    // A normal package that never calls new_plan: the same domain without its policy.
    let (prefix, _) = ACCESS_PLAN.split_once(" access Rule;").unwrap();
    let plain = format!("{prefix};");
    let files = [("policy.omg", plain.as_str())];
    let prepared = prepare_source_package(&sources::resolve(&files)).expect("the plain program prepares");
    let unused = select(prepared.typed(), selected);
    let (reported, local) = counts(&prepared);
    assert!(foreign_rows(prepared.typed()).is_empty());
    assert_eq!(reported, local, "the reporter counts each local row once");
    // Corrupt control: the baseline's import prepended a library row to every importer.
    // The same reader flags exactly that row.
    let typed = prepared.typed();
    let library_row = typed
        .library
        .unwrap()
        .obligations
        .iter()
        .find(|o| matches!(o.kind, ObligationKind::ProveGuarantee { .. }))
        .unwrap();
    let corrupted = std::iter::once(library_row).chain(typed.obligations.iter());
    assert_eq!(foreign_in(typed, corrupted), [library_row]);
    sources::lower_prepared(&prepared, &files).expect("the plain program lowers");
    drop(prepared);

    // The swapped identity: an `app` machine at the toolchain's path that a local caller selects.
    let local_machine = "public machine new_plan(x: u8) -> u8 { x }";
    let relay = "machine relay(y: u8) -> u8 { language\\core\\access\\new_plan(y) }";
    let files = [
        ("policy.omg", plain.as_str()),
        ("language/core/access/access.omg", local_machine),
        ("relay.omg", relay),
    ];
    let prepared = prepare_source_package(&sources::resolve(&files)).expect("the swapped program prepares");
    let swapped = select(prepared.typed(), selected);
    assert!(foreign_rows(prepared.typed()).is_empty());
    let lowered = sources::lower_prepared(&prepared, &files).expect("the swapped program lowers");
    // Same input, same product.
    assert_eq!(sources::lower_prepared(&prepared, &files).unwrap(), lowered);
    assert_eq!(declarations_named(&lowered, ("app", selected.path.as_str())), 1);
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

    spec_corpus::observe_handoff!(
        "library-definition-guarantees/consumer",
        assert_eq!(
            (&real, unused.callers.len(), swapped.callers.len(), declarations_named(&lowered, toolchain)),
            (
                &Selection {
                    callers: vec!["Rule::plan".to_owned()],
                    body: true,
                    guarantees: 4
                },
                0,
                0,
                0
            )
        ),
    );
    assert_eq!(unused, Selection { callers: vec![], ..real });
}
