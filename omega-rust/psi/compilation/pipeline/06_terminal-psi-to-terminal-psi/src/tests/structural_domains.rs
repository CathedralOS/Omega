//! A record's domains are part of its lowered type, so the checker judges
//! them without the reader: a ticket stamped by its one issuer, kept in a
//! field, read back out and stripped. Each twin removes or forges one piece
//! of the lowered form and must be refused, though its payload is unchanged.

use super::*;

const STAMPED: &str = r#"
data Ticket { number: u32; }
domain Ticket::Stamped established by stamp;
data Stub { ticket: Ticket in Stamped; count: u32; }
machine stamp(ticket: Ticket) -> Ticket in Stamped { ticket as +Stamped }
machine holder(stub: Stub) -> Ticket in Stamped { stub.ticket }
machine entry() -> u32 {
    let stamped: Ticket in Stamped = stamp(Ticket { number: 7u32 });
    let stub: Stub = Stub { ticket: stamped, count: 1u32 };
    let back: Ticket in Stamped = holder(stub);
    let plain: Ticket = back as -Stamped;
    plain.number
}
"#;

fn invocation(text: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let typed = boundary_calls::typed(text);
    let entry = boundary_calls::entry(&typed);
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    typed_trees_to_terminal_psi::elaborate_for(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn refused(elaboration: typed_trees_to_terminal_psi::Elaboration, expected: &str) {
    let refusal = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(
        refusal.reason,
        RefusalReason::ForeignIdentity,
        "{refusal:?}"
    );
    assert!(refusal.detail.contains(expected), "{refusal:?}");
}

#[test]
fn a_persistent_reference_carries_the_exact_referent_domain() {
    use terminal_interpreter::{
        TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalScalarValue,
    };

    let source = r#"
data Ticket { number: u32; }
domain Ticket::Stamped established by stamp;
machine stamp(ticket: Ticket) -> Ticket in Stamped { ticket as +Stamped }
machine number(ticket: &Ticket in Stamped) -> u32 { ticket.number }
machine entry() -> u32 {
    let stamped: Ticket in Stamped = stamp(Ticket { number: 7u32 });
    let saved: &Ticket in Stamped = &stamped;
    number(saved)
}
"#;
    let mut elaboration = invocation(source).unwrap();
    let domain = elaboration.module.structural_domains[0].id;
    let formed = elaboration
        .module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishReference { .. }
            )
        })
        .expect("source forms the persistent reference");
    let terminal_psi::OperationResult::Structural(result) = &mut formed.result else {
        panic!("forming a reference yields its descriptor");
    };
    assert!(result.qualifications.is_empty());
    assert_eq!(
        result.projected_qualifications,
        [terminal_psi::StructuralPathQualification {
            path: vec![terminal_psi::StructuralPathSegment::Referent],
            domain,
        }]
    );
    result.projected_qualifications.clear();
    let encoded = terminal_codec::ClosedTerminalModule::new(elaboration.module.clone()).unwrap();
    let detail = "a reference does not keep its referent's exact domains";
    assert!(matches!(
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes()),
        Err(terminal_codec::CodecError::MalformedStructuralFoundation(reason)) if reason == detail
    ));
    refused(elaboration, detail);
    let verdict = judge(invocation(source)).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the unmodified reference must check: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(matches!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(7),
                ..
            }
        ))
    ));
}

#[test]
fn array_element_domains_are_checked_at_establishment_and_at_runtime_index() {
    use terminal_psi::{
        OperationKind, OperationResult, StructuralPathQualification, StructuralPathSegment,
    };

    let source = r#"
data Ticket { number: u32; }
domain Ticket::Stamped established by stamp;
machine stamp(ticket: Ticket) -> Ticket in Stamped { ticket as +Stamped }
machine number(ticket: &Ticket in Stamped) -> u32 { ticket.number }
machine select(tickets: &[Ticket in Stamped; 2], index: u64) -> u32
requires index < 2u64
{
    number(&tickets[index])
}
machine entry() -> u32 {
    let tickets: [Ticket in Stamped; 2] = [stamp(Ticket { number: 3u32 }), stamp(Ticket { number: 5u32 })];
    select(&tickets, 1u64)
}
"#;
    let mut elaboration = invocation(source).unwrap();
    let domain = elaboration.module.structural_domains[0].id;
    let array = elaboration
        .module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::EstablishStructuralArray { .. }
            )
        })
        .expect("the source establishes an array");
    let OperationResult::Structural(result) = &mut array.result else {
        panic!("the establishment yields a structural result");
    };
    assert!(result.qualifications.is_empty());
    assert_eq!(
        result.projected_qualifications,
        [StructuralPathQualification {
            path: vec![StructuralPathSegment::AllElements],
            domain,
        }]
    );
    result.projected_qualifications.clear();
    let encoded = terminal_codec::ClosedTerminalModule::new(elaboration.module.clone()).unwrap();
    let detail = "a collection element does not deliver its declared domains";
    let rejected =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes());
    assert!(
        matches!(&rejected, Err(terminal_codec::CodecError::MalformedOperationFoundation { reason, .. })
            if *reason == "structural field store does not restore the selected memberships"),
        "{rejected:?}",
    );
    refused(elaboration, detail);

    assert_unsigned_invocation(source, 5);
}

#[test]
fn zeroed_arrays_keep_unrestricted_record_element_domains() {
    let source = r#"
data Ticket { number: u32; }
domain Ticket::Stamped;
machine entry() -> u32 {
    let mut tickets: [Ticket in Stamped; 2];
    tickets[1u64].number
}
"#;
    assert_unsigned_invocation(source, 0);
    let predicated = source.replace(
        "domain Ticket::Stamped;",
        "domain Ticket::Stamped requires self.number > 0u32;",
    );
    let refusal = invocation(&predicated).unwrap_err();
    assert!(
        refusal
            .detail
            .contains("zero-established value in a domain that must be established"),
        "{refusal:?}"
    );
}

fn assert_unsigned_invocation(source: &str, expected: u128) {
    use terminal_interpreter::{
        TerminalExecutionResult, TerminalExecutionStatus, TerminalProgram, TerminalScalarValue,
    };

    let verdict = judge(invocation(source)).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("the qualified array must check: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let actual = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(1_000))
        .unwrap();
    assert!(
        matches!(
            actual,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                }
            )) if value == expected
        ),
        "{actual:?}"
    );
}

#[test]
fn the_lowered_record_type_retains_its_domains_and_casts() {
    let elaboration = invocation(STAMPED).unwrap();
    let module = &elaboration.module;
    assert_eq!(module.structural_domains.len(), 1, "one record domain");
    assert!(
        module.structural_domains[0].establishment_routes.is_some(),
        "the issuer restriction survives"
    );
    // The issuer's cast names its route; the erasure names none.
    let changes = &module.structural_qualification_changes;
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert_eq!(
        changes
            .iter()
            .filter(|change| !change.establishments.is_empty())
            .count(),
        1
    );
    // A formal of the record holding the qualified field states that
    // field's domain beneath it.
    assert!(module.machines.iter().any(|machine| {
        machine
            .structural_parameters
            .iter()
            .any(|formal| !formal.projected_qualifications.is_empty())
    }));
    let lowered = elaboration.module.clone();
    let verdict = judge(Ok(elaboration)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    // The encoded module keeps the same rows, and artifact ingress, which
    // runs the same judgment, reads them back unchanged.
    let closed = terminal_codec::ClosedTerminalModule::new(lowered.clone()).unwrap();
    let decoded =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(closed.semantic_bytes()).unwrap();
    let decoded = decoded.module();
    assert_eq!(decoded.structural_domains, lowered.structural_domains);
    assert_eq!(
        decoded.structural_qualification_changes,
        lowered.structural_qualification_changes
    );
    for (decoded, lowered) in decoded.machines.iter().zip(&lowered.machines) {
        assert_eq!(decoded.structural_parameters, lowered.structural_parameters);
        assert_eq!(decoded.result, lowered.result);
    }
}

#[test]
fn a_qualified_parameter_does_not_replace_its_cast_edge() {
    let mut elaboration = invocation(STAMPED).unwrap();
    elaboration.module.structural_qualification_changes.clear();
    refused(
        elaboration,
        "a structural successor does not deliver its parameter's exact domains",
    );
}

#[test]
fn a_routed_cast_needs_its_issuer_route() {
    let mut elaboration = invocation(STAMPED).unwrap();
    for change in &mut elaboration.module.structural_qualification_changes {
        change.establishments.clear();
    }
    refused(
        elaboration,
        "a structural cast adds a routed domain without its issuer route",
    );
}

#[test]
fn an_empty_issuer_set_permits_no_cast() {
    let mut elaboration = invocation(STAMPED).unwrap();
    elaboration.module.structural_domains[0].establishment_routes = Some(Vec::new());
    refused(
        elaboration,
        "a structural cast is not an authorized issuer of its domain",
    );
}

#[test]
fn a_field_membership_cannot_be_dropped_from_one_formal() {
    let mut elaboration = invocation(STAMPED).unwrap();
    let formal = elaboration
        .module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.contract.structural_parameters)
        .find(|formal| !formal.projected_qualifications.is_empty())
        .unwrap();
    formal.projected_qualifications.clear();
    // The body's read of the field and the caller's argument each disagree
    // with the forged formal; whichever machine is judged first refuses it.
    refused(elaboration, "exact domains");
}

#[test]
fn a_move_cannot_carry_an_unchecked_operation_issuer_row() {
    let source = STAMPED
        .replace("machine holder(stub:", "machine transport(ticket: Ticket) -> Ticket in Stamped { stamp(ticket) }\nmachine holder(stub:")
        .replace("stamp(Ticket { number: 7u32 })", "transport(Ticket { number: 7u32 })");
    let mut elaboration = invocation(&source).unwrap();
    let domain = elaboration.module.structural_domains[0].id;
    let caller = elaboration
        .module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .declaration
                .as_ref()
                .and_then(|declaration| elaboration.module.identities.text(declaration.callable))
                .is_some_and(|identity| identity.contains("transport"))
        })
        .unwrap();
    let operation = caller
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::MoveStructuralValue { .. }
            )
        })
        .expect("the ordinary caller moves the plain ticket to the issuer");
    let terminal_psi::OperationResult::Structural(result) = &mut operation.result else {
        panic!("the move has a structural result");
    };
    assert!(result.qualifications.is_empty() && result.qualification_establishments.is_empty());
    result
        .qualification_establishments
        .push(terminal_psi::ResultQualificationEstablishment { domain, route: 0 });
    let operation = operation.id;
    let encoded = terminal_codec::ClosedTerminalModule::new(elaboration.module.clone()).unwrap();
    let Err(error) =
        terminal_codec::ClosedTerminalModule::from_semantic_bytes(encoded.semantic_bytes())
    else {
        panic!("artifact ingress must refuse the unchecked claim");
    };
    assert!(
        matches!(
            error,
            terminal_codec::CodecError::MalformedOperationFoundation {
                operation: received,
                reason: "owned value transfer requires owned custody and cannot issue authority",
                ..
            } if received == operation
        ),
        "{error:?}"
    );
    refused(
        elaboration,
        "a structural issuer claim on an operation requires a boundary completion",
    );
}

#[test]
fn a_value_match_keeps_each_arriving_records_domain() {
    const SOURCE: &str = r#"
data Ticket { number: u32; }
domain Ticket::Stamped established by stamp;
machine stamp(ticket: Ticket) -> Ticket in Stamped { ticket as +Stamped }
machine choose(test: bool, left: Ticket in Stamped, right: Ticket in Stamped) -> Ticket in Stamped {
    let selected: Ticket in Stamped = match test { true -> left, _ -> right };
    selected
}
machine entry() -> u32 {
    let selected: Ticket in Stamped =
        choose(false, stamp(Ticket { number: 3u32 }), stamp(Ticket { number: 5u32 }));
    selected.number
}
"#;
    let elaboration = invocation(SOURCE).unwrap();
    let module = &elaboration.module;
    let domain = module.structural_domains[0].id;
    let choose = module
        .machines
        .iter()
        .find(|machine| {
            machine
                .declaration
                .as_ref()
                .and_then(|declaration| module.identities.text(declaration.callable))
                .is_some_and(|identity| identity.contains("choose"))
        })
        .expect("the chosen machine owns the match");
    assert!(
        choose
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .any(|parameter| parameter.qualifications == [domain])
    );
    let verdict = judge(Ok(elaboration)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    let invalid = SOURCE.replace("_ -> right }", "_ -> right as -Stamped }");
    let refusal = invocation(&invalid).unwrap_err();
    assert!(
        refusal.detail.contains("the match's common result type")
            && refusal
                .detail
                .contains("a type gains or loses a domain only"),
        "{refusal:?}"
    );
}

const NORMAL_MEMBERSHIP: &str = r#"// Expected exit: 0

data Ticket { number: u32; }
domain Ticket::Stamped requires self.number > 0u32 established by stamp;
machine stamp(ticket: Ticket) -> Ticket
requires ticket.number > 0u32
ensures result in Stamped {
    let tagged: Ticket in Stamped = Ticket { number: ticket.number } as +Stamped;
    tagged as -Stamped
}
machine record_value(number: u32) -> u32 requires number > 0u32 {
    let plain: Ticket = stamp(Ticket { number: number });
    let tagged: Ticket in Stamped = plain as +Stamped;
    tagged.number
}
machine other(ticket: Ticket) -> Ticket { ticket }
domain u32::WordStamped established by stamp_word;
machine stamp_word(number: u32) -> u32 ensures result in WordStamped {
    let tagged: u32 in WordStamped = number as +WordStamped;
    tagged as -WordStamped
}
machine scalar_value(number: u32) -> u32 {
    let plain: u32 = stamp_word(number);
    let tagged: u32 in WordStamped = plain as +WordStamped;
    tagged as -WordStamped
}
machine entry() -> u32 crashes Trap {
    let first: u32 = record_value(7u32);
    let second: u32 = record_value(11u32);
    let first_word: u32 = scalar_value(7u32);
    let second_word: u32 = scalar_value(11u32);
    transition first == 7u32 && second == 11u32 && first_word == 7u32 && second_word == 11u32 {
        true -> 0u32
        false -> wrong()
    }
    machine wrong() -> u32 crashes Trap { crash Trap; }
}
"#;

#[test]
fn normal_result_membership_is_a_fact_without_changing_the_declared_result() {
    let elaboration = invocation(NORMAL_MEMBERSHIP).unwrap();
    let contracts: Vec<_> = elaboration
        .module
        .machines
        .iter()
        .filter(|machine| !machine.contract.membership_ensures.is_empty())
        .collect();
    assert_eq!(contracts.len(), 2);
    for machine in contracts {
        match &machine.result {
            terminal_psi::TerminalMachineResult::Structural(result) => {
                assert!(result.qualifications.is_empty())
            }
            terminal_psi::TerminalMachineResult::Scalar(result) => {
                assert_eq!(result.qualifications, Default::default())
            }
            _ => panic!("membership issuer has an actual runtime result"),
        }
    }
    let verdict = judge(Ok(elaboration)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn normal_result_membership_rejects_missing_facts_and_wrong_subjects() {
    let base = invocation(NORMAL_MEMBERSHIP).unwrap();
    let mut cases = Vec::new();
    let mut missing = invocation(NORMAL_MEMBERSHIP).unwrap();
    for machine in &mut missing.module.machines {
        machine.contract.membership_ensures.retain(|row| {
            !matches!(
                row,
                terminal_psi::RuntimeDomainMembership::Structural { .. }
            )
        });
    }
    cases.push(("missing guarantee", missing));
    let mut wrong = base;
    for machine in &mut wrong.module.machines {
        for membership in &mut machine.contract.membership_ensures {
            if let terminal_psi::RuntimeDomainMembership::Structural { root, .. } = membership {
                *root = machine.structural_parameters[0].place;
            }
        }
    }
    cases.push(("input named as result", wrong));
    let observed: Vec<_> = cases
        .into_iter()
        .map(|(name, elaboration)| (name, judge(Ok(elaboration))))
        .collect();
    for (name, verdict) in &observed {
        eprintln!("{name}: {verdict:?}");
    }
    for (name, verdict) in observed {
        let refusal = verdict.expect_err(name);
        let expected = match name {
            "input named as result" => "membership promise does not name its exact normal result",
            _ => "lacks both an existing membership and an authorized issuer route",
        };
        assert!(refusal.detail.contains(expected), "{name}: {refusal:?}");
    }
}

#[test]
fn native_normal_membership_matches_the_authored_service_guarantee() {
    let text = r#"
data Ticket { number: u32; }
domain Ticket::Stamped established by Seal::issue;
boundary trait Seal {
    machine issue(ticket: Ticket) -> Ticket
    reaches none
    ensures result in Stamped
    crashes none;
}
machine entry() -> u32 crashes none {
    let ticket: Ticket = Seal::issue(Ticket { number: 7u32 });
    let stamped: Ticket in Stamped = ticket as +Stamped;
    stamped.number
}
"#;
    let elaboration = invocation(text).unwrap();
    let module = elaboration.module;
    let boundary = &module.boundary_machines[0];
    let method = &module.service_schemas[0].surface.methods[0];
    assert_eq!(method.normal_result_memberships.len(), 1);
    terminal_semantics::validate_boundary_normal_memberships(&module, boundary, method).unwrap();
    let mut missing = boundary.clone();
    missing.membership_ensures.clear();
    let mut changed = module.clone();
    let mut identities = changed.identities.clone().into_builder();
    changed.structural_domains[0].identity = identities.intern("another-exact-domain-application");
    changed.identities = identities.build();
    let mut changed_commitment = changed.boundary_machines[0].clone();
    changed_commitment
        .declaration
        .as_mut()
        .unwrap()
        .source
        .contract = [9; 32];
    let observed = [
        (
            "missing row",
            terminal_semantics::validate_boundary_normal_memberships(&module, &missing, method),
        ),
        (
            "changed domain",
            terminal_semantics::validate_boundary_normal_memberships(
                &changed,
                &changed.boundary_machines[0],
                method,
            ),
        ),
        (
            "changed domain and header commitment",
            terminal_semantics::validate_boundary_normal_memberships(
                &changed,
                &changed_commitment,
                method,
            ),
        ),
    ];
    for (name, result) in &observed {
        eprintln!("{name}: {result:?}");
    }
    for (name, result) in observed {
        assert!(
            result
                .unwrap_err()
                .contains("authored normal-result membership guarantees"),
            "{name}"
        );
    }
}
