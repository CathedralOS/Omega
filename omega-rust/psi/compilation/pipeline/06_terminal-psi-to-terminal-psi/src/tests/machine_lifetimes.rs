//! Machines that declare lifetime parameters, and calls that pass byte
//! views. Read from source, as a Build program: the toolchain's
//! `dependency::from_source<'storage>` is such a machine, and every
//! `Build::dependency` call goes through it.
//!
//! lifetimes.md: "Lifetime arguments are erased regions, separate from
//! runtime generic arity, layout and monomorphization." A machine with
//! lifetime parameters is therefore one machine, judged once: each parameter
//! is a region the body knows nothing else about, named by the type of every
//! view written with it. A call stands a region of the caller in for each.

use super::build_purpose::judge_build;
use super::*;

/// `accepted` for a program the checker accepts and can publish, `rejected`
/// for one it judges wrong, else the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { .. }) => "rejected".to_owned(),
        other => panic!("{other:?}"),
    }
}

/// "An explicit result lifetime relates its reference leaves to the
/// corresponding input lifetime." The body may return the view it was given
/// for that lifetime and no other: "A signature does not authorize returning
/// unrelated storage".
#[test]
fn a_machine_returns_only_a_view_of_the_region_its_result_names() {
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8]) { let kept: &[u8] = same(name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8], second: &[u8]) { let kept: &[u8] = pick(name, second); }
machine pick<'a, 'b>(view: &'a [u8], other: &'b [u8]) -> &'a [u8] { other }
"#
        ),
        "rejected"
    );
}

/// The call gives the lifetime the region of the view it passes, and the
/// result is a view of that region: a view taken from a record's field comes
/// back as a value of that field, and a view of any other region does not.
/// The two formals state that they are at one region; elided, each would be
/// at its own (lifetimes.md: "Several unnamed carried references do not
/// become one source merely because they occupy one parameter"), and
/// `other`'s view would not be a value of `session`'s field.
#[test]
fn a_call_gives_a_lifetime_the_region_of_the_view_it_passes() {
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry<'s>(session: &mut Session<'s>, other: &Session<'s>) { session.name = same(other.name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, other: &Session) { session.name = same(other.name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "typing call that gives one lifetime two regions"
    );
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, name: &[u8]) { session.name = same(name); }
machine same<'a>(view: &'a [u8]) -> &'a [u8] { view }
"#
        ),
        "typing call that gives one lifetime two regions"
    );
}

/// One lifetime stands for one region at a call. There are no "general
/// authored outlives bounds" to relate two, so views of two regions passed
/// for one lifetime are refused; a literal is within every region.
#[test]
fn one_lifetime_is_not_given_two_regions() {
    let both = "machine both<'a>(left: &'a [u8], right: &'a [u8]) -> &'a [u8] { left }";
    assert_eq!(
        verdict(&format!(
            r#"
data Session<'storage> {{ ready: bool; name: &'storage [u8]; }}
machine build_entry<'s>(session: &mut Session<'s>, other: &Session<'s>) {{ session.name = both(other.name, "literal"); }}
{both}
"#
        )),
        "accepted"
    );
    assert_eq!(
        verdict(&format!(
            r#"
data Session<'storage> {{ ready: bool; name: &'storage [u8]; }}
machine build_entry<'s>(session: &mut Session<'s>, other: &Session<'s>, name: &[u8]) {{ session.name = both(other.name, name); }}
{both}
"#
        )),
        "typing call that gives one lifetime two regions"
    );
}

/// "For an elided output, one unambiguous reference input supplies the
/// relation. ... Ambiguous elision requires an explicit relation, not a guess
/// by name or position." A machine with one input view returns a view of it;
/// one with two must say which, or its result is refused wherever it is
/// declared.
#[test]
fn a_result_without_a_lifetime_needs_one_input_lifetime() {
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8]) { let kept: &[u8] = same(name); }
machine same(view: &[u8]) -> &[u8] { view }
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8], second: &[u8]) { let kept: &[u8] = pick(name, second); }
machine pick(view: &[u8], other: &[u8]) -> &[u8] { other }
"#
        ),
        "typing result without a lifetime of a signature with several input lifetimes"
    );
    // Two regions among the views passed matter to nothing else.
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &Session, name: &[u8]) { take(session.name, name); }
machine take(view: &[u8], other: &[u8]) { }
"#
        ),
        "accepted"
    );
}

/// A result lifetime no argument is a view of has no region at the call.
#[test]
fn a_result_lifetime_no_view_is_passed_for_is_refused() {
    assert_eq!(
        verdict(
            r#"
machine build_entry() { let kept: &[u8] = made(); }
machine made<'a>() -> &'a [u8] { "made" }
"#
        ),
        "typing result lifetime that no argument is a view of"
    );
}

/// What a signature may write with a lifetime. A view names a lifetime its
/// machine declares. A reference to anything else is a loan of one place,
/// and what its lifetime relates it to is not read yet.
#[test]
fn a_signature_names_only_lifetimes_its_machine_declares_and_only_on_views() {
    assert_eq!(
        verdict(
            r#"
machine build_entry(name: &[u8]) { take(name); }
machine take(view: &'missing [u8]) { }
"#
        ),
        "type byte view has no complete owned region identity"
    );
    assert_eq!(
        verdict(
            r#"
machine build_entry() { let count: u32 = 1; take(&count); }
machine take<'a>(count: &'a u32) { }
"#
        ),
        "parameter reference parameter with a named lifetime"
    );
}

/// A machine declared under a data type's name owns none of that data's
/// lifetimes: it cannot take a view at one and store it.
#[test]
fn a_machine_under_a_data_name_is_not_given_the_data_lifetimes() {
    assert_eq!(
        verdict(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, name: &[u8]) { Session::keep(session, name); }
machine Session::keep(session: &mut Session, view: &'storage [u8]) { session.name = view; }
"#
        ),
        "type byte view has no complete owned region identity"
    );
}

/// The toolchain's `dependency::from_source` and the call that consumes its
/// result: data applied at the machine's lifetime is the declaration's one
/// record, a literal fills a stored view, and a stored view is passed to a
/// view formal beside its sibling fields.
#[test]
fn a_description_is_made_from_a_source_and_taken_apart_for_a_request() {
    let program = |request: &str| {
        format!(
            r#"
data Source<'storage> {{ location: &'storage [u8]; }}
data Description<'storage> {{ source: Source<'storage>; explicit_alias: bool; alias: &'storage [u8]; }}
machine build_entry() {{ register(from_source(Source {{ location: "std" }})); }}
machine from_source<'storage>(source: Source<'storage>) -> Description<'storage> {{
    Description {{ source: source, explicit_alias: false, alias: "" }}
}}
machine register(description: Description) {{ {request} }}
machine request(explicit_alias: bool, alias: &[u8], source: Source) {{
    path(explicit_alias, alias, source.location);
}}
machine path(explicit_alias: bool, alias: &[u8], location: &[u8]) {{ }}
machine whole(alias: &[u8], description: Description) {{ }}
machine whole_source(source: &Source, description: Description) {{ }}
"#
        )
    };
    assert_eq!(
        verdict(&program(
            "request(description.explicit_alias, description.alias, description.source);"
        )),
        "accepted"
    );
    // The first argument captures the shared descriptor before the second
    // transfers its carrier. Its backing remains the original literal;
    // moving the carrier does not consume those bytes.
    assert_eq!(
        verdict(&program("whole(description.alias, description);")),
        "accepted"
    );
    // A reference to an inline record still refers to its carrier's
    // storage. Snapshotting shared views must not remove the source guard
    // against transferring that same storage in another argument.
    assert_eq!(
        verdict(&program("whole_source(&description.source, description);")),
        "call argument that names storage another argument lends"
    );
}

/// A machine's lifetime applied to data makes the data's stored views views
/// of that lifetime, so `dependency::with_alias` stores a view of its own
/// lifetime into a description at it. Data applied at another lifetime does
/// not take it.
#[test]
fn a_view_of_a_machine_lifetime_is_stored_in_data_applied_at_it() {
    assert_eq!(
        verdict(
            r#"
data Description<'storage> { explicit_alias: bool; alias: &'storage [u8]; }
machine build_entry() { }
machine with_alias<'storage>(description: &mut Description<'storage>, alias: &'storage [u8]) {
    description.explicit_alias = true;
    description.alias = alias;
}
"#
        ),
        "accepted"
    );
    assert_eq!(
        verdict(
            r#"
data Description<'storage> { explicit_alias: bool; alias: &'storage [u8]; }
machine build_entry() { }
machine with_alias<'storage, 'other>(description: &mut Description<'storage>, alias: &'other [u8]) {
    description.alias = alias;
}
"#
        ),
        "typing assigned value type"
    );
}

/// A case written with its sum, `Shape::Dot { at }`, heads a pattern.
#[test]
fn a_pattern_is_headed_by_a_case_written_with_its_sum() {
    assert_eq!(
        verdict(
            r#"
data Shape [copy] { case Dot(at: u32); case Pair(left: u32, right: u32); }
machine build_entry() { let total: u32 = weigh(Shape::Dot { at: 1 }); }
machine weigh(shape: Shape) -> u32 {
    transition shape {
        Shape::Dot { at } -> at
        Shape::Pair { left, right } -> left
    }
}
"#
        ),
        "accepted"
    );
}
