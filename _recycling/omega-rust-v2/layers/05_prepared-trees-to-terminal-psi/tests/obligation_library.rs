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
//! rows is local to a lowered package. The toolchain’s own verification unit lowers its checked constructor and identifies
//! each catalog-supplied declaration separately. The importing package never copies those bodies.
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
    let plan = psi.machines.iter().find(|machine| machine.declaration.path == "Rule::plan").unwrap();
    let [call] = plan.body.call_regions.as_slice() else { panic!("one selected constructor call") };
    let (callee, regions) = psi_handoff_consumer::applied_regions(&psi, plan, call.call).unwrap();
    assert_eq!((callee.declaration.package.as_str(), callee.declaration.path.as_str()), toolchain);
    assert_eq!(regions.iter().map(|region| region.ordinal).collect::<Vec<_>>(), [0]);
    assert_eq!(callee.contract.ensures.len(), real.guarantees);
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

#[test]
fn the_toolchain_defines_its_own_constructor_verification_unit() {
    let source = omega_typer::SourcePackage::toolchain_definition();
    let prepared = prepare_source_package(&source).expect("prepare the actual toolchain definition");
    let definition = omega_toolchain::core_interface().id(r"access\new_plan");
    assert!(prepared.typed().library.is_none());
    let guarantees = prepared.typed().obligations.iter().filter(|row| {
        matches!(&row.kind, ObligationKind::ProveGuarantee { machine, .. } if machine == definition)
    }).count();
    assert_eq!(guarantees, 4);

    let product = omega_lowering::lower_package(&prepared).expect("lower the toolchain");
    // The constructor is one closed body in its defining unit, with no open template beside it.
    let constructor = product.machines.iter().find(|machine| {
        machine.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && machine.declaration.path == definition.path
    }).expect("the defining unit owns the actual constructor body");
    assert!(product.machine_templates.iter().all(|template| template.declaration != constructor.declaration));
    assert_eq!(constructor.contract.ensures.len(), guarantees);
    let supplied: std::collections::BTreeSet<_> = prepared.typed().signatures.iter()
        .filter(|sig| omega_toolchain::core_interface().supplies(&sig.machine))
        .map(|sig| (&sig.machine.package, &sig.machine.path))
        .collect();
    assert_eq!(supplied.len(), 9);
    assert_eq!(
        product.catalog_declarations.iter().map(|decl| (&decl.package, &decl.path)).collect::<Vec<_>>(),
        supplied.into_iter().collect::<Vec<_>>()
    );
    assert!(!product.catalog_declarations.contains(&constructor.declaration));
    let client = sources::lower(&[("client.omg", "machine local(x:u8)->u8{x}")]).unwrap();
    assert!(client.catalog_declarations.is_empty());
}
