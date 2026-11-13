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

/// Eliding a lifetime relates nothing. lifetimes.md: "A signature does not
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
        "typing arm value type"
    );
    assert_eq!(
        refusal(
            r#"
data Session<'storage> { ready: bool; name: &'storage [u8]; }
machine build_entry(name: &[u8]) { let made: Session = helper(name); }
machine helper(name: &[u8]) -> Session { Session { ready: true, name: name } }
"#
        ),
        "typing record literal field type"
    );
}
