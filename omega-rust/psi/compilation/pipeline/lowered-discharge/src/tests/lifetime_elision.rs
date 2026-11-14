//! Data that declares lifetime parameters, named in a machine's signature
//! without them. Read from source, as a Build program: every project's
//! `build.omg` takes `builder: &mut Build`, and the toolchain declares
//! `data Build<'storage>`.

use super::build_purpose::judge_build;
use super::*;

fn refusal(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_formal_names_lifetime_parameterised_data_without_its_lifetimes() {
    let verdict = judge_build(
        r#"
data Session<'storage> { ready: bool; count: u32; }
machine build_entry(session: &mut Session) { session.ready = true; }
"#,
    )
    .unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    accepted.publish_invocation().unwrap();
}

#[test]
fn a_formal_that_names_a_lifetime_nothing_declares_is_refused() {
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; count: u32; }
machine build_entry(session: &mut Session<'missing>) { session.ready = true; }
"#
        ),
        "typing lifetime argument its owner does not declare"
    );
}

/// `Build` itself stores views: `identifier: &'storage [u8]`. The view's
/// lifetime is a binder of the data declaration the field belongs to.
#[test]
fn a_field_stores_a_view_for_a_lifetime_its_declaration_binds() {
    let verdict = judge_build(
        r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session) { session.ready = true; }
"#,
    )
    .unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    accepted.publish_invocation().unwrap();
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'other [u8]; }
machine build_entry(session: &mut Session) { session.ready = true; }
"#
        ),
        "type byte view has no complete owned region identity"
    );
}

/// Eliding a lifetime relates nothing but an elided result to its one input
/// (`an_elided_result_is_at_the_region_of_the_one_input`). lifetimes.md: "A signature does not
/// authorize returning unrelated storage merely because its type has the same
/// lifetime spelling", and an elided input is not even the same spelling. A
/// view's type carries the region it is valid for, so a view from one region
/// is not a value of a field, or a result, of another.
#[test]
fn an_elided_lifetime_does_not_relate_a_view_to_another_region() {
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session, name: &[u8]) { session.name = name; }
"#
        ),
        "typing assigned value type"
    );
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(session: &mut Session) { let kept: &[u8] = helper(session); }
machine helper(session: &Session) -> &[u8] { session.name }
"#
        ),
        "typing result without a lifetime of a signature with several input lifetimes"
    );
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(name: &[u8], other: &[u8]) { let made: Session = helper(name, other); }
machine helper(name: &[u8], other: &[u8]) -> Session { Session { ready: true, name: name } }
"#
        ),
        "typing result without a lifetime of a signature with several input lifetimes"
    );
}

/// The one place eliding relates something: "For an elided output, one
/// unambiguous reference input supplies the relation." With one input
/// view, an elided result is at its region, so the result may hold it.
#[test]
fn an_elided_result_is_at_the_region_of_the_one_input() {
    let verdict = judge_build(
        r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(name: &[u8]) { let made: Session = helper(name); }
machine helper(name: &[u8]) -> Session { Session { ready: true, name: name } }
"#,
    )
    .unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    accepted.publish_invocation().unwrap();
}

/// The source record rows of an accepted Build program, by the name of the
/// data declaration each came from: whether the row kept a type occurrence.
fn record_rows(build: &str) -> Vec<(String, bool)> {
    let typed = super::build_purpose::frontier_of("machine product_entry() { }", build);
    let verdict = judge_build(build).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    let rows = accepted
        .elaboration
        .source_records
        .iter()
        .map(|row| {
            let data = typed.data_definition_by_symbol(row.declaration).unwrap();
            (data.name.as_str().to_owned(), row.application.is_some())
        })
        .collect();
    accepted.publish_invocation().unwrap();
    rows
}

/// `Build` stores `elf_interpreter: ElfInterpreterSelection<'storage>`: data
/// the program names only applied at a lifetime, as a field. That occurrence
/// is the record's source application, as a bare `Session` is.
#[test]
fn data_named_only_at_a_lifetime_keeps_that_occurrence() {
    assert_eq!(
        record_rows(
            r#"
data Inner<'held> { count: u32; name: &'held [u8]; }
data Session<'storage> { ready: bool; inner: Inner<'storage>; }
machine build_entry(session: &mut Session) { session.ready = true; }
"#
        ),
        [("Inner".to_owned(), true), ("Session".to_owned(), true)]
    );
    assert_eq!(
        refusal(
            r#"
data Inner<'held> { count: u32; }
data Session<'storage> { ready: bool; inner: Inner<'missing>; }
machine build_entry(session: &mut Session) { session.ready = true; }
"#
        ),
        "typing lifetime argument its owner does not declare"
    );
}

/// A declaration the program never names as a type has no occurrence to
/// keep. It is still a record of the judged program, identified by its
/// declaration; data with a type parameter is not such a record.
#[test]
fn a_declaration_nothing_names_is_a_record_without_an_occurrence() {
    assert_eq!(
        record_rows(
            r#"
data Unused<'held> { count: u32; name: &'held [u8]; }
machine build_entry() { }
"#
        ),
        [("Unused".to_owned(), false)]
    );
    assert_eq!(
        refusal(
            r#"
data Unused<Payload> { count: u32; payload: Payload; }
machine build_entry() { }
"#
        ),
        "type generic data"
    );
}

/// Lifetime arguments are erased regions, so every way of writing such a
/// type, bare or at any lifetime, names the declaration's one record.
#[test]
fn every_spelling_of_lifetime_parameterised_data_is_one_record() {
    let verdict = judge_build(
        r#"
data Inner<'held> { count: u32; name: &'held [u8]; }
data Session<'storage> { ready: bool; inner: Inner<'storage>; }
data Other<'kept> { inner: Inner<'kept>; }
machine build_entry(session: &mut Session, other: &Other, bare: &Inner) {
    session.ready = count(&session.inner) == count(&other.inner) && count(bare) == 0;
}
machine count(inner: &Inner) -> u32 { inner.count }
"#,
    )
    .unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    accepted.publish_invocation().unwrap();
}
