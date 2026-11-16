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
        Ok(RouteVerdict::Rejected {
            violations,
            unestablished,
            ..
        }) => match (&violations[..], &unestablished[..]) {
            ([], [one]) => one.detail.to_owned(),
            ([one], []) => one.statement.to_owned(),
            _ => panic!("{violations:?} {unestablished:?}"),
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
data Rejection<'reason> {{ reason: &'reason [u8] in Text; }}
data Outcome<'reason> {{
    case Accepted(value: u64);
    case Rejected(reason: Rejection<'reason>);
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
/// literal without the cast that would give it the domain, a domain that is
/// no primitive byte predicate, one the empty view is not in, and a store
/// whose bytes come from another field rather than a literal.
#[test]
fn what_no_bytes_decide_is_refused() {
    let declarations = r#"
domain [u8]::Text
requires
    valid_utf8(self);
data Rejection<'reason> { reason: &'reason [u8] in Text; }
"#;
    let uncast = format!(
        "{declarations}machine build_entry() {{ let r: Rejection = Rejection {{ reason: \"x\" }}; }}"
    );
    // Not a refusal: a literal is plain bytes, and a type gains a domain
    // only by a cast, so the program is wrong and is rejected as such.
    assert!(
        verdict(&uncast).starts_with(
            "this value's type carries no domain and the field it initializes declares"
        ),
        "{}",
        verdict(&uncast)
    );
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
data Plain<'reason> { reason: &'reason [u8]; }
machine build_entry() { let p: Plain = Plain { reason: ("\xff" as +Text) as -Text }; }
"#;
    assert_eq!(
        verdict(laundered),
        "expression removal of a byte predicate a quoted literal was just given while reading the \
         local binding of `build_entry::entry::p` while reading the body of `build_entry`"
    );
    let non_empty = r#"
domain [u8]::Filled
requires
    non_empty(self);
data Named<'name> { name: &'name [u8] in Filled; }
machine build_entry() { let n: Named = Named { name: "x" as +Filled }; }
"#;
    assert_eq!(
        verdict(non_empty),
        "type byte view in a domain the empty view is not in while reading the record application \
         `named(name(nominal\\(package-owner\\(32:0707070707070707070707070707070707070707070707070707070707070707\\)\\,path\\(Named\\)\\)))`"
    );
}

#[test]
fn byte_predicate_spelling_needs_its_exact_domain_call() {
    const UNKNOWN: &str = "this call has no visible declaration in its source scope";
    for source in [
        "domain [u8]::Text requires missing_byte_predicate(self); machine build_entry() {}"
            .to_owned(),
        "domain [u8]::Text requires valid_utf8<u8>(self); machine build_entry() {}".to_owned(),
        "domain u64::Text requires valid_utf8(self); machine build_entry() {}".to_owned(),
        "domain [u16; 2]::Text requires valid_utf8(self); machine build_entry() {}".to_owned(),
        "machine build_entry(bytes: &[u8]) { let valid: bool = valid_utf8(bytes); }".to_owned(),
    ] {
        let reason = verdict(&source);
        assert!(reason.starts_with(UNKNOWN), "{source}\n{reason}");
    }

    let shadowed = format!(
        "machine valid_utf8(value: &[u8]) -> bool {{ false }}\n{}",
        rejection("valid_utf8", "plain")
    );
    let reason = verdict(&shadowed);
    assert!(
        reason.starts_with("type byte view in a domain that is not a primitive byte predicate"),
        "{shadowed}\n{reason}"
    );
}

/// A formal in a byte domain, `name: &[u8] in Name`, owes the domain's
/// primitive byte predicate at entry: every caller proves it of the bytes it
/// passes, and the body may rely on it (byte_views.md: "An ordinary or
/// boundary shared-byte argument may satisfy its formal's primitive byte
/// domains from these reconstructed predicates"). The forwarding call passes
/// the caller's own formal, whose entry requirement is that same predicate
/// of the same bytes. The twins differ only in the bytes the entry passes.
fn forwarded(bytes: &str) -> String {
    format!(
        r#"
domain [u8]::Name
requires
    no_nul(self);
machine keep(name: &[u8] in Name) {{ }}
machine forward(name: &[u8] in Name) {{ keep(name); }}
machine build_entry() {{ forward("{bytes}" as +Name); }}
"#
    )
}

#[test]
fn a_formal_in_a_byte_domain_requires_its_predicate_of_the_bytes_passed() {
    assert_eq!(verdict(&forwarded("plain")), "accepted");
    let Ok(RouteVerdict::Rejected { unproved, .. }) = judge_build(&forwarded(r"pla\x00in")) else {
        panic!("not rejected");
    };
    let statements: Vec<&str> = unproved.iter().map(|one| one.statement.as_str()).collect();
    assert_eq!(
        statements,
        [
            "cannot prove requires contract for call forward from build_entry: no_nul(self) \
          (needs fact `no_nul(self)` here; establish it before the call via a prior contract \
          guarantee, domain membership, or guard) in state `entry`"
        ]
    );
}

/// A fixed byte array in a byte domain, `bytes: [u8; 3] in Text`.
///
/// The array's length is in its type, so the domain's one clause is a
/// statement about its three bytes (domains.md: "`[u8; N] in D` is the same
/// exact `N`-byte fixed-array carrier as `[u8; N]`"). A cast proves that
/// statement from what is known about each byte, and a reader of the field
/// may rely on it. `bytes` are the array's three bytes as literals.
fn fixed_text(predicate: &str, bytes: &str) -> String {
    format!(
        r#"
domain [u8; 3]::Text
requires
    {predicate}(self);
data Holder {{ bytes: [u8; 3] in Text; }}
machine first(bytes: &[u8; 3] in Text) -> u8 {{ bytes[0] }}
machine build_entry() {{
    let plain: [u8; 3] = [{bytes}];
    let holder: Holder = Holder {{ bytes: plain as +Text }};
    let read: u8 = first(&holder.bytes);
}}
"#
    )
}

/// Whether the checker rejects the program for one obligation it cannot
/// prove, and for nothing else.
fn refused_cast(build: &str) -> bool {
    matches!(
        judge_build(build),
        Ok(RouteVerdict::Rejected { formations, violations, unproved, unestablished, conflicts, exceeded })
            if unproved.len() == 1
                && formations.is_empty()
                && violations.is_empty()
                && unestablished.is_empty()
                && conflicts.is_empty()
                && exceeded.is_empty()
    )
}

/// Each twin differs only in its bytes: `A` then `é` (`C3 A9`) is UTF-8, and
/// the same two bytes swapped are a stray continuation and a lead with
/// nothing after it.
#[test]
fn a_fixed_byte_array_is_cast_into_its_domain_only_when_its_bytes_are_in_it() {
    assert_eq!(
        verdict(&fixed_text("valid_utf8", "65u8, 0xC3u8, 0xA9u8")),
        "accepted"
    );
    assert!(refused_cast(&fixed_text(
        "valid_utf8",
        "65u8, 0xA9u8, 0xC3u8"
    )));
    assert_eq!(
        verdict(&fixed_text("ascii_only", "65u8, 66u8, 67u8")),
        "accepted"
    );
    assert!(refused_cast(&fixed_text(
        "ascii_only",
        "65u8, 0xC3u8, 0xA9u8"
    )));
    assert_eq!(
        verdict(&fixed_text("no_nul", "65u8, 66u8, 67u8")),
        "accepted"
    );
    assert!(refused_cast(&fixed_text("no_nul", "65u8, 0u8, 67u8")));
}

/// Bytes nothing is known about are never assumed to be in the domain.
#[test]
fn a_fixed_byte_array_of_unknown_bytes_is_not_cast_into_its_domain() {
    let unknown = r#"
domain [u8; 3]::Text
requires
    valid_utf8(self);
data Raw { bytes: [u8; 3]; }
data Holder { bytes: [u8; 3] in Text; }
machine relabel(raw: &Raw) -> u8 {
    let held: Holder = Holder { bytes: raw.bytes as +Text };
    held.bytes[0]
}
machine build_entry() {
    let raw: Raw = Raw { bytes: [65u8, 66u8, 67u8] };
    let read: u8 = relabel(&raw);
}
"#;
    let verdict = judge_build(unknown);
    let unproved_at_cast = matches!(
        &verdict,
        Err(crate::CompilationRefusal::Elaboration(refusal))
            if matches!(refusal.reason, terminal_check_tables::RefusalReason::Unproved(_))
    );
    assert!(
        unproved_at_cast || refused_cast(unknown),
        "{:?}",
        verdict.map(|_| ())
    );
}
