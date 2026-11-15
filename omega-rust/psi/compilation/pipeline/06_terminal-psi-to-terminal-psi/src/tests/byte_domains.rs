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
use semantic_vocabulary::StructuralCaseId;
use terminal_psi::{
    ByteSequencePredicate, StructuralFieldType, StructuralPathSegment, StructuralTypeShape,
};

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

fn rejects_byte_cast(source: &str, predicate: &str) {
    let found = judge_build(source);
    // False Boolean equalities currently produce an incomplete proof verdict.
    // Require the actual cast obligation so a source error cannot stand in
    // for checking the bytes.
    let unproved = match &found {
        Ok(RouteVerdict::Rejected { unproved, .. } | RouteVerdict::Incomplete(unproved)) => {
            unproved
        }
        _ => panic!("{source}\n{found:?}"),
    };
    assert!(
        unproved.iter().any(|failure| matches!(
            failure.kind,
            terminal_check_tables::obligations::ObligationKind::ArrivalInvariant { .. }
        ) && failure.statement.contains(predicate)),
        "{source}\n{found:?}"
    );
}

#[test]
fn forming_a_byte_qualification_does_not_prove_its_contents() {
    let declarations = "domain [u8]::Text requires no_nul(self);\n";
    for body in [
        "machine build_entry() { let bytes: &[u8] in Text = \"safe\" as +Text; }",
        "machine text() -> &[u8] in Text { \"safe\" as +Text }\nmachine build_entry() { let bytes: &[u8] in Text = text(); }",
    ] {
        for nul in [false, true] {
            let source = format!("{declarations}{body}");
            let source = if nul {
                source.replace("\"safe\"", "\"sa\\0e\"")
            } else {
                source
            };
            if nul {
                rejects_byte_cast(&source, "no_nul");
            } else {
                let found = judge_build(&source);
                let Ok(RouteVerdict::Accepted(accepted)) = found else {
                    panic!("{source}\n{found:?}");
                };
                accepted.publish_invocation().unwrap();
                let mut forged = accepted.elaboration.module.clone();
                let invariant = forged
                    .scalar_block_invariants
                    .iter_mut()
                    .find(|invariant| {
                        let mut observes_bytes = false;
                        invariant.predicate.visit_scalar_terms(|term| {
                            observes_bytes |=
                                matches!(term, ScalarTerm::ByteSequencePredicate { .. });
                        });
                        observes_bytes
                    })
                    .expect("the cast retains its actual byte obligation");
                invariant.predicate = Proposition::Truth;
                assert_eq!(
                    terminal_semantics::structural_qualifications::validate(&forged)
                        .unwrap_err()
                        .detail,
                    "a structural cast lost its declared predicate arrival obligation"
                );
            }
        }
    }
    let implicit =
        format!("{declarations}machine build_entry() {{ let bytes: &[u8] in Text = \"safe\"; }}");
    let found = judge_build(&implicit);
    assert!(
        matches!(&found, Ok(RouteVerdict::Rejected { violations, .. }) if violations.iter().any(|failure|
        failure.statement.contains("the local it initializes declares"))),
        "{found:?}"
    );
    let unsupported = verdict(
        "domain [u8]::Text requires non_empty(self); machine keep(bytes: &[u8] in Text) {} machine build_entry() {}",
    );
    assert!(
        unsupported.starts_with("type byte view in a domain the empty view is not in"),
        "{unsupported}"
    );
    let mutable = verdict(
        "domain [u8]::Text requires no_nul(self); machine keep(bytes: &mut [u8] in Text) {} machine build_entry() {}",
    );
    assert!(
        mutable.starts_with("type mutable byte view that carries a domain"),
        "{mutable}"
    );
}

#[test]
fn a_zeroed_shared_byte_field_retains_only_conditions_of_the_empty_view() {
    let source = r#"
domain [u8]::Text requires no_nul(self);
data Message<'a> { bytes: &'a [u8] in Text; }
machine build_entry() { let message: Message; let length: u64 = message.bytes.len; }
"#;
    let found = judge_build(source);
    let Ok(RouteVerdict::Accepted(accepted)) = found else {
        panic!("{found:?}");
    };
    accepted.publish_invocation().unwrap();
    let mut forged = accepted.elaboration.module.clone();
    let domain = forged
        .structural_domains
        .iter_mut()
        .find(|domain| domain.byte_predicate == Some(ByteSequencePredicate::NoNul))
        .expect("the field and whole descriptor use the same primitive domain row");
    domain.byte_predicate = Some(ByteSequencePredicate::NonEmpty);
    assert_eq!(
        terminal_semantics::structural_qualifications::validate(&forged)
            .unwrap_err()
            .detail,
        "zero provisioning cannot establish a structural domain"
    );
}

#[test]
fn valid_text_is_stored_and_invalid_bytes_are_rejected() {
    assert_eq!(verdict(&rejection("valid_utf8", "no plan")), "accepted");
    rejects_byte_cast(&rejection("valid_utf8", r"no \xff plan"), "valid_utf8");
}

#[test]
fn ascii_text_is_stored_and_other_text_is_rejected() {
    assert_eq!(verdict(&rejection("ascii_only", "plain")), "accepted");
    rejects_byte_cast(&rejection("ascii_only", "café"), "ascii_only");
    assert_eq!(verdict(&rejection("no_nul", "plain")), "accepted");
    rejects_byte_cast(&rejection("no_nul", r"pla\x00in"), "no_nul");
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

#[test]
fn a_case_literal_owes_only_its_active_fields_without_losing_conditional_domains() {
    let rejected =
        rejection("valid_utf8", "no plan").replace("case Accepted(value: u64);", "case Accepted;");
    let accepted = rejected.replace(
        "Outcome::Rejected { reason: Rejection { reason: \"no plan\" as +Text } }",
        "Outcome::Accepted {}",
    );
    assert_ne!(accepted, rejected);
    for (source, has_qualified_payload) in [(&accepted, false), (&rejected, true)] {
        let Ok(RouteVerdict::Accepted(checked)) = judge_build(source) else {
            panic!("{source}\n{:?}", judge_build(source));
        };
        checked.publish_invocation().unwrap();
        let mut forged = checked.elaboration.module.clone();
        let operation = forged
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishScalarCase { .. }
                        | OperationKind::EstablishStructuralCase { .. }
                ) && operation
                    .result
                    .structural()
                    .is_some_and(|result| !result.projected_qualifications.is_empty())
            })
            .expect("the case keeps its full conditional field roster");
        if has_qualified_payload {
            let OperationKind::EstablishStructuralCase { fields, .. } = &mut operation.kind else {
                panic!("the rejected case has its record payload");
            };
            fields.clear();
            assert_eq!(
                terminal_semantics::structural_qualifications::validate(&forged)
                    .unwrap_err()
                    .detail,
                "a structural literal declares a domain no field initializer supplies"
            );
        } else {
            let OperationResult::Structural(result) = &mut operation.result else {
                unreachable!();
            };
            let StructuralPathSegment::Case(case) = &mut result.projected_qualifications[0].path[0]
            else {
                panic!("the qualified field belongs to the sibling case");
            };
            *case = StructuralCaseId::new(999_999).unwrap();
            assert_eq!(
                terminal_semantics::structural_qualifications::validate(&forged)
                    .unwrap_err()
                    .detail,
                "a structural place's domain has a different carrier"
            );
        }
    }
}

/// Forwarding a qualified byte field also needs the declared payload lifetime.
/// Byte truth supplies neither that relation nor an implicit source cast.
/// Domains that exclude the empty default view remain unsupported.
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
    let formal = format!(
        "{declarations}machine keep(text: &[u8] in Text) {{ }}\nmachine build_entry() {{ keep(\"x\" as +Text); }}"
    );
    assert_eq!(verdict(&formal), "accepted");
    let copied = format!(
        "{declarations}machine copy(from: &Rejection, into: &mut Rejection) {{ into.reason = from.reason; }}\n\
         machine build_entry() {{ let a: Rejection = Rejection {{ reason: \"x\" as +Text }}; let mut b: Rejection; copy(&a, &mut b); }}"
    );
    assert_eq!(
        verdict(&copied),
        "typing assigned value type while reading the body of `copy`"
    );
    let related = copied.replace(
        "machine copy(from: &Rejection, into: &mut Rejection)",
        "machine copy<'reason>(from: &Rejection<'reason>, into: &mut Rejection<'reason>)",
    );
    assert_eq!(verdict(&related), "accepted");
    // Given the domain and at once relieved of it, the literal would reach a
    // plain field with its predicate judged nowhere.
    let laundered = r#"
domain [u8]::Text
requires
    valid_utf8(self);
data Plain<'reason> { reason: &'reason [u8]; }
machine build_entry() { let p: Plain = Plain { reason: ("\xff" as +Text) as -Text }; }
"#;
    rejects_byte_cast(laundered, "valid_utf8");
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
    ] {
        let reason = verdict(&source);
        assert!(reason.starts_with(UNKNOWN), "{source}\n{reason}");
    }

    assert_eq!(
        verdict("domain u64::Text requires valid_utf8(self); machine build_entry() {}"),
        "contract byte predicate without its exact storage subject"
    );
    // Primitive identity permits contract reading; it does not provide a
    // runtime implementation for an ordinary Boolean expression.
    let runtime =
        verdict("machine build_entry(bytes: &[u8]) { let valid: bool = valid_utf8(bytes); }");
    assert!(
        runtime.starts_with("call without a bound target"),
        "{runtime}"
    );

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
