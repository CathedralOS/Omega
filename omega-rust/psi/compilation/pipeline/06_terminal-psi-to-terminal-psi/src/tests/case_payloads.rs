//! Loans of a case payload. Read from source, as a Build program: the
//! toolchain's `Build::request_dependency` dispatches on a `Source` and hands
//! the selected case's views to the request it makes.
//!
//! A payload field is a place like any other projected place, one that
//! exists only while its case is selected. patterns.md, "A sum held in a
//! field": "The case fact belongs to the place", and "Payload mutation and
//! case replacement retain ordinary subject invalidation and loan rules."
//! So a loan of a payload is a loan of that exact place, formed where the
//! sum is known to hold the case, and it conflicts with whatever overlaps
//! it, as a loan of a field does.

use super::build_purpose::judge_build;
use super::*;

/// `accepted` for a program the checker accepts and can publish. For one it
/// judges wrong, what it could not establish or the loan that conflicts.
/// Else the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected {
            unestablished,
            conflicts,
            ..
        }) => match (&unestablished[..], &conflicts[..]) {
            ([one], []) => one.detail.to_owned(),
            ([], [one]) => one.statement.clone(),
            _ => panic!("{unestablished:?} {conflicts:?}"),
        },
        other => panic!("{other:?}"),
    }
}

const NO_CASE: &str = "cannot prove selected case for payload access: a path to this access does not establish the case";

const VIEWS: &str = r#"
data Source<'storage> {
    case Path(location: &'storage [u8]);
    case Git(repository: &'storage [u8], revision: u32);
}
machine take(view: &[u8]) { }
machine measure(view: &[u8]) -> u32 { 1 }
"#;

const SCALARS: &str = r#"
data Source [copy] {
    case Path(location: u32);
    case Git(repository: u32, revision: u32);
}
machine look(value: &u32) -> u32 { 1 }
machine bump(value: &mut u32) -> u32 { 1 }
machine two(value: &mut u32, other: &u32) -> u32 { 1 }
machine both(held: &mut Source, value: &u32) -> u32 { 1 }
machine replace(held: &mut Source) { }
"#;

/// The toolchain's shape: each arm of a dispatch over an owned sum passes
/// a view its case holds. The binding is the payload place of the subject
/// the dispatch saved, and the arm is where that case is selected. A
/// binding of another case is no name in the arm.
#[test]
fn a_dispatch_arm_passes_the_view_its_case_holds() {
    let arms = |git: &str| {
        format!(
            r#"{VIEWS}
machine build_entry(source: Source) {{ route(source); }}
machine route(source: Source) {{
    transition source {{
        Source::Path {{ location }} -> take(location)
        Source::Git {{ repository, revision }} -> take({git})
    }}
}}
"#
        )
    };
    assert_eq!(verdict(&arms("repository")), "accepted");
    assert_eq!(verdict(&arms("location")), "spelling name without a symbol");
}

/// The same from a value-position `match`, and for a scalar payload lent
/// shared.
#[test]
fn a_match_arm_lends_the_payload_its_case_holds() {
    assert_eq!(
        verdict(&format!(
            r#"{VIEWS}
machine build_entry(source: Source) {{ let n: u32 = route(source); }}
machine route(source: Source) -> u32 {{
    match source {{
        Source::Path {{ location }} -> measure(location)
        Source::Git {{ repository, revision }} -> measure(repository)
    }}
}}
"#
        )),
        "accepted"
    );
    let scalar = |lent: &str| {
        format!(
            r#"{SCALARS}
machine build_entry(source: Source) {{ let n: u32 = route(source); }}
machine route(source: Source) -> u32 {{
    let mut held: Source = source;
    match held {{
        Source::Path {{ location }} -> {lent}
        Source::Git {{ repository, revision }} -> 2
    }}
}}
"#
        )
    };
    assert_eq!(verdict(&scalar("look(&location)")), "accepted");
    // The payload lies inside `held`, so lending both is lending one
    // place twice.
    assert_eq!(
        verdict(&scalar("both(&mut held, &location)")),
        "state `both` receives `&<saved subject #1:1>.location` as both mutable and read-only"
    );
}

/// A transition holds its subject in a binding of its own, which nothing
/// may write, so an arm's payload is not lent exclusively.
#[test]
fn a_transition_arm_does_not_lend_its_payload_exclusively() {
    assert_eq!(
        verdict(&format!(
            r#"{SCALARS}
machine build_entry(source: Source) {{ route(source); }}
machine route(source: Source) {{
    let mut held: Source = source;
    transition held {{
        Source::Path {{ location }} -> replace_scalar(&mut location)
        Source::Git {{ repository, revision }} -> replace_scalar(&mut repository)
    }}
}}
machine replace_scalar(value: &mut u32) {{ }}
"#
        )),
        "call exclusive loan of an immutable binding"
    );
}

/// A payload named directly is lent where the sum is known to hold its
/// case, here from the construction that is its only reaching write, and
/// nowhere else: not a payload of another case, and not after a write that
/// may have changed the case.
#[test]
fn a_payload_is_lent_only_where_its_case_is_selected() {
    let view = |lent: &str| {
        format!(
            r#"{VIEWS}
machine build_entry() {{ let n: u32 = route(); }}
machine route() -> u32 {{
    let held: Source = Source::Path {{ location: "here" }};
    measure(held.{lent})
}}
"#
        )
    };
    assert_eq!(verdict(&view("location")), "accepted");
    assert_eq!(verdict(&view("repository")), NO_CASE);

    let scalar = |before: &str| {
        format!(
            r#"{SCALARS}
machine build_entry() {{ let n: u32 = route(); }}
machine route() -> u32 {{
    let mut held: Source = Source::Path {{ location: 3 }};
    {before}
    bump(&mut held.location)
}}
"#
        )
    };
    assert_eq!(verdict(&scalar("")), "accepted");
    assert_eq!(
        verdict(&scalar(
            "held = Source::Git { repository: 1, revision: 2 };"
        )),
        NO_CASE
    );
    // An exclusive loan of the sum lets its holder replace the case, and
    // what a helper may have written is the write frame it publishes
    // (dependent_values.md, "Mutation frames"). This helper writes nothing,
    // so the case it was lent still stands after the call.
    assert_eq!(verdict(&scalar("replace(&mut held);")), "accepted");
}

/// A sum never known to hold the case lends no payload of it.
#[test]
fn a_sum_of_unknown_case_lends_no_payload() {
    assert_eq!(
        verdict(&format!(
            r#"{VIEWS}
machine build_entry(source: Source) {{ let n: u32 = route(source); }}
machine route(source: Source) -> u32 {{ measure(source.location) }}
"#
        )),
        NO_CASE
    );
}

/// Two payload fields of one case are disjoint places: one lent
/// exclusively beside the other. The same field is one place.
#[test]
fn payload_fields_of_one_case_are_lent_as_sibling_places() {
    let lent = |other: &str| {
        format!(
            r#"{SCALARS}
machine build_entry() {{ let n: u32 = route(); }}
machine route() -> u32 {{
    let mut held: Source = Source::Git {{ repository: 1, revision: 2 }};
    two(&mut held.repository, &held.{other})
}}
"#
        )
    };
    assert_eq!(verdict(&lent("revision")), "accepted");
    assert_eq!(
        verdict(&lent("repository")),
        "state `two` receives `&held.repository` as both mutable and read-only"
    );
}

// An owned payload is replaced at its captured case path, while its sum keeps
// its case and neighboring storage. The false arm makes the authored value seven an
// executable oracle rather than accepting any completing store.
const OWNED_STORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/fixtures/run/ownership/known_case_payload_store/main.omg"
));

// The inline source harness selects build_entry; the kept executable binds
// the same body as main through each target's ordinary ProgramEntry.
fn owned_store_source() -> String {
    OWNED_STORE.replace("machine main()", "machine build_entry()")
}

fn execute_owned_store(source: &str) {
    let verdict = judge_build(source);
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("the exact selected payload store must be checked: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut psi_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "the stored payload must retain seven: {status:?}"
    );
}

#[test]
fn a_known_owned_case_payload_replacement_publishes_and_executes() {
    execute_owned_store(&owned_store_source());
}

#[test]
fn a_nested_scalar_store_keeps_the_selected_payload_path() {
    execute_owned_store(
        &owned_store_source().replace("choice.item = item(7);", "choice.item.value = 7;"),
    );
}

#[test]
fn an_owned_payload_store_requires_its_exact_selected_case() {
    let source = owned_store_source().replace(
        "Choice::Left { item: Item { value: 3 } }",
        "Choice::Right {}",
    );
    let verdict = judge_build(&source);
    let Ok(RouteVerdict::Rejected { unestablished, .. }) = verdict else {
        panic!("a different selected case cannot receive the payload: {verdict:?}");
    };
    // A member's source span names its selector. Pin it inside this exact
    // assignment target, independently of the later payload read.
    let target = "choice.item";
    let start = source.find(&format!("{target} =")).unwrap();
    let end = start + target.len();
    assert!(unestablished.iter().any(|failure| failure.detail == NO_CASE
        && matches!(&failure.site, Some(SourceOrigin::Authored(span))
            if start <= span.span.start && span.span.end <= end)));
}

#[test]
fn an_owned_payload_store_requires_mutability_of_the_existing_root() {
    let source = owned_store_source().replace("let mut choice:", "let choice:");
    assert_eq!(
        verdict(&source),
        "statement store into an immutable binding"
    );
}

#[test]
fn a_payload_store_cannot_use_case_membership_invalidated_by_its_replacement() {
    let source = owned_store_source()
        .replace("machine item(value: u32) -> Item { Item { value: value } }",
            "data Holder { choice: Choice; }\nmachine item(value: u32, owner: &mut Holder) -> Item { Item { value: value } }")
        .replace("machine consume(choice: Choice) { }", "machine consume(holder: Holder) { }")
        .replace("let mut choice: Choice = Choice::Left { item: Item { value: 3 } };",
            "let mut holder: Holder = Holder { choice: Choice::Left { item: Item { value: 3 } } };")
        .replace("choice.item = item(7);", "holder.choice.item = item(7, &mut holder);")
        .replace("choice.item.value", "holder.choice.item.value")
        .replace("consume(choice)", "consume(holder)");
    execute_owned_store(&source);
    let changed = source.replace(
        "machine item(value: u32, owner: &mut Holder) -> Item { Item { value: value } }",
        "machine item(value: u32, owner: &mut Holder) -> Item { let replacement: Choice = Choice::Right {}; owner.choice = replacement; Item { value: value } }",
    );
    let verdict = judge_build(&changed);
    let Ok(RouteVerdict::Rejected { unestablished, .. }) = verdict else {
        panic!("the RHS cannot remove the case of the captured destination: {verdict:?}");
    };
    // A member's source span names its selector. Pin it inside this exact
    // assignment target, independently of the later payload read.
    let target = "holder.choice.item";
    let start = changed.find(&format!("{target} =")).unwrap();
    let end = start + target.len();
    assert!(unestablished.iter().any(|failure| failure.detail == NO_CASE
        && matches!(&failure.site, Some(SourceOrigin::Authored(span))
            if start <= span.span.start && span.span.end <= end)));
}
