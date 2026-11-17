//! Removing a qualification keeps the value and its facts, without granting
//! permission to issue that qualification again.

use super::*;

#[test]
fn the_lowered_scalar_type_retains_its_declared_domain() {
    // This is the ordinary static unit-domain transport used by the kept
    // qualified_operator_home program, without depending on operator selection.
    let elaboration = invocation(UNIT_DOMAIN).unwrap();
    assert_eq!(
        elaboration.module.scalar_qualifications.domains.len(),
        1,
        "the checker must receive the nominal domain, not only its erased carrier"
    );
    assert!(
        elaboration.module.machines.iter().any(|machine| {
            machine.parameters.iter().any(|parameter| {
                parameter.qualifications != semantic_vocabulary::ScalarQualificationSetId::ZERO
            })
        }),
        "a qualified formal must retain its complete declared type"
    );
    let verdict = judge(Ok(elaboration)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

const UNIT_DOMAIN: &str = r#"
domain u32::Span;
machine relay(distance: u32 in Span) -> u32 in Span { distance }
machine entry() -> u32 { relay(7u32 as +Span) as -Span }
"#;

#[test]
fn the_checker_rejects_a_missing_nominal_argument_even_with_the_same_carrier() {
    let mut elaboration = invocation(UNIT_DOMAIN).unwrap();
    let caller = elaboration
        .module
        .machines
        .iter_mut()
        .find(|machine| {
            machine.operations().any(|operation| {
                matches!(
                    operation.kind,
                    terminal_psi::OperationKind::IntegerConstant { .. }
                )
            })
        })
        .unwrap();
    let bare = caller
        .operations()
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant { .. }
            )
        })
        .unwrap()
        .result
        .scalar_ref()
        .unwrap()
        .id;
    let call = caller
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.declared_invocation().is_some())
        .unwrap();
    let terminal_psi::OperationKind::Call { arguments, .. } = &mut call.kind else {
        panic!("the relay has an ordinary scalar call")
    };
    arguments[0] = bare;
    let refusal = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert!(
        refusal
            .detail
            .contains("scalar call argument does not preserve its complete declared type"),
        "{refusal:?}"
    );
}

#[test]
fn a_qualified_result_annotation_does_not_replace_its_cast_edge() {
    let mut elaboration = invocation(UNIT_DOMAIN).unwrap();
    elaboration.module.scalar_qualifications.coercions.clear();
    let refusal = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert!(
        refusal.detail.contains(
            "scalar successor changes membership without its explicit cast or checked completion"
        ),
        "{refusal:?}"
    );
}

#[test]
fn a_forged_completion_cannot_issue_a_domain_whose_issuer_set_is_empty() {
    use terminal_psi::{
        ScalarDomainPredicate, ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite,
    };
    let mut elaboration = invocation(UNIT_DOMAIN).unwrap();
    let catalog = &mut elaboration.module.scalar_qualifications;
    catalog.domains[0].establishment_routes = Some(Vec::new());
    catalog.domains[0].predicate = Some(ScalarDomainPredicate {
        subject: semantic_vocabulary::ValueId::new(1).unwrap(),
        proposition: Proposition::Truth,
    });
    let introduced = catalog
        .coercions
        .iter()
        .find(|edge| {
            elaboration
                .module
                .machines
                .iter()
                .find(|machine| machine.id == edge.machine)
                .unwrap()
                .block_parameters()
                .any(|value| value.id == edge.destination && !value.qualifications.is_empty())
        })
        .unwrap();
    catalog.returns.push(ScalarReturnQualification {
        site: ScalarReturnSite::Completion {
            argument_ordinal: introduced.argument_ordinal,
        },
        machine: introduced.machine,
        edge: introduced.edge,
        source: introduced.source,
        result: introduced.destination,
        domain: catalog.domains[0].id,
        origins: vec![ScalarReturnOrigin::Vacuous],
    });
    let refusal = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert!(refusal.detail.contains("authorized issuer"), "{refusal:?}");
}

const DOMAIN: &str = r#"
domain i64::Classified
requires self != -1
established by issue;
machine issue(value: i64) -> i64 in Classified
requires value != -1
ensures (result as -Classified) == value
{
    value as +Classified
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

fn consumer(actual: i64, requirement: &str) -> String {
    format!(
        r#"
{DOMAIN}
machine consume(handle: i64 in Classified) -> i64
requires {requirement}
ensures result != -1
{{
    handle as -Classified
}}
machine entry() -> i64 {{
    let handle: i64 in Classified = issue({actual});
    consume(handle)
}}
"#
    )
}

#[test]
fn explicit_domain_removal_keeps_the_call_precondition_and_value_fact() {
    let requirement = "(handle as -Classified) != 0";
    let accepted = judge(invocation(&consumer(7, requirement))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    // Zero satisfies Classified's predicate, but not consume's independent
    // nonzero precondition. The domain cannot establish that extra fact.
    let refused = judge(invocation(&consumer(0, requirement))).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{refused:?}"
    );
}

#[test]
fn a_boundary_formal_keeps_its_domain_and_independent_requirement() {
    let source = |actual| {
        format!(
            r#"
{DOMAIN}
boundary trait Probe {{
    machine consume(handle: i64 in Classified) -> i64
    requires handle in Classified
    requires (handle as -Classified) != 0
    reaches none crashes none;
}}
machine entry() -> i64 {{ Probe::consume(issue({actual})) }}
"#
        )
    };
    let accepted = judge(invocation(&source(7))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let refused = judge(invocation(&source(0))).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{refused:?}"
    );
}

#[test]
fn membership_of_another_formal_is_not_implied_by_a_qualified_formal() {
    let refused = invocation(&format!(
        r#"
{DOMAIN}
boundary trait Probe {{
    machine consume(handle: i64 in Classified, other: i64) -> i64
    requires other in Classified
    reaches none crashes none;
}}
machine entry() -> i64 {{ Probe::consume(issue(7), 0) }}
"#
    ))
    .unwrap_err();
    assert_eq!(refused.reason, RefusalReason::Unsupported);
    assert_eq!(
        refused.detail,
        "contract domain membership while reading the body of `entry`"
    );
}

/// Typing's error for a program that breaks a language rule.
fn typing_error(text: &str) -> String {
    boundary_calls::try_typed(text).unwrap_err().message
}

/// domains.md, "Operands": a builtin comparison takes unqualified operands
/// only, the same in `requires` as in executable code. The twins differ only
/// in the cast that removes the domain.
#[test]
fn a_bare_qualified_comparison_does_not_become_a_builtin_comparison() {
    let accepted = judge(invocation(&consumer(7, "(handle as -Classified) != 0"))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    assert_eq!(
        typing_error(&consumer(7, "handle != 0")),
        "builtin operator `!=` takes unqualified operands only, but `handle` has type \
         `i64 in Classified` and no operator `!=` is declared for it; remove the domain with a \
         cast first, such as `(handle as -Classified)`"
    );
}

#[test]
fn a_bare_qualified_operand_in_executable_code_needs_its_cast() {
    let source = |left, right| {
        format!(
            r#"
{DOMAIN}
machine farther(left: i64 in Classified, right: i64 in Classified) -> bool {{
    {left} > {right}
}}
machine entry() -> bool {{ farther(issue(8), issue(7)) }}
"#
        )
    };
    let accepted = judge(invocation(&source(
        "(left as -Classified)",
        "(right as -Classified)",
    )))
    .unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    assert_eq!(
        typing_error(&source("left", "(right as -Classified)")),
        "builtin operator `>` takes unqualified operands only, but `left` has type \
         `i64 in Classified` and no operator `>` is declared for it; remove the domain with a \
         cast first, such as `(left as -Classified)`"
    );
}

#[test]
fn removing_a_domain_does_not_authorize_its_reintroduction() {
    let refused = invocation(&format!(
        r#"
{DOMAIN}
machine rebuild(handle: i64 in Classified) -> i64 in Classified {{
    handle as -Classified as +Classified
}}
machine entry() -> i64 in Classified {{ rebuild(issue(7)) }}
"#
    ))
    .unwrap_err();
    assert!(matches!(refused.reason, RefusalReason::Violation));
    assert!(
        refused
            .detail
            .contains("not one of the domain's establishment routes"),
        "{refused:?}"
    );
}

#[test]
fn incoming_value_facts_never_replace_the_declared_domain_obligation() {
    let refused = judge(invocation(
        r#"
domain i64::Positive requires self > 0;
machine entry() -> i64 in Positive { 0 as +Positive }
"#,
    ))
    .unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::ArrivalInvariant { .. }))),
        "{refused:?}"
    );
}

#[test]
fn partial_removal_does_not_make_a_qualified_clause_operand_plain() {
    let refused = typing_error(&format!(
        r#"
{DOMAIN}
domain i64::Other;
machine consume(handle: i64 in Classified & Other)
requires (handle as -Classified =Other) != 0
{{}}
machine entry(handle: i64 in Classified & Other) {{
    consume(handle);
}}
"#
    ));
    assert!(
        refused.starts_with(
            "builtin operator `!=` takes unqualified operands only, but `handle as -Classified"
        ) && refused.contains("has type `i64 in Other`")
            && refused.ends_with("as -Other)`"),
        "{refused}"
    );
}

#[test]
fn a_qualified_result_keeps_its_exact_input_equality_after_removal() {
    let source = |returned| {
        format!(
            r#"
{DOMAIN}
machine relay(handle: i64 in Classified, other: i64 in Classified) -> i64 in Classified
ensures (result as -Classified) == (handle as -Classified)
{{ {returned} }}
machine entry() -> i64 in Classified {{
    let handle: i64 in Classified = issue(7);
    let other: i64 in Classified = issue(8);
    relay(handle, other)
}}
"#
        )
    };
    let accepted = judge(invocation(&source("handle"))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let refused = judge(invocation(&source("other"))).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
        "{refused:?}"
    );
}

#[test]
fn a_qualified_record_field_keeps_its_fact_and_distinct_sibling() {
    let source = |canary| {
        format!(
            r#"
{DOMAIN}
data Packet {{ handle: i64 in Classified; canary: i64; }}
machine consume(packet: Packet) -> i64
requires (packet.handle as -Classified) != -1 && packet.canary == 13
ensures result != -1
{{ packet.handle as -Classified }}
machine entry() -> i64 {{
    let packet: Packet = Packet {{ handle: issue(7), canary: {canary} }};
    consume(packet)
}}
"#
        )
    };
    let accepted = judge(invocation(&source(13))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let refused = judge(invocation(&source(14))).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{refused:?}"
    );

    // A retained field label cannot stand in for the default-domain clause
    // which protects its contents through construction and mutation.
    let mut erased = invocation(&source(13)).unwrap();
    for owner in &mut erased.module.structural_types {
        owner.requires.clear();
    }
    let refusal = judge(Ok(erased)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert!(
        refusal
            .detail
            .contains("scalar field's default domain lost its declared predicate"),
        "{refusal:?}"
    );
}

#[test]
fn mutable_reference_removal_is_not_admitted_as_value_removal() {
    let text = format!(
        r#"
{DOMAIN}
machine clear(value: &mut i64) {{ value = 0; }}
machine borrow_remove(value: &mut i64 in Classified) {{ clear(value as -Classified); }}
machine entry() {{
    let mut handle: i64 in Classified = issue(7);
    borrow_remove(&mut handle);
}}
"#
    );
    // Typing retains this reference operation as unserved. By-value domain
    // removal supplies no proof of the lender's re-establishment obligation.
    let typed = boundary_calls::typed(&text);
    let helper = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "borrow_remove")
        .unwrap()
        .symbol;
    assert!(typed.deferred_refusals_of(helper).any(|row| {
        row.request == typed_trees::deferred_refusals::DeferredRequest::DomainCastThroughReference
    }));
    let refused = invocation(&text).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::Unsupported);
    assert_eq!(
        refused.detail,
        "declaration holding a domain cast through a reference, which no pass accounts for yet \
         while reading the signature of `borrow_remove` while reading the body of `entry`"
    );
}

#[test]
fn old_scalar_inputs_keep_their_qualification_until_an_admitted_cast() {
    let source = |guarantee, returned| {
        format!(
            r#"
{DOMAIN}
machine observe(handle: i64 in Classified) -> i64
ensures {guarantee}
{{ {returned} }}
machine entry() -> i64 {{ observe(issue(7)) }}
"#
        )
    };
    let guarantee = "result == (old(handle) as -Classified)";
    let accepted = judge(invocation(&source(guarantee, "handle as -Classified"))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    assert_eq!(
        typing_error(&source("result == old(handle)", "handle as -Classified")),
        "builtin operator `==` takes unqualified operands only, but `old(handle)` has type \
         `i64 in Classified` and no operator `==` is declared for it; remove the domain with a \
         cast first, such as `(old(handle) as -Classified)`"
    );

    let wrong_value = judge(invocation(&source(guarantee, "0"))).unwrap();
    assert!(
        matches!(wrong_value, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
        "{wrong_value:?}"
    );
}

/// A domain's predicates over an array or record formal are that formal's
/// entry clauses, exactly as `requires path[7] == 0` written on the machine
/// would be: the body may use them, and a predicate that says something else
/// proves nothing about the element it does not name.
#[test]
fn a_data_formal_in_a_predicated_domain_assumes_its_predicates() {
    let terminated = |element: u32| {
        format!(
            r#"
domain [u16; 8]::Terminated requires self[{element}] == 0;
machine entry(path: &[u16; 8] in Terminated) -> u16
ensures result == 0
{{
    path[7]
}}
"#
        )
    };
    let accepted = judge(invocation(&terminated(7))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let elsewhere = judge(invocation(&terminated(6))).unwrap();
    assert!(
        matches!(elsewhere, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
        "{elsewhere:?}"
    );

    let ordered = judge(invocation(
        r#"
data Span { low: u32; high: u32; }
domain Span::Ordered requires self.low <= self.high;
machine entry(span: Span in Ordered) -> u32
ensures result <= span.high
{
    span.low
}
"#,
    ))
    .unwrap();
    assert!(matches!(ordered, RouteVerdict::Accepted(_)), "{ordered:?}");
}

/// A declared borrowed domain contributes its predicates again at normal
/// handback, including when the incoming reference permits mutation.
#[test]
fn a_mutably_borrowed_data_formal_keeps_its_domain_at_handback() {
    let source = r#"
domain [u16; 8]::Terminated requires self[7] == 0;
machine entry(path: &mut [u16; 8] in Terminated) -> u16 { path[7] }
"#;
    let verdict = judge(invocation(source)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    let preserving = source.replace("{ path[7] }", "{ path[7] = 0; path[7] }");
    let verdict = judge(invocation(&preserving)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");

    let invalid = preserving.replace("path[7] = 0", "path[7] = 1");
    let elaboration = invocation(&invalid).unwrap();
    assert!(elaboration.tables.machines.iter().any(|(_, tables)| {
        tables
            .obligations
            .iter()
            .any(|(_, row)| matches!(row.kind, ObligationKind::DataDomain { .. }))
    }));
    let verdict = judge(Ok(elaboration)).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected {
        ref formations, ref violations, ref unproved, ref unestablished,
        ref conflicts, ref exceeded,
    } if formations.is_empty() && violations.is_empty() && conflicts.is_empty()
        && exceeded.is_empty()
        && (unproved.iter().any(|failure|
            matches!(failure.kind, ObligationKind::DataDomain { .. }))
            || !unestablished.is_empty())),
        "the changed byte must fail restoration of the declared domain: {verdict:?}"
    );
}

/// A record's own `requires` may remove a field's domain to compare it,
/// exactly as a machine's clause does (domains.md, "Operands": a qualified
/// operand "is cast first"). The cast's operand is the field, typed at the
/// type the record declares for it, and the clause is the record's
/// invariant: a reader of the field may rely on it.
#[test]
fn a_record_clause_casts_its_own_qualified_field() {
    let source = |bound: i64| {
        format!(
            r#"
domain i64::Handle requires self != -1;
data Opened
requires (root as -Handle) != {bound}
{{
    root: i64 in Handle;
}}
machine entry(opened: Opened) -> i64
ensures result != 0
{{
    opened.root as -Handle
}}
"#
        )
    };
    let accepted = judge(invocation(&source(0))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let elsewhere = judge(invocation(&source(5))).unwrap();
    assert!(
        matches!(elsewhere, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
        "{elsewhere:?}"
    );
}

/// A field that is a shared reference carries the domain of what it lends:
/// `buffer: &'s [u16; 4] in Terminated` owes `self.buffer[3] == 0`, a fact
/// about the referent, read through one `Referent` step. A shared referent
/// cannot change while it is lent, so the fact holds while the record does.
#[test]
fn a_record_clause_reads_through_a_stored_shared_reference() {
    let source = |element: u32| {
        format!(
            r#"
domain [u16; 4]::Terminated requires self[{element}] == 0;
data Named<'s> {{ buffer: &'s [u16; 4] in Terminated; }}
machine entry(named: &Named) -> u16
ensures result == 0
{{
    named.buffer[3]
}}
"#
        )
    };
    let accepted = judge(invocation(&source(3))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let elsewhere = judge(invocation(&source(2))).unwrap();
    assert!(
        matches!(elsewhere, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::NormalGuarantee { .. }))),
        "{elsewhere:?}"
    );
}

/// Building such a record establishes the domain on the lent array:
/// `&text as +Terminated` lends `text` and owes `text[3] == 0` on the edge
/// into the block the cast continues in (domains.md, "Domain-only casts":
/// "Has predicates | Prove them for this value."). The array's last element
/// decides it: 0 proves it, 5 leaves the establishment unproved.
#[test]
fn a_stored_referents_domain_is_established_by_its_proved_cast() {
    let source = |last: u16| {
        format!(
            r#"
domain [u16; 4]::Terminated requires self[3] == 0;
data Named<'s> {{ buffer: &'s [u16; 4] in Terminated; }}
machine last(named: &Named) -> u16 ensures result == 0 {{ named.buffer[3] }}
machine entry() -> u16 {{
    let text: [u16; 4] = [7, 8, 9, {last}];
    let named: Named = Named {{ buffer: &text as +Terminated }};
    last(&named)
}}
"#
        )
    };
    let accepted = judge(invocation(&source(0))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let unproved = judge(invocation(&source(5))).unwrap();
    assert!(
        matches!(unproved, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::ArrivalInvariant { .. }))),
        "{unproved:?}"
    );
}

/// The same cast on the array itself: `text as +Terminated` owes
/// `text[3] == 0` on the edge into the block the cast continues in, and the
/// formal in the domain then knows it.
#[test]
fn an_arrays_domain_is_established_by_its_proved_cast() {
    let source = |last: u16| {
        format!(
            r#"
domain [u16; 4]::Terminated requires self[3] == 0;
machine last(text: [u16; 4] in Terminated) -> u16 ensures result == 0 {{
    let plain: [u16; 4] = text as -Terminated;
    plain[3]
}}
machine entry() -> u16 {{
    let text: [u16; 4] = [7, 8, 9, {last}];
    last(text as +Terminated)
}}
"#
        )
    };
    let accepted = judge(invocation(&source(0))).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let unproved = judge(invocation(&source(5))).unwrap();
    assert!(
        matches!(unproved, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::ArrivalInvariant { .. }))),
        "{unproved:?}"
    );
}
