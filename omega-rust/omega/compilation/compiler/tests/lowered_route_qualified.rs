//! Data declarations are not machine namespaces. The reader must reject a
//! carrier-qualified declaration even when it has an explicit parameter and
//! its body could otherwise be checked (machines.md, "Static declarations
//! and explicit state"). Ordinary trait qualification keeps its own path.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

#[path = "support/qualified_operator_homes.rs"]
mod qualified_operator_homes;

use terminal_psi_to_terminal_psi::{RouteVerdict, judge};
use typed_trees_to_terminal_psi::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The route's verdict on everything the check of `fixture` claims, with
/// `entry` bound as its `build.omg` binds it.
fn route_verdict(fixture: &str, entry: &str) -> RouteVerdict {
    let frontier = unbuilt_frontier::bound_fixture_frontier(fixture, Some(entry), &[]);
    judge(elaborate(
        &frontier.typed,
        Compilation {
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            calling: frontier
                .normalization
                .calling
                .borrow(&frontier.typed)
                .selection(),
            providers: frontier.provider_selections(),
            bindings: &frontier.bindings,
            supplies: &[],
        },
    ))
    .unwrap_or_else(|defect| panic!("{fixture}: the route must reach a verdict: {defect:?}"))
}

/// Both bodies reached the checker before this retirement was enforced: the
/// first meets its callee's contract and the second breaks it. Neither body
/// gives a data declaration permission to own a machine.
#[test]
fn data_qualified_declarations_are_rejected_independently_of_their_bodies() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for fixture in [
        "tests/fixtures/pass/calls/attached_terminal_machine_call_exit",
        "tests/fixtures/fail/calls/free_machine_named_transition_rejected",
    ] {
        let verdict = route_verdict(fixture, "start");
        let RouteVerdict::Rejected { violations, .. } = verdict else {
            panic!("{fixture}: expected a source-rule rejection, not another verdict");
        };
        assert!(
            violations
                .iter()
                .any(|violation| violation.message().contains(
                    "a data declaration cannot own a machine; declare an ordinary static machine"
                )),
            "{fixture}: {violations:?}"
        );
    }
}

#[test]
fn module_qualification_does_not_attach_a_machine_to_data() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(
        "module-qualified-static-call",
        "module utility;\n\
         machine step() {}\n\
         machine main() { utility::step(); }\n",
    );
    let entry = unbuilt_frontier::program_machine(&frontier.typed, "main")
        .expect("the module's exact main declaration");
    let verdict = judge(typed_trees_to_terminal_psi::elaborate_closure(
        &frontier.typed,
        entry,
    ))
    .expect("module qualification reaches a verdict");
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn type_scoped_constants_read_their_checked_initializers() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let verdict = route_verdict(
        "tests/fixtures/pass/constants/attached_predicate_handoff",
        "main",
    );
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    let negative = unbuilt_frontier::repository(
        "tests/fixtures/fail/constants/attached_predicate_handoff/main.omg",
    );
    let Err(messages) = unbuilt_frontier::frontier(&negative, None, &[]) else {
        panic!("the initializer must establish its record's predicate");
    };
    assert!(
        messages.iter().any(|message| {
            message.contains("constant `Index::SELECTED` evaluation")
                && message.contains("cannot prove requires clause")
        }),
        "{messages:?}"
    );
}

#[test]
fn a_public_carrier_does_not_expose_its_private_constant() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = std::env::temp_dir().join(format!("omega-private-constant-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let main = root.join("main.omg");
    std::fs::write(
        &main,
        "use dependency::attached_values;\n\
         machine main() -> u64 { dependency::attached_values::Index::SELECTED.value }",
    )
    .unwrap();
    let result = unbuilt_frontier::frontier(
        &main,
        None,
        &[(
            "dependency",
            "tests/fixtures/pass/constants/attached_predicate_handoff",
        )],
    );
    std::fs::remove_dir_all(root).unwrap();
    let Err(messages) = result else {
        panic!("a public carrier exposes no private constant to its dependent");
    };
    assert!(
        messages.iter().any(|message| {
            message.contains("Index::SELECTED` is private to its declaring package")
        }),
        "{messages:?}"
    );
}
