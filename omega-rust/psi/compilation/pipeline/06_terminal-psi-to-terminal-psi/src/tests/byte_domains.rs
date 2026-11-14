//! A byte view in a domain, `reason: &[u8] in Utf8`, stored in a record.
//!
//! A domain whose one `requires` is a primitive byte predicate of `self`
//! (`valid_utf8`, `no_nul`, `ascii_only`) is decided by the bytes a view
//! shows. The lowered form carries those predicates on the field's own type
//! (`StructuralFieldType::PredicateByteSequence`), and the checker judges
//! every store into the field from the bytes its source supplies
//! (byte_views.md: "Checking rejoins the source's declared predicates or
//! literal contents"). A quoted literal supplies raw bytes and gains the
//! domain only by a cast (domains.md, "Byte containers and encoding
//! domains": "an explicit qualification must establish the selected library
//! predicate even when the known bytes permit compile-time proof").
//!
//! Each twin below differs from its partner only in the bytes it stores.

use super::build_purpose::judge_build;
use super::*;
use terminal_psi::{ByteSequencePredicate, StructuralFieldType, StructuralTypeShape};

/// `accepted` for a program the checker accepts and can publish; for one it
/// judges wrong, what it could not establish; else the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { unestablished, .. }) => match &unestablished[..] {
            [one] => one.detail.to_owned(),
            _ => panic!("{unestablished:?}"),
        },
        other => panic!("{other:?}"),
    }
}

/// A rejection reason stored in a case payload, as the standard library's
/// calling policies build one, with `bytes` as the quoted literal and
/// `predicate` as the domain's one clause.
fn rejection(predicate: &str, bytes: &str) -> String {
    format!(
        r#"
domain [u8]::Text
requires
    {predicate}(self);
data Rejection {{ reason: &[u8] in Text; }}
data Outcome {{
    case Accepted(value: u64);
    case Rejected(reason: Rejection);
}}
machine refuse() -> Outcome {{
    Outcome::Rejected {{ reason: Rejection {{ reason: "{bytes}" as +Text }} }}
}}
machine build_entry() {{
    let outcome: Outcome = refuse();
}}
"#
    )
}

const NOT_IN_DOMAIN: &str = "a byte view is stored into a field of a byte domain, and the \
                             bytes it shows are not in that domain";

#[test]
fn valid_text_is_stored_and_invalid_bytes_are_rejected() {
    assert_eq!(verdict(&rejection("valid_utf8", "no plan")), "accepted");
    assert_eq!(
        verdict(&rejection("valid_utf8", r"no \xff plan")),
        NOT_IN_DOMAIN
    );
}

#[test]
fn ascii_text_is_stored_and_other_text_is_rejected() {
    assert_eq!(verdict(&rejection("ascii_only", "plain")), "accepted");
    assert_eq!(verdict(&rejection("ascii_only", "café")), NOT_IN_DOMAIN);
    assert_eq!(verdict(&rejection("no_nul", "plain")), "accepted");
    assert_eq!(verdict(&rejection("no_nul", r"pla\x00in")), NOT_IN_DOMAIN);
}

/// The field is the view's own byte field with the domain's predicate on
/// it, and the view row is the one a plain `&[u8]` has.
#[test]
fn the_field_carries_its_predicates_on_its_type() {
    let Ok(RouteVerdict::Accepted(accepted)) = judge_build(&rejection("valid_utf8", "no plan"))
    else {
        panic!("not accepted");
    };
    let fields: Vec<&StructuralFieldType> = accepted
        .elaboration
        .module
        .structural_types
        .iter()
        .filter_map(|declared| match &declared.shape {
            StructuralTypeShape::Record { fields, .. } => Some(fields),
            _ => None,
        })
        .flatten()
        .map(|field| &field.field_type)
        .collect();
    let [StructuralFieldType::PredicateByteSequence { predicates, .. }] = fields[..] else {
        panic!("{fields:?}");
    };
    assert_eq!(
        predicates.predicates().collect::<Vec<_>>(),
        [ByteSequencePredicate::ValidUtf8]
    );
}

/// What this slice does not judge is refused by name, never accepted: the
/// literal without the cast that would give it the domain, a view in a
/// domain anywhere but a field, a domain that is no primitive byte
/// predicate, one the empty view is not in, and a store whose bytes come
/// from another field rather than a literal.
#[test]
fn what_no_bytes_decide_is_refused() {
    let declarations = r#"
domain [u8]::Text
requires
    valid_utf8(self);
data Rejection { reason: &[u8] in Text; }
"#;
    let uncast = format!(
        "{declarations}machine build_entry() {{ let r: Rejection = Rejection {{ reason: \"x\" }}; }}"
    );
    assert_eq!(verdict(&uncast), "typing record literal field type");
    let formal = format!(
        "{declarations}machine keep(text: &[u8] in Text) {{ }}\nmachine build_entry() {{ keep(\"x\" as +Text); }}"
    );
    assert_ne!(verdict(&formal), "accepted");
    let copied = format!(
        "{declarations}machine copy(from: &Rejection, into: &mut Rejection) {{ into.reason = from.reason; }}\n\
         machine build_entry() {{ let a: Rejection = Rejection {{ reason: \"x\" as +Text }}; let mut b: Rejection; copy(&a, &mut b); }}"
    );
    assert_ne!(verdict(&copied), "accepted");
    // Given the domain and at once relieved of it, the literal would reach a
    // plain field with its predicate judged nowhere.
    let laundered = r#"
domain [u8]::Text
requires
    valid_utf8(self);
data Plain { reason: &[u8]; }
machine build_entry() { let p: Plain = Plain { reason: ("\xff" as +Text) as -Text }; }
"#;
    assert_eq!(
        verdict(laundered),
        "expression removal of a byte predicate a quoted literal was just given"
    );
    let non_empty = r#"
domain [u8]::Filled
requires
    non_empty(self);
data Named { name: &[u8] in Filled; }
machine build_entry() { let n: Named = Named { name: "x" as +Filled }; }
"#;
    assert_eq!(
        verdict(non_empty),
        "type byte view in a domain the empty view is not in"
    );
}
