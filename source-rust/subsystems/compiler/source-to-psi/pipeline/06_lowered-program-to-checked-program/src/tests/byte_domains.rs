//! A byte view in a domain, `reason: @[u8] in Utf8`, stored in a record.
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

use super::*;
use terminal_psi::{ByteSequencePredicate, StructuralFieldType, StructuralTypeShape};

const BYTE_OBSERVATION_SOURCE: &[(&str, &str, &str)] = &[(
    "byte_observation/byte_observation.omg",
    "byte_observation",
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source/library/core/byte_observation/byte_observation.omg"
    )),
)];

pub(super) fn judge_build(build: &str) -> Result<RouteVerdict, crate::CompilationRefusal> {
    let source =
        format!("use {{omega}}/language/core/byte_observation/byte_observation.omg;\n{build}");
    super::build_purpose::judge_build_with_core(&source, BYTE_OBSERVATION_SOURCE)
}

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
        Ok(
            ref rejected @ RouteVerdict::Rejected {
                ref violations,
                ref unestablished,
                ..
            },
        ) => match (&violations[..], &unestablished[..]) {
            ([], [one]) => one.detail.to_owned(),
            ([one], []) => one.statement.to_owned(),
            _ => panic!("{rejected:?}"),
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
    |byte_observation|::{predicate}(@self[..]);
data Rejection<'reason> {{ reason: @'reason [u8] in Text; }}
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

fn judge_portable_byte_build(
    source: &str,
) -> Result<RouteVerdict, crate::CompilationRefusal> {
    use super::build_purpose::{frontier_with_core, package, roster, symbol};
    let source =
        format!("use {{omega}}/language/core/byte_observation/byte_observation.omg;\n{source}");
    let typed = crate::prepare_fixture(frontier_with_core(
        "machine product_entry() { }",
        &source,
        source_provenance::DependencyScope::Build,
        BYTE_OBSERVATION_SOURCE,
    ));
    let entries = [symbol(typed.trees(), "build_entry")];
    let subjects = [Some(package())];
    let prepared = crate::lowering::prepare_compilation(
        &typed,
        roster(
            &entries,
            &subjects,
            source_provenance::DependencyScope::Build,
        ),
        terminal_codec::TerminalPublication::Portable,
    )
    .expect("fixture must lower to checker input");
    let mut verdicts = crate::judge_compilation(prepared.program);
    assert_eq!(
        verdicts.len(),
        1,
        "the requested build_entry compilation returned these complete verdicts: {verdicts:#?}",
    );
    verdicts.remove(0)
}

const NOT_IN_DOMAIN: &str = "a byte transfer's source does not establish its primitive \
                             byte-domain qualifications";

#[test]
fn valid_text_is_stored_and_invalid_bytes_are_rejected() {
    assert_eq!(verdict(&rejection("valid_utf8", "no plan")), "accepted");
    let accepted = match judge_portable_byte_build(&rejection("valid_utf8", "no plan")) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the same literal must form a portable checked program: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    terminal_codec::decode_module(&terminal_codec::encode_module(module).unwrap())
        .expect("the complete primitive theory crosses the actual receiving boundary");
    let mut incomplete = module.clone();
    incomplete
        .types.structural_domains
        .iter_mut()
        .find(|domain| domain.byte_predicate == Some(ByteSequencePredicate::ValidUtf8))
        .expect("the literal retains its exact primitive domain")
        .complete_predicate_theory = false;
    let refused = terminal_semantics::structural_qualifications::validate_shape(&incomplete)
        .expect_err("known bytes cannot complete an unavailable domain theory");
    assert!(
        refused.detail.contains("complete primitive byte domain"),
        "{refused:?}"
    );
    let refused =
        terminal_codec::decode_module(&terminal_codec::encode_module(&incomplete).unwrap())
            .expect_err("receiving must reject the same incomplete establishment");
    assert!(
        format!("{refused:?}").contains("complete primitive byte domain"),
        "{refused:?}"
    );
    accepted.publish().unwrap();
    assert_eq!(
        verdict(&rejection("valid_utf8", r"no \xff plan")),
        NOT_IN_DOMAIN
    );
}

#[test]
fn a_case_literal_retains_other_cases_conditional_domains() {
    use terminal_psi::{OperationKind, OperationResult, StructuralPathSegment};

    let source = r#"
domain [u8]::Text requires |byte_observation|::valid_utf8(@self[..]);
data Rejection<'reason> { reason: @'reason [u8] in Text; }
data Outcome<'reason> {
    case Accepted(value: @'reason [u8]);
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
        module.types.structural_domains[0].byte_predicate,
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
fn a_callable_byte_result_keeps_only_its_public_predicates() {
    let source = r#"
domain [u8]::Text requires |byte_observation|::valid_utf8(@self[..]);
trait Window {
    machine view<'input>(packet: @'input [u8] in Text) -> @'input [u8] in Text;
}
Direct satisfies Window {
    machine view<'input>(packet: @'input [u8] in Text) -> @'input [u8] in Text { packet }
}
data ViewPacket = machine(packet: @[u8] in Text) -> @[u8] in Text reaches none crashes none;
machine build_entry() -> bool {
    let packet: @[u8] in Text = "é" as +Text;
    let selected: ViewPacket = Direct::view;
    let viewed: @[u8] in Text = selected(packet);
    (viewed as -Text) == "é"
}
"#;
    let accepted = match judge_build(source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the callable's public result retains its byte domain: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    let (caller, place, contract) = module
        .machines
        .iter()
        .find_map(|machine| {
            machine.operations().find_map(|operation| {
                let terminal_psi::OperationKind::CallMachine { contract, .. } = operation.kind
                else {
                    return None;
                };
                Some((machine.id, operation.result.structural()?.place, contract))
            })
        })
        .expect("the source invokes its retained machine value");
    let predicates = terminal_semantics::byte_sequence_source_predicates(
        module,
        module.machine(caller).unwrap(),
        place,
    )
    .expect("the public contract authenticates the actual result");
    assert!(
        predicates
            .predicates()
            .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
    );

    let mut changed_result = module.clone();
    let result = changed_result
        .machines
        .iter_mut()
        .find(|machine| machine.id == caller)
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            operation
                .result
                .structural()
                .is_some_and(|result| result.place == place)
        })
        .unwrap();
    let terminal_psi::OperationResult::Structural(result) = &mut result.result else {
        unreachable!()
    };
    result.qualifications.clear();
    assert!(
        terminal_semantics::byte_sequence_source_predicates(
            &changed_result,
            changed_result.machine(caller).unwrap(),
            place,
        )
        .is_none(),
        "an operation cannot change the public result's qualifications"
    );

    let mut changed_header = module.clone();
    let callable = changed_header
        .execution.callables
        .contracts
        .iter_mut()
        .find(|row| row.id == contract)
        .unwrap();
    let terminal_psi::TerminalMachineResult::Structural(result) = &mut callable.proof_result else {
        unreachable!()
    };
    result.qualifications.clear();
    assert!(
        terminal_semantics::byte_sequence_source_predicates(
            &changed_header,
            changed_header.machine(caller).unwrap(),
            place,
        )
        .is_none(),
        "disagreeing public result columns establish no predicate"
    );

    let mut missing_header = module.clone();
    missing_header.execution.callables.contracts.clear();
    assert!(
        terminal_semantics::byte_sequence_source_predicates(
            &missing_header,
            missing_header.machine(caller).unwrap(),
            place,
        )
        .is_none(),
        "an absent contract cannot be replaced by the operation annotation"
    );

    let publication = accepted.publish_invocation().unwrap();
    let program =
        terminal_interpreter::TerminalProgram::borrow(publication.artifact.view()).unwrap();
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Boolean(true)
                )
            )
        ),
        "{status:?}"
    );
}

#[test]
fn byte_domain_membership_survives_locals_formals_and_call_results() {
    let source = r#"
domain [u8]::Text requires |byte_observation|::no_nul(@self[..]);
data Choice { case First; case Second; }
machine same<'a>(text: @'a [u8] in Text, choice: Choice) -> @'a [u8] in Text {
    match choice {
        Choice::First -> text,
        Choice::Second -> text
    }
}
machine take(text: @[u8] in Text) { }
machine build_entry() {
    let local: @[u8] in Text = "plain" as +Text;
    let returned: @[u8] in Text = same(local, Choice::First {});
    take(returned);
}
"#;
    let accepted = match judge_portable_byte_build(source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the declared membership must cross the actual call: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    assert!(module.types.structural_domains.iter().any(|domain| {
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
        .types.structural_domains
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
    forged.execution.unoptimized_behavior = forged
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
    match judge_portable_byte_build(&invalid) {
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
/// it, and the view row is the one a plain `@[u8]` has.
#[test]
fn the_field_carries_its_predicates_on_its_type() {
    let accepted = match judge_build(&rejection("valid_utf8", "no plan")) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the field must retain its exact byte predicate: {other:?}"),
    };
    let fields: Vec<&StructuralFieldType> = accepted
        .elaboration
        .module
        .types.structural_types
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
/// literal without its named cast, incompatible loan lifetimes, or a
/// primitive domain the empty view is not in. Removing a qualification
/// must still check the cast that originally established it.
#[test]
fn what_no_bytes_decide_is_refused() {
    let declarations = r#"
domain [u8]::Text
requires
    |byte_observation|::valid_utf8(@self[..]);
data Rejection<'reason> { reason: @'reason [u8] in Text; }
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
        "{declarations}machine keep(text: @[u8] in Text) {{ }}\nmachine build_entry() {{ keep(\"x\" as +Text); }}"
    );
    assert_eq!(verdict(&formal), "accepted");
    let copied = format!(
        "{declarations}machine copy(from: @Rejection, into: @mutable Rejection) {{ into.reason = from.reason; }}\n\
         machine build_entry() {{ let a: Rejection = Rejection {{ reason: \"x\" as +Text }}; let mutable b: Rejection; copy(@a, @mutable b); }}"
    );
    // Independent input regions cannot promise the stored view survives in
    // the other record. State their common referent lifetime for the copy.
    assert_eq!(
        verdict(&copied),
        "this view is of the region of another lifetime of the machine than the place it is assigned to names: nothing relates two lifetimes of one machine while reading the body of `copy`"
    );
    let copied = copied.replacen(
        "copy(from: @Rejection, into: @mutable Rejection)",
        "copy<'reason>(from: @Rejection<'reason>, into: @mutable Rejection<'reason>)",
        1,
    );
    assert_eq!(verdict(&copied), "accepted");
    // Removing a qualification does not erase the earlier establishment's
    // obligation to justify that qualification for these exact bytes.
    let laundered = r#"
domain [u8]::Text
requires
    |byte_observation|::valid_utf8(@self[..]);
data Plain<'reason> { reason: @'reason [u8]; }
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
    |byte_observation|::non_empty(@self[..]);
data Named<'name> { name: @'name [u8] in Filled; }
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
        "let mutable b: Rejection;",
        "let mutable b: Rejection; let zeros: FixedDefault;",
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
        .types.structural_domains
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
                module.types.structural_types.iter(),
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
        "domain [u8]::Text requires valid_utf8::<u8>(self); machine build_entry() {}".to_owned(),
        "domain u64::Text requires valid_utf8(self); machine build_entry() {}".to_owned(),
        "domain [u16; 2]::Text requires valid_utf8(self); machine build_entry() {}".to_owned(),
        "machine build_entry(bytes: @[u8]) { let valid: bool = valid_utf8(bytes); }".to_owned(),
    ] {
        let reason = verdict(&source);
        assert!(reason.starts_with(UNKNOWN), "{source}\n{reason}");
    }

    let shadowed = format!(
        "machine valid_utf8(value: @[u8]) -> bool {{ false }}\n{}",
        rejection("valid_utf8", "plain").replace(r"|byte_observation|::valid_utf8", r"valid_utf8")
    );
    let source =
        format!("use {{omega}}/language/core/byte_observation/byte_observation.omg;\n{shadowed}");
    let typed = super::build_purpose::frontier_with_core(
        "machine product_entry() {}",
        &source,
        source_provenance::DependencyScope::Build,
        BYTE_OBSERVATION_SOURCE,
    );
    let domains: Vec<_> = typed
        .domain_definitions()
        .iter()
        .filter(|domain| {
            matches!(
                typed
                    .type_reference_table
                    .type_reference(domain.target_type),
                crate::source_state::types::TypeReferenceNode::Slice { .. }
            )
        })
        .collect();
    let [domain] = domains.as_slice() else {
        panic!("the fixture declares exactly one slice domain: {domains:?}");
    };
    assert!(
        crate::source_state::byte_predicates::byte_slice_domain_predicate(&typed, domain).is_none(),
        "a user declaration with the same spelling has no primitive byte meaning"
    );
    // Ordinary predicate translation now reads this helper. Its borrowed
    // subject still needs lifetime support; this is not proof of its false
    // predicate, and must not be counted as a primitive byte rejection.
    let reason = verdict(&shadowed);
    assert!(
        reason.starts_with("typing data field that elides a view's lifetime")
            && reason.contains("the domain predicate"),
        "{shadowed}\n{reason}"
    );
}

/// A formal in a byte domain, `name: @[u8] in Name`, owes the domain's
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
    |byte_observation|::no_nul(@self[..]);
machine keep(name: @[u8] in Name) {{ }}
machine forward(name: @[u8] in Name) {{ keep(name); }}
machine build_entry() {{ forward("{bytes}" as +Name); }}
"#
    )
}

#[test]
fn a_formal_in_a_byte_domain_requires_its_predicate_of_the_bytes_passed() {
    assert_eq!(verdict(&forwarded("plain")), "accepted");
    // The explicit qualification must form before `forward` can use it.
    let Ok(RouteVerdict::Rejected { unproved, .. }) = judge_build(&forwarded(r"pla\x00in")) else {
        panic!("not rejected");
    };
    let statements: Vec<&str> = unproved.iter().map(|one| one.statement.as_str()).collect();
    assert_eq!(
        statements,
        [
            "cannot prove the value of this cast is in the domain it adds in machine `build_entry` \
             state `entry`: domain `[u8]::Name` requires `|byte_observation|::no_nul(@self[..])` \
             (needs that fact about the operand here; establish it before the cast via a guard, \
             a prior contract guarantee or a `requires` of this machine)"
        ]
    );
}

fn sliced_cast(predicate: &str, source_domain: &str, bytes: &str) -> String {
    let collection = if source_domain.is_empty() { "text" } else { "(text as -Text)" };
    format!(
        r#"
domain [u8]::Text requires |byte_observation|::{predicate}(@self[..]);
machine length(text: @[u8] in Text) -> u64 {{ text.length }}
machine prefix_length(text: @[u8]{source_domain}, end: u64) -> u64
requires end <= text.length {{
    let prefix: @[u8] = @{collection}[0..end];
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
fn removing_a_domain_keeps_the_backing_bytes_of_a_prefix() {
    let prefix = |predicate, bytes| sliced_cast(predicate, " in Text", bytes);
    for predicate in ["no_nul", "ascii_only"] {
        assert_eq!(verdict(&prefix(predicate, r#""pl" as +Text"#)), "accepted");
    }
    let Ok(RouteVerdict::Rejected { unproved, .. }) =
        judge_build(&prefix("valid_utf8", r#""é" as +Text"#))
    else {
        panic!("a prefix may split a UTF-8 sequence");
    };
    assert!(unproved.iter().any(|row| {
        row.statement
            .contains("cannot prove the value of this cast")
    }));
}

#[test]
fn a_slice_uses_the_byte_predicate_of_the_current_loop_parameter() {
    let source = r#"
domain [u8]::Text requires |byte_observation|::no_nul(@self[..]);
machine length(text: @[u8] in Text) -> u64 { text.length }
machine prefix_length(text: @[u8] in Text, end: u64, remaining: u32) -> u64
requires end <= text.length {
    let prefix: @[u8] = @(text as -Text)[0..end];
    let measured: u64 = length(prefix as +Text);
    transition remaining > 0 {
        true -> prefix_length(text, end, remaining - 1),
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
        .replace("(text as -Text)", "text")
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
    |byte_observation|::{predicate}(@self[..]);
data Holder {{ bytes: [u8; 3] in Text; }}
machine first(bytes: @[u8; 3] in Text) -> u8 {{ (bytes as -Text)[0] }}
machine build_entry() {{
    let plain: [u8; 3] = [{bytes}];
    let holder: Holder = Holder {{ bytes: plain as +Text }};
    let read: u8 = first(@holder.bytes);
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
    |byte_observation|::valid_utf8(@self[..]);
data Raw { bytes: [u8; 3]; }
data Holder { bytes: [u8; 3] in Text; }
machine relabel(raw: @Raw) -> u8 {
    let held: Holder = Holder { bytes: raw.bytes as +Text };
    (@held.bytes as -Text)[0]
}
machine build_entry() {
    let raw: Raw = Raw { bytes: [65u8, 66u8, 67u8] };
    let read: u8 = relabel(@raw);
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

#[test]
fn a_shared_byte_view_join_checks_the_bytes_from_each_branch() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/views/run/shared_byte_view_join/main.omg"
    ));
    let invalid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/views/fail/shared_byte_view_join/main.omg"
    ));
    let build_source = |source: &str| {
        source
            .replace(
                "use {omega}/language/core/byte_observation/byte_observation.omg;",
                "",
            )
            .replace("machine entry_start()", "machine build_entry()")
    };
    match judge_portable_byte_build(&build_source(valid)) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            let bytes = terminal_codec::encode_module(&accepted.elaboration.module).unwrap();
            terminal_codec::decode_module(&bytes).unwrap();
        }
        other => panic!("both branches supply ASCII bytes: {other:?}"),
    }
    let verdict = judge_portable_byte_build(&build_source(invalid)).unwrap();
    assert!(
        matches!(&verdict, RouteVerdict::Rejected {
            formations, violations, unproved, unestablished, conflicts, exceeded,
        } if formations.is_empty()
            && violations.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
            && unproved.len() == 1
            && unproved[0].statement.contains("ascii_only")
            && unestablished.len() == 1
            && unestablished[0].detail.contains("byte-domain qualifications")),
        "one branch supplies a non-ASCII byte: {verdict:?}"
    );
}

#[test]
fn a_shared_array_prefix_uses_its_own_backing_membership() {
    let valid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/views/run/shared_fixed_array_prefix/main.omg"
    ));
    let invalid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/views/fail/shared_fixed_array_prefix/main.omg"
    ));
    let build_source = |source: &str| {
        source
            .replace(
                "use {omega}/language/core/byte_observation/byte_observation.omg;",
                "",
            )
            .replace("machine entry_start()", "machine build_entry()")
    };
    match judge_portable_byte_build(&build_source(valid)) {
        Ok(RouteVerdict::Accepted(accepted)) => {
            let bytes = terminal_codec::encode_module(&accepted.elaboration.module).unwrap();
            terminal_codec::decode_module(&bytes).unwrap();
        }
        other => panic!("the borrowed array field supplies the ASCII prefix: {other:?}"),
    }
    let verdict = judge_portable_byte_build(&build_source(invalid)).unwrap();
    assert!(
        matches!(&verdict, RouteVerdict::Rejected {
            formations, violations, unproved, unestablished, conflicts, exceeded,
        } if formations.is_empty()
            && violations.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
            && unproved.len() == 1
            && unproved[0].statement.contains("ascii_only")
            && unestablished.len() == 1
            && unestablished[0].detail.contains("byte-domain qualifications")),
        "the adjacent unqualified array supplies no ASCII proof: {verdict:?}"
    );
}

#[test]
fn a_byte_read_keeps_ascii_facts_about_its_own_view() {
    let source = r#"
domain [u8]::Ascii requires |byte_observation|::ascii_only(@self[..]);

machine ascii_byte(bytes: @[u8] in Ascii, other: @[u8], index: u64) -> u8
requires index < bytes.length, index < other.length
ensures result <= 127
{
    (bytes as -Ascii)[index]
}

machine build_entry() {
    let byte: u8 = ascii_byte("A1" as +Ascii, "\xff2", 0);
}
"#;
    assert_eq!(verdict(source), "accepted");
    let other_source = source.replace("(bytes as -Ascii)[index]", "other[index]");
    let verdict = judge_build(&other_source).unwrap();
    assert!(
        matches!(&verdict, RouteVerdict::Rejected {
            formations, violations, unproved, unestablished, conflicts, exceeded,
        } if formations.is_empty()
            && violations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
            && unproved.len() == 1
            && unproved[0].statement.contains("result <= 127")),
        "the other view does not establish the same byte bound: {verdict:?}"
    );
}

#[test]
fn a_variable_byte_store_preserves_the_buffers_utf8_domain() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/views/run/mutable_domain_buffer/main.omg"
    ))
    .replace("use {omega}/language/core/byte_observation/byte_observation.omg;", "")
    .replace("machine main()", "machine build_entry()");
    let multibyte = source.replace("ABCD", r"\xc3\xa9CD");
    for valid in [&source, &multibyte] {
        assert_eq!(verdict(valid), "accepted");
    }
    for invalid in [
        source.replace("] = 48;", "] = 128;"),
        multibyte.replace("index: 13 as", "index: 1 as"),
        multibyte.replace(
            "    (@mutable packet.line",
            "    packet.index = 1 as +Wrapping;\n    (@mutable packet.line",
        ),
    ] {
        let verdict = judge_build(&invalid).unwrap();
        assert!(
            matches!(verdict, RouteVerdict::Rejected {
            ref formations, ref violations, ref unproved, ref unestablished, ref conflicts, ref exceeded,
        } if formations.is_empty()
            && violations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty()
            && !unproved.is_empty()
            && unproved.iter().all(|row| matches!(row.kind, ObligationKind::DataDomain { .. }))),
            "the write must fail Utf8 restoration: {verdict:?}"
        );
    }
}

#[test]
fn a_false_edge_retires_a_copyable_view_before_its_backing() {
    let source = r#"
data Packet { before: u8; bytes: [u8; 3]; after: u8; }
machine build_entry() crashes Trap {
    let packet: Packet = Packet { before: 11, bytes: [65u8, 66u8, 67u8], after: 37 };
    let view: @[u8] = (@packet.bytes)[..];
    transition view.length == 3 && view[1] == 66 && packet.before == 11 && packet.after == 37 {
        true -> done(),
        _ -> bad()
    }
    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#;
    let verdict = judge_build(source).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}")
    };
    let encoded = terminal_codec::ClosedTerminalModule::new(accepted.elaboration.module).unwrap();
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes())
        .expect("the receiving checker must recognize the same carried reference custody");
}

/// A declared length rule is an ordinary domain predicate. Casts prove it
/// for the actual descriptor; a call also requires that exact domain name.
#[test]
fn shared_byte_view_domains_check_lengths_and_keep_their_names() {
    let source = r#"
domain [u8]::FourBytes requires self.length == 4;
machine length(input: @[u8] in FourBytes) -> u64 {
    (input as -FourBytes).length
}
machine build_entry() {
    let view: @[u8] in FourBytes = "room" as +FourBytes;
    let count: u64 = length(view);
}
"#;
    assert_eq!(verdict(source), "accepted");
    let accepted = match judge_portable_byte_build(source) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        other => panic!("the declared length must be proved and published: {other:?}"),
    };
    let module = &accepted.elaboration.module;
    terminal_codec::decode_module(&terminal_codec::encode_module(module).unwrap())
        .expect("the ordinary domain crosses independent artifact validation");
    let mut forged = module.clone();
    for domain in &mut forged.types.structural_domains {
        if domain.byte_predicate.is_none()
            && let Some(predicate) = &mut domain.predicate
        {
            predicate.proposition = Proposition::Falsehood;
        }
    }
    terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
        .expect_err("a changed domain theory cannot reuse the original cast proof");
    accepted.publish().unwrap();
    let wrong_length = source.replace("\"room\" as +FourBytes", "\"rooms\" as +FourBytes");
    match judge_build(&wrong_length) {
        Ok(RouteVerdict::Rejected {
            formations,
            violations,
            unproved,
            unestablished,
            conflicts,
            exceeded,
        }) => {
            assert!(
                formations.is_empty()
                    && violations.is_empty()
                    && unestablished.is_empty()
                    && conflicts.is_empty()
                    && exceeded.is_empty()
            );
            assert_eq!(unproved.len(), 1, "{unproved:?}");
            assert!(matches!(
                unproved[0].kind,
                terminal_check_tables::obligations::ObligationKind::ArrivalInvariant { .. }
            ));
            assert!(
                unproved[0].statement.contains("FourBytes")
                    && unproved[0].statement.contains("self.length == 4"),
                "{unproved:?}"
            );
        }
        other => panic!("the longer literal must fail only its length-domain cast: {other:?}"),
    }
    let plain = source.replace("length(view)", "length(\"room\")");
    let reason = verdict(&plain);
    assert!(
        reason.contains("domain") && reason.contains("FourBytes"),
        "{reason}"
    );
}
