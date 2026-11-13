//! Service reach and synchronous entries, read from source. Every `Build::*`
//! operation of the toolchain's Build vocabulary has this shape: a checked
//! machine with written `reaches` and `invokes` ceilings whose body calls a
//! boundary requirement.

use super::boundary_calls::{entry, typed};
use super::*;

/// The checker's verdict on the closure of `entry` in `text`. The source
/// belongs to this activation and is no package's subject, as in
/// `boundary_calls`.
fn judged(text: &str) -> RouteVerdict {
    let typed = typed(text);
    let entry = entry(&typed);
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    let mut verdicts = crate::judge_compilation_for(
        &frozen,
        structure_elaboration::Compilation {
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    assert_eq!(verdicts.len(), 1);
    verdicts.remove(0).unwrap()
}

/// `Output` is a service identity with no operation; `Console::write` is a
/// boundary requirement that may reach it; `relay` is a checked body that
/// calls the requirement under `relay_reach`; the entry calls `relay` under
/// `entry_reach`.
fn program(relay_reach: &str, entry_reach: &str) -> String {
    format!(
        r#"
boundary trait Output {{}}
boundary trait Console {{
    machine write(value: u32) reaches Output crashes none;
}}
machine relay(value: u32) {relay_reach} {{ Console::write(value); }}
machine entry() {entry_reach} {{ relay(1); }}
"#
    )
}

fn verdict_kind(verdict: &RouteVerdict) -> String {
    match verdict {
        RouteVerdict::Accepted(_) => "accepted".into(),
        RouteVerdict::Rejected { .. } => "rejected".into(),
        RouteVerdict::Incomplete(_) => "incomplete".into(),
        RouteVerdict::FallBack(refusal) => format!("{refusal:?}"),
    }
}

fn accepted(text: &str) -> Box<crate::AcceptedClosure> {
    match judged(text) {
        RouteVerdict::Accepted(accepted) => accepted,
        other => panic!("{}", verdict_kind(&other)),
    }
}

fn exceeded(text: &str) -> Vec<String> {
    match judged(text) {
        RouteVerdict::Rejected {
            formations,
            unproved,
            unestablished,
            conflicts,
            exceeded,
        } => {
            assert!(formations.is_empty() && unproved.is_empty());
            assert!(unestablished.is_empty() && conflicts.is_empty());
            exceeded.iter().map(|entry| entry.message()).collect()
        }
        other => panic!("{}", verdict_kind(&other)),
    }
}

/// The rows of `name`, as (written ceiling, published reach), by service
/// identity.
fn rows(accepted: &crate::AcceptedClosure, position: u64) -> (Vec<String>, Vec<String>) {
    let module = &accepted.elaboration.module;
    let machine = module.machine(id(position)).unwrap();
    let names = |row: &[semantic_vocabulary::ServiceId]| {
        row.iter()
            .map(|service| {
                let declaration = module
                    .services
                    .iter()
                    .find(|declaration| declaration.id == *service)
                    .unwrap();
                module
                    .identities
                    .text(declaration.identity)
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    (
        names(&machine.declared_service_reach),
        names(&machine.published_service_ceiling),
    )
}

#[test]
fn a_body_publishes_its_written_ceiling_and_an_omitted_one_is_derived() {
    let accepted = accepted(&program("reaches Output", ""));
    // The entry wrote nothing: it publishes what its call of `relay`
    // contributes, and that is the module's root reach.
    let (written, published) = rows(&accepted, 1);
    assert!(written.is_empty());
    assert_eq!(published.len(), 1);
    assert!(published[0].contains("Output"), "{published:?}");
    assert_eq!(
        accepted.elaboration.module.root_service_reach.concrete,
        accepted
            .elaboration
            .module
            .machine(id(1))
            .unwrap()
            .published_service_ceiling
    );
    // `relay` wrote its ceiling; the row is both.
    let (written, published) = rows(&accepted, 2);
    assert_eq!(written, published);
    assert_eq!(written.len(), 1);
    accepted.publish_invocation().unwrap();
}

#[test]
fn a_call_outside_the_callers_written_ceiling_is_rejected_at_that_call() {
    let messages = exceeded(&program("reaches Output", "reaches none"));
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].starts_with("machine `entry` reaches `none` but its call of `relay`"),
        "{messages:?}"
    );
    // The twin differs only in the entry's clause.
    accepted(&program("reaches Output", "reaches Output"));
}

/// effects.md: "A boundary call contributes what its requirement declares
/// and not the called trait's name, so a ceiling that names the slot does
/// not cover its operations".
#[test]
fn a_ceiling_that_names_the_called_trait_does_not_cover_its_operation() {
    let messages = exceeded(&program("reaches Console", ""));
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].starts_with("machine `relay` reaches `"),
        "{messages:?}"
    );
    assert!(messages[0].contains("its call of `"), "{messages:?}");
    assert!(messages[0].contains("Output"), "{messages:?}");
    assert!(messages[0].ends_with(" (service reach)"), "{messages:?}");
}

/// The shape of `Build::application`: a ceiling wider than what the body
/// reaches. "A present clause is an upper bound, not an assertion of exact
/// equality", and the caller is charged the written ceiling.
#[test]
fn a_ceiling_wider_than_the_body_is_what_callers_are_charged() {
    let program = |entry_reach: &str| {
        format!(
            r#"
boundary trait Requests {{
    machine application(value: u32) reaches none crashes none;
}}
machine application(value: u32) reaches Requests {{ Requests::application(value); }}
machine entry() {entry_reach} {{ application(1); }}
"#
        )
    };
    let accepted = accepted(&program(""));
    let (_, published) = rows(&accepted, 1);
    assert_eq!(published.len(), 1);
    assert!(published[0].contains("Requests"), "{published:?}");
    let messages = exceeded(&program("reaches none"));
    assert_eq!(messages.len(), 1, "{messages:?}");
}

/// The published synchronous entries of the machine at `position`, by
/// service identity.
fn entries(accepted: &crate::AcceptedClosure, position: u64) -> Vec<String> {
    let module = &accepted.elaboration.module;
    module
        .machine(id(position))
        .unwrap()
        .published_synchronous_entries
        .iter()
        .map(|service| {
            let declaration = module
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .unwrap();
            module
                .identities
                .text(declaration.identity)
                .unwrap()
                .to_owned()
        })
        .collect()
}

/// `Registrar::execute` may run a `Handler` before it returns; `relay` is a
/// checked body that calls it under `relay_invokes`; the entry calls `relay`
/// under `entry_invokes`. `Other` is an entry nothing here arrives at.
fn entering(relay_invokes: &str, entry_invokes: &str) -> String {
    format!(
        r#"
boundary trait Handler {{}}
boundary trait Other {{}}
boundary trait Registrar {{
    machine execute(value: u32) reaches none crashes none invokes Handler;
    machine register(value: u32) reaches none crashes none;
}}
machine relay(value: u32) {relay_invokes} {{ Registrar::execute(value); }}
machine entry() {entry_invokes} {{ relay(1); }}
"#
    )
}

/// effects.md, "Direct synchronous invocation": on a checked body an omitted
/// `invokes` is "Infer and publish its entries, including forwarding through
/// local helpers", and "the callee's published `invokes` joins the caller's,
/// as its reach does".
#[test]
fn a_body_publishes_its_written_entries_and_an_omitted_clause_is_derived() {
    let accepted = accepted(&entering("invokes Handler", ""));
    let relay = entries(&accepted, 2);
    assert_eq!(relay.len(), 1);
    assert!(relay[0].contains("Handler"), "{relay:?}");
    assert_eq!(entries(&accepted, 1), relay);
    // The boundary declaration carries its requirement's row.
    let module = &accepted.elaboration.module;
    let published: Vec<usize> = module
        .boundary_machines
        .iter()
        .map(|boundary| boundary.published_synchronous_entries.len())
        .collect();
    assert_eq!(published, [1]);
    // Entering a callback reaches no service by itself.
    assert!(module.root_service_reach.concrete.is_empty());
    accepted.publish_invocation().unwrap();
    // Derived through both bodies when neither writes a clause.
    let derived = self::accepted(&entering("", ""));
    assert_eq!(entries(&derived, 1), relay);
    assert_eq!(entries(&derived, 2), relay);
}

#[test]
fn a_call_outside_the_callers_written_invokes_is_rejected_at_that_call() {
    let messages = exceeded(&entering("invokes Handler", "invokes Other"));
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].starts_with("machine `entry` invokes `"),
        "{messages:?}"
    );
    assert!(
        messages[0].contains("but its call of `relay` contributes `"),
        "{messages:?}"
    );
    assert!(
        messages[0].ends_with(" (synchronous invocation)"),
        "{messages:?}"
    );
    // The boundary call itself is judged the same way, in the body that
    // makes it.
    let messages = exceeded(&entering("invokes Other", ""));
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].starts_with("machine `relay` invokes `"),
        "{messages:?}"
    );
    // The twins differ only in the clause.
    accepted(&entering("invokes Handler", "invokes Handler"));
    accepted(&entering(
        "invokes Handler",
        "invokes Handler invokes Other",
    ));
}

/// A requirement that omits `invokes` has "No synchronous entry", so a call
/// of it contributes nothing. The entry row is not the reach row: naming an
/// identity in `reaches` covers no entry.
#[test]
fn a_requirement_that_omits_invokes_enters_nothing_and_reach_covers_no_entry() {
    let program = |body: &str, clause: &str| {
        format!(
            r#"
boundary trait Handler {{}}
boundary trait Registrar {{
    machine execute(value: u32) reaches none crashes none invokes Handler;
    machine register(value: u32) reaches none crashes none;
}}
machine entry() {clause} {{ Registrar::{body}(1); }}
"#
        )
    };
    let accepted = accepted(&program("register", ""));
    assert!(entries(&accepted, 1).is_empty());
    let messages = exceeded(&program("execute", "reaches Handler invokes Registrar"));
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].ends_with(" (synchronous invocation)"),
        "{messages:?}"
    );
}

/// The shape of `Build::application`: `reaches Requests invokes Requests`
/// over a requirement that enters nothing. "A row that no selected provider
/// uses is a broader ceiling, not an error", and callers are charged it.
#[test]
fn an_invokes_ceiling_wider_than_the_body_is_what_callers_are_charged() {
    let program = |entry_invokes: &str| {
        format!(
            r#"
boundary trait Other {{}}
boundary trait Requests {{
    machine application(value: u32) reaches none crashes none;
}}
machine application(value: u32) reaches Requests invokes Requests {{
    Requests::application(value);
}}
machine entry() {entry_invokes} {{ application(1); }}
"#
        )
    };
    let accepted = accepted(&program(""));
    let published = entries(&accepted, 1);
    assert_eq!(published.len(), 1);
    assert!(published[0].contains("Requests"), "{published:?}");
    accepted.publish_invocation().unwrap();
    let messages = exceeded(&program("invokes Other"));
    assert_eq!(messages.len(), 1, "{messages:?}");
}

/// The callable-refinement guard, from source: a supplied callable whose
/// published row lies outside the required contract's ceiling is refused,
/// on either axis, and its twin inside the ceiling is proved.
#[test]
fn a_supplied_callable_outside_the_required_ceiling_is_refused() {
    let program = |required: &str, supplied: &str| {
        format!(
            r#"
boundary trait Output {{}}
boundary trait Handler {{}}
boundary trait Console {{
    machine write(value: u32) reaches Output crashes none invokes Handler;
}}
trait Sink {{
    machine put(value: u32) {required};
}}
machine supplied(value: u32) {supplied} {{ Console::write(value); }}
"#
        )
    };
    let refine = |required: &str, supplied: &str| {
        let typed = typed(&program(required, supplied));
        let requirement = typed
            .trait_machine_signatures
            .iter()
            .find(|(_, signature)| signature.name.as_str() == "put")
            .unwrap()
            .1;
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "supplied")
            .unwrap()
            .symbol;
        crate::prove_callable_refinement(
            &typed,
            requirement,
            structure_elaboration::CallableDeclaration::Machine(machine),
            &mut crate::ProofSearchActivity::default(),
        )
    };
    let inside = "reaches Output crashes none invokes Handler";
    refine(inside, inside).unwrap();
    let refused = refine("reaches none crashes none invokes Handler", inside).unwrap_err();
    assert_eq!(
        refused.detail,
        "supplied callable reaches a service outside the required ceiling"
    );
    let refused = refine("reaches Output crashes none", inside).unwrap_err();
    assert_eq!(
        refused.detail,
        "supplied callable enters a callback outside the required invokes ceiling"
    );
}
