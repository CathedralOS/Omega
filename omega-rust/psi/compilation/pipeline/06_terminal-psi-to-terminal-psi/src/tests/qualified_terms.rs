//! Removing a qualification keeps the value and its facts, without granting
//! permission to issue that qualification again.

use super::*;

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
fn a_bare_qualified_comparison_does_not_become_a_builtin_comparison() {
    let refused = invocation(&consumer(7, "handle != 0")).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::Unsupported);
    assert_eq!(
        refused.detail,
        "contract builtin operator on a value with a declared domain in a clause"
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
    assert!(matches!(refused.reason, RefusalReason::Violation(_)));
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
    let refused = invocation(&format!(
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
    ))
    .unwrap_err();
    assert_eq!(refused.reason, RefusalReason::Unsupported);
    assert_eq!(
        refused.detail,
        "contract builtin operator on a value with a declared domain in a clause"
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
        "declaration holding a domain cast through a reference, which no pass accounts for yet"
    );
}
