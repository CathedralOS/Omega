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
fn a_case_literal_retains_other_cases_conditional_domains() {
    use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment};

    let source = r#"
domain [u8]::Text requires valid_utf8(self);
data Rejection<'reason> { reason: &'reason [u8] in Text; }
data Outcome<'reason> {
    case Accepted(value: &'reason [u8]);
    case Rejected(reason: Rejection<'reason>);
}
machine accept() -> Outcome { Outcome::Accepted { value: "plan" } }
machine build_entry() { let outcome: Outcome = accept(); }
"#;
    let accepted = match judge_build(source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("only the active case supplies storage: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    fn constructor(module: &mut terminal_psi::TerminalModule) -> &mut terminal_psi::Operation {
        module
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::EstablishStructuralCase { .. }
                )
            })
            .expect("the source establishes its Accepted case")
    }
    let mut retained = module.clone();
    let operation = constructor(&mut retained);
    let OperationKind::EstablishStructuralCase { result_case, .. } = &operation.kind else {
        unreachable!();
    };
    let active_case = *result_case;
    let OperationResult::Structural(result) = &operation.result else {
        panic!("the constructor produces an Outcome");
    };
    let [conditional] = result.projected_qualifications.as_slice() else {
        panic!("the complete result keeps its rejected reason's Text domain: {result:?}");
    };
    let [
        StructuralPathSegment::Case(inactive_case),
        StructuralPathSegment::Field(_),
        StructuralPathSegment::Field(_),
    ] = conditional.path.as_slice()
    else {
        panic!("the qualification names the exact nested rejected reason: {conditional:?}");
    };
    assert_ne!(active_case, *inactive_case);
    let conditional_path = conditional.path.clone();
    let active_field = module
        .structural_type(result.structural_type)
        .unwrap()
        .shape
        .cases()
        .iter()
        .find(|case| case.id == active_case)
        .unwrap()
        .fields[0]
        .identity;
    assert_eq!(
        module.structural_domains[0].byte_predicate,
        Some(ByteSequencePredicate::ValidUtf8)
    );
    accepted.publish_invocation().unwrap();

    let mut active = module.clone();
    let active_path = vec![
        StructuralPathSegment::Case(active_case),
        StructuralPathSegment::Field(active_field),
    ];
    // Keep transfers consistent with the forged claim, so the constructor
    // itself must reject the domain its plain field initializer cannot supply.
    let redirect = |rows: &mut [terminal_psi::StructuralPathQualification]| {
        for row in rows {
            if row.path == conditional_path {
                row.path.clone_from(&active_path);
            }
        }
    };
    for machine in &mut active.machines {
        if let terminal_psi::TerminalMachineResult::Structural(result) = &mut machine.result {
            redirect(&mut result.projected_qualifications);
        }
        for parameter in &mut machine.structural_parameters {
            redirect(&mut parameter.projected_qualifications);
        }
        for block in &mut machine.blocks {
            for parameter in &mut block.structural_parameters {
                redirect(&mut parameter.projected_qualifications);
            }
            for operation in &mut block.operations {
                if let OperationResult::Structural(result) = &mut operation.result {
                    redirect(&mut result.projected_qualifications);
                }
            }
        }
    }
    let error = terminal_semantics::structural_qualifications::validate_shape(&active).unwrap_err();
    assert_eq!(
        error.detail,
        "a structural literal's field does not deliver its declared domains"
    );

    let mut unknown_case = conditional_path.clone();
    unknown_case[0] =
        StructuralPathSegment::Case(semantic_vocabulary::StructuralCaseId::new(u64::MAX).unwrap());
    let mut unknown_field = conditional_path;
    unknown_field[1] = StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(u32::MAX));
    for path in [unknown_case, unknown_field] {
        let mut forged = module.clone();
        let OperationResult::Structural(result) = &mut constructor(&mut forged).result else {
            unreachable!();
        };
        result.projected_qualifications[0].path = path;
        let error =
            terminal_semantics::structural_qualifications::validate_shape(&forged).unwrap_err();
        assert_eq!(
            error.detail,
            "a structural place's domain names an absent path"
        );
    }
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
        assert_eq!(
            verdicts.len(),
            1,
            "the requested build_entry compilation returned these complete verdicts: {verdicts:#?}",
        );
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
    let mut forged_theory = module.clone();
    let domain = forged_theory
        .structural_domains
        .iter_mut()
        .find(|domain| domain.byte_predicate == Some(terminal_psi::ByteSequencePredicate::NoNul))
        .expect("the source retains its primitive NoNul domain");
    let theory = domain
        .predicate
        .as_mut()
        .expect("the primitive domain retains its complete theory");
    theory.proposition =
        Proposition::Conjunction(vec![theory.proposition.clone(), Proposition::Falsehood]);
    let refused = terminal_semantics::structural_qualifications::validate_shape(&forged_theory)
        .expect_err("an octet predicate cannot establish an unrelated generic clause");
    assert_eq!(
        refused.detail, "a byte qualification change is not a complete primitive byte domain",
        "{refused:?}"
    );
    let refused = terminal_codec::decode_module(
        &terminal_codec::encode_module(&forged_theory)
            .expect("the theory mutation retains portable source custody"),
    )
    .expect_err("artifact ingress must retain every domain clause");
    assert!(
        matches!(
            refused,
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "a byte qualification change is not a complete primitive byte domain"
            ) | terminal_codec::CodecError::MalformedOperationFoundation {
                reason: "byte transfer qualification is not a complete primitive byte domain",
                ..
            }
        ),
        "the unsupported extra theory must explain refusal: {refused:?}"
    );
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
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "a byte transfer's source does not establish its primitive byte-domain qualifications"
            )
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
    // Independent input regions cannot promise the stored view survives in
    // the other record. State their common referent lifetime for the copy.
    assert_eq!(
        verdict(&copied),
        "typing assigned value type while reading the body of `copy`"
    );
    let copied = copied.replacen(
        "copy(from: &Rejection, into: &mut Rejection)",
        "copy<'reason>(from: &Rejection<'reason>, into: &mut Rejection<'reason>)",
        1,
    );
    assert_eq!(verdict(&copied), "accepted");
    // Removing a qualification does not erase the earlier establishment's
    // obligation to justify that qualification for these exact bytes.
    let laundered = r#"
domain [u8]::Text
requires
    valid_utf8(self);
data Plain<'reason> { reason: &'reason [u8]; }
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
data Named<'name> { name: &'name [u8] in Filled; }
machine build_entry() { let n: Named = Named { name: "x" as +Filled }; }
"#;
    let refused = verdict(non_empty);
    assert!(
        refused.starts_with("type byte view in a domain the empty view is not in"),
        "{refused}"
    );

    // The default owner must distinguish an empty descriptor from the one
    // zero octet in this actual parsed and lowered fixed-array field.
    let default_source = format!("data FixedDefault {{ bytes: [u8; 1]; }}\n{copied}").replacen(
        "let mut b: Rejection;",
        "let mut b: Rejection; let zeros: FixedDefault;",
        1,
    );
    let accepted = match judge_build(&default_source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the ordinary fixed default must form: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    let (root, field, array) = module
        .machines
        .iter()
        .flat_map(|machine| machine.operations())
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishZeroedStructural {}
            )
        })
        .find_map(|operation| {
            let result = operation.result.structural()?;
            let record = module.structural_type(result.structural_type)?;
            record.shape.record_fields()?.iter().find_map(|field| {
                let array = module.structural_type(field.field_type.as_structural()?)?;
                matches!(
                    array.shape,
                    StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(1),
                        ..
                    }
                )
                .then_some((record.id, field.identity, array.id))
            })
        })
        .expect("the source emitted its zeroed fixed-array record");
    let mut domain = module
        .structural_domains
        .iter()
        .find(|domain| domain.byte_predicate == Some(ByteSequencePredicate::ValidUtf8))
        .expect("the source retains its complete primitive byte theory")
        .clone();
    domain.carrier = array;
    let qualification = terminal_psi::StructuralPathQualification {
        path: vec![terminal_psi::StructuralPathSegment::Field(field)],
        domain: domain.id,
    };
    // These are inputs to the shared default predicate query, not a claim
    // that a changed domain row belongs to an accepted artifact.
    for (predicate, expected) in [
        (ByteSequencePredicate::AsciiOnly, true),
        (ByteSequencePredicate::ValidUtf8, true),
        (ByteSequencePredicate::NoNul, false),
    ] {
        domain.byte_predicate = Some(predicate);
        let theory = domain
            .predicate
            .as_mut()
            .expect("complete primitive theory");
        let mut required = terminal_psi::ByteSequencePredicateSet::EMPTY;
        required.insert(predicate);
        theory.proposition = terminal_semantics::byte_predicate_requirement(
            required,
            terminal_psi::ByteSequencePredicateSet::EMPTY,
            theory.subject,
            &[],
        );
        assert_eq!(
            terminal_semantics::zeroed_projected_byte_qualifications_hold(
                module.structural_types.iter(),
                std::iter::once(&domain),
                root,
                std::slice::from_ref(&qualification),
            ),
            expected,
            "the actual fixed-array zero default under {predicate:?}",
        );
    }
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

fn sliced_cast(predicate: &str, source_domain: &str, bytes: &str) -> String {
    format!(
        r#"
domain [u8]::Text requires {predicate}(self);
machine length(text: &[u8] in Text) -> u64 {{ text.len }}
machine prefix_length(text: &[u8]{source_domain}, end: u64) -> u64
requires end <= text.len {{
    let prefix: &[u8] = &text[0..end];
    length(prefix as +Text)
}}
machine build_entry() {{
    let measured: u64 = prefix_length({bytes}, 1);
}}
"#
    )
}

#[test]
fn a_subslice_cast_requires_the_exact_backing_to_preserve_its_predicate() {
    assert_eq!(
        verdict(&sliced_cast("no_nul", " in Text", r#""pl" as +Text"#)),
        "accepted"
    );
    for source in [
        sliced_cast("no_nul", "", r#""\x00l""#),
        sliced_cast("valid_utf8", " in Text", r#""é" as +Text"#),
    ] {
        let Ok(RouteVerdict::Rejected { unproved, .. }) = judge_build(&source) else {
            panic!("the cast must be rejected: {source}");
        };
        assert!(
            unproved.iter().any(|obligation| {
                matches!(obligation.kind, ObligationKind::ArrivalInvariant { .. })
                    && obligation
                        .statement
                        .contains("cannot prove the value of this cast is in the domain it adds")
            }),
            "{unproved:?}"
        );
    }
}

#[test]
fn a_slice_uses_the_byte_predicate_of_the_current_loop_parameter() {
    let source = r#"
domain [u8]::Text requires no_nul(self);
machine length(text: &[u8] in Text) -> u64 { text.len }
machine prefix_length(text: &[u8] in Text, end: u64, remaining: u32) -> u64
requires end <= text.len {
    let prefix: &[u8] = &text[0..end];
    let measured: u64 = length(prefix as +Text);
    transition remaining > 0 {
        true -> prefix_length(text, end, remaining - 1)
        false -> measured
    }
}
machine build_entry() {
    let measured: u64 = prefix_length("plain" as +Text, 2, 3);
}
"#;
    assert_eq!(verdict(source), "accepted");
    let unknown = source
        .replace(" in Text, end:", ", end:")
        .replace("\"plain\" as +Text", "\"plain\"");
    let Ok(RouteVerdict::Rejected { unproved, .. }) = judge_build(&unknown) else {
        panic!("an unqualified loop parameter cannot establish the slice's byte predicate");
    };
    assert!(
        unproved.iter().any(|obligation| {
            matches!(obligation.kind, ObligationKind::ArrivalInvariant { .. })
                && obligation
                    .statement
                    .contains("cannot prove the value of this cast is in the domain it adds")
        }),
        "{unproved:?}"
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
