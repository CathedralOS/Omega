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

const NOT_IN_DOMAIN: &str = "a byte transfer's source does not establish its primitive \
                             byte-domain qualifications";

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

#[test]
fn byte_domain_membership_survives_locals_formals_and_call_results() {
    fn judged(source: &str) -> Result<RouteVerdict, crate::CompilationRefusal> {
        use super::build_purpose::{frontier_of, package, roster, symbol};
        let typed = frontier_of("machine product_entry() { }", source);
        let entries = [symbol(&typed, "build_entry")];
        let subjects = [Some(package())];
        let prepared = typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
            roster(&entries, &subjects, source::DependencyScope::Build),
            terminal_codec::TerminalPublication::Portable,
        );
        let mut verdicts = crate::judge_compilation(prepared.program);
        assert_eq!(verdicts.len(), 1);
        verdicts.remove(0)
    }
    let source = r#"
domain [u8]::Text requires no_nul(self);
data Choice { case First; case Second; }
machine same<'a>(text: &'a [u8] in Text, choice: Choice) -> &'a [u8] in Text {
    match choice {
        Choice::First -> text
        Choice::Second -> text
    }
}
machine take(text: &[u8] in Text) { }
machine build_entry() {
    let local: &[u8] in Text = "plain" as +Text;
    let returned: &[u8] in Text = same(local, Choice::First {});
    take(returned);
}
"#;
    let accepted = match judged(source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the declared membership must cross the actual call: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    assert!(module.structural_domains.iter().any(|domain| {
        domain.byte_predicate == Some(terminal_psi::ByteSequencePredicate::NoNul)
    }));
    assert!(module.machines.iter().any(|machine| {
        machine
            .contract
            .structural_parameters
            .iter()
            .any(|parameter| !parameter.qualifications.is_empty())
    }));
    assert!(module.machines.iter().any(|machine| matches!(
        &machine.contract.result,
        terminal_psi::TerminalMachineResult::Structural(result)
            if !result.qualifications.is_empty()
    )));
    let encoded = terminal_codec::encode_module(module)
        .expect("the actual package declarations have portable custody");
    terminal_codec::decode_module(&encoded)
        .expect("the actual established descriptor crosses receiving validation");
    let mut forged = module.clone();
    let mut corrupted = 0;
    for machine in &mut forged.machines {
        for block in &mut machine.blocks {
            for operation in &mut block.operations {
                if let terminal_psi::OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
                    &mut operation.kind
                    && bytes.as_slice() == b"plain"
                {
                    bytes[0] = 0;
                    corrupted += 1;
                }
            }
        }
    }
    assert_eq!(
        corrupted, 1,
        "corrupt the actual source of the qualification"
    );
    forged.unoptimized_behavior = forged
        .machines
        .iter()
        .map(terminal_psi::UnoptimizedMachineBehavior::capture)
        .collect();
    let refused = terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
        .expect_err("the receiving boundary must rejoin actual primitive bytes");
    assert!(
        matches!(
            refused,
            terminal_codec::CodecError::MalformedOperationFoundation {
                reason: "value transfer does not establish its byte domains",
                ..
            }
        ),
        "the corrupted transfer must fail its byte-domain establishment: {refused:?}"
    );
    accepted.publish().unwrap();

    let invalid = source.replace("\"plain\"", r#""pla\x00in""#);
    match judged(&invalid) {
        Ok(RouteVerdict::Rejected {
            violations,
            unestablished,
            ..
        }) => {
            let reasons = format!("{violations:?} {unestablished:?}");
            assert!(
                reasons.contains("byte")
                    || reasons.contains("predicate")
                    || reasons.contains("qualification"),
                "the exact missing NoNul fact must explain rejection: {reasons}",
            );
        }
        other => panic!("only the declared byte predicate was broken: {other:?}"),
    }
}

/// The field is the view's own byte field with the domain's predicate on
/// it, and the view row is the one a plain `&[u8]` has.
#[test]
fn the_field_carries_its_predicates_on_its_type() {
    let accepted = match judge_build(&rejection("valid_utf8", "no plan")) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the field must retain its exact byte predicate: {other:?}"),
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
/// literal without the cast that would give it the domain, a domain that
/// is no primitive byte predicate, or one the empty view is not in. A
/// previously given qualification must also be justified before it is erased.
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
    assert_eq!(verdict(&copied), "accepted");
    // Removing a qualification does not erase the earlier establishment's
    // obligation to justify that qualification for these exact bytes.
    let laundered = r#"
domain [u8]::Text
requires
    valid_utf8(self);
data Plain { reason: &[u8]; }
machine build_entry() { let p: Plain = Plain { reason: ("\xff" as +Text) as -Text }; }
"#;
    let judged = judge_build(laundered);
    assert!(
        matches!(&judged, Ok(RouteVerdict::Rejected { .. })),
        "{judged:?}"
    );
    let non_empty = r#"
domain [u8]::Filled
requires
    non_empty(self);
data Named { name: &[u8] in Filled; }
machine build_entry() { let n: Named = Named { name: "x" as +Filled }; }
"#;
    let refused = verdict(non_empty);
    assert!(
        refused.starts_with("type byte view in a domain the empty view is not in"),
        "{refused}"
    );
}
