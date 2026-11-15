//! The actual by-value consumer owes its authored contract at disposal.
use super::*;
use semantic_vocabulary::InvocationSite;
use terminal_psi::TerminalAffineCleanupAction;

fn program(valid: u64, receiver: &str, body: &str, effects: &str) -> String {
    format!(
        r#"
data Counter {{ value: u64; }}
data Resource<'view> dropped by dispose {{ dropped: &'view mut Counter; valid: u64; value: u64; }}
machine dispose(resource: {receiver})
requires resource.valid == 1u64
requires resource.value <= 30u64
{effects}
{{ {body} }}
machine caller(counter: &mut Counter) {{
    let resource: Resource = Resource {{ dropped: counter, valid: {valid}u64, value: 30u64 }};
}}
"#
    )
}

const DECOMPOSE: &str =
    "let { dropped, valid, value as _ } = move resource; dropped.value = valid;";

fn accepted() -> terminal_check_tables::program::Elaboration {
    let text = program(1, "Resource", DECOMPOSE, "");
    match super::entryless::judged(&text, false) {
        RouteVerdict::Accepted(accepted) => accepted.elaboration,
        other => panic!("actual owned consumer must be admitted: {other:?}"),
    }
}

#[test]
fn owned_cleanup_requires_are_proved_at_the_exact_action() {
    let elaboration = accepted();
    let mut found = 0;
    for (id, tables) in elaboration.tables.machines.iter() {
        for (invocation, call) in tables.invocations.iter() {
            if let InvocationSite::Cleanup {
                edge,
                action_ordinal,
            } = call.site
            {
                found += 1;
                assert_eq!(call.requirements.len(), 2);
                for (clause, obligation) in call.requirements.iter().enumerate() {
                    let row = tables.obligations.get(*obligation).unwrap();
                    assert_eq!(
                        row.created_at,
                        ProgramPoint::BeforeCleanup {
                            edge,
                            action_ordinal
                        }
                    );
                    assert_eq!(
                        row.kind,
                        ObligationKind::CallRequirement {
                            invocation,
                            clause: clause as u32
                        }
                    );
                }
                assert!(
                    elaboration
                        .module
                        .machine(id)
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| block.terminator.cleanup_rosters())
                        .any(|(selected, actions)| selected == edge
                            && matches!(
                                actions.get(action_ordinal as usize),
                                Some(TerminalAffineCleanupAction::InvokeOwned(_))
                            ))
                );
            }
        }
    }
    assert_eq!(found, 1);
    let negative = super::entryless::judged(&program(0, "Resource", DECOMPOSE, ""), false);
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref unproved, .. } if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { clause: 0, .. }))),
        "{negative:?}"
    );
}

#[test]
fn receiving_rejects_missing_reordered_or_relocated_cleanup_requirements() {
    let accepted = accepted();
    let (machine, invocation, call) = accepted
        .tables
        .machines
        .iter()
        .find_map(|(machine, tables)| {
            tables
                .invocations
                .iter()
                .find(|(_, call)| matches!(call.site, InvocationSite::Cleanup { .. }))
                .map(|(invocation, call)| (machine, invocation, call.clone()))
        })
        .unwrap();
    for mutation in 0..3 {
        let mut changed = accepted.clone();
        let tables = changed.tables.machines.get_mut(machine).unwrap();
        match mutation {
            0 => {
                tables
                    .invocations
                    .get_mut(invocation)
                    .unwrap()
                    .requirements
                    .pop();
            }
            1 => {
                tables
                    .invocations
                    .get_mut(invocation)
                    .unwrap()
                    .requirements
                    .swap(0, 1);
            }
            _ => {
                tables
                    .obligations
                    .get_mut(call.requirements[0])
                    .unwrap()
                    .created_at = match call.site {
                    InvocationSite::Cleanup { edge, .. } => ProgramPoint::Edge(edge),
                    _ => unreachable!(),
                };
            }
        }
        let refused = crate::discharge(&mut changed)
            .expect_err("receiving must not repair generated call proofs");
        assert_eq!(
            refused.reason,
            RefusalReason::ForeignIdentity,
            "{refused:?}"
        );
    }
}

#[test]
fn undecomposed_recursive_or_abnormal_consumers_never_gain_a_disposition() {
    for (receiver, body, effects, expected) in [
        ("Resource", "", "", "recursive generated cleanup"),
        ("&mut Resource", "", "", "incompatible value parameter"),
        (
            "Resource",
            "let { dropped, valid, value as _ } = move resource; dropped.value = valid; crash Trap;",
            "crashes Trap",
            "abnormal completion",
        ),
    ] {
        let typed = super::boundary_calls::typed(&program(1, receiver, body, effects));
        let result = super::entryless::judged_typed_result(&typed, false);
        let refusal = match result {
            Err(crate::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal,
            other => panic!("{receiver}: expected {expected}, got {other:?}"),
        };
        assert!(refusal.detail.contains(expected), "{receiver}: {refusal:?}");
        assert!(
            matches!(
                refusal.reason,
                RefusalReason::Unsupported | RefusalReason::ForeignIdentity
            ),
            "{receiver}: {refusal:?}"
        );
    }
}
