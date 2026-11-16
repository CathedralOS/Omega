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
machine caller(counter: &mut Counter) -> u64
ensures result <= 50u64
{{
    let resource: Resource = Resource {{ dropped: counter, valid: {valid}u64, value: 30u64 }};
    resource.value
}}
"#
    )
}

const DECOMPOSE: &str =
    "let { dropped, valid, value as _ } = move resource; dropped.value = valid;";

fn prepared(text: &str) -> typed_trees::FrozenTypedTrees {
    typed_trees_to_typed_trees::prepare_typed_trees(
        super::boundary_calls::typed(text),
        &typed_trees_to_typed_trees::PreparationRequest::settled(),
    )
    .expect("ordinary typed preparation must preserve this source program")
    .0
}

fn judged(text: &str) -> Result<RouteVerdict, crate::CompilationRefusal> {
    super::entryless::judged_typed_result(&prepared(text), false)
}

fn accepted() -> terminal_check_tables::program::Elaboration {
    let text = program(1, "Resource", DECOMPOSE, "");
    match judged(&text).expect("owned cleanup source must finish preparation") {
        RouteVerdict::Accepted(accepted) => accepted.elaboration,
        other => panic!("actual owned consumer must be admitted: {other:?}"),
    }
}

#[test]
fn owned_cleanup_requires_are_proved_at_the_exact_action() {
    let elaboration = accepted();
    for machine in &elaboration.module.machines {
        for (_, actions) in machine
            .blocks
            .iter()
            .flat_map(|block| block.terminator.cleanup_rosters())
        {
            for action in actions {
                if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                    assert!(
                        cleanup.subject.path.is_empty(),
                        "the whole Resource must reach its consumer"
                    );
                    assert!(
                        !machine.operations().any(|operation| matches!(
                            &operation.kind,
                            terminal_psi::OperationKind::ReleaseReference { source, path }
                                if *source == cleanup.subject.place && !path.is_empty()
                        )),
                        "an intact nominal wrapper must keep its reference components"
                    );
                }
            }
        }
    }
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
    let negative = judged(&program(0, "Resource", DECOMPOSE, ""))
        .expect("the invalid value must reach its actual call requirement");
    assert!(
        matches!(negative, RouteVerdict::Rejected { ref unproved, .. } if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { clause: 0, .. }))),
        "{negative:?}"
    );
}

#[test]
fn ordinary_inferred_bindings_keep_the_parent_values_reference_region() {
    let text = r#"
data Counter { value: u64; }
data Held<'view> { target: &'view mut Counter; value: u64; }
machine consume(held: Held) {
    let { target, value } = move held;
    target.value = value;
}
machine caller(counter: &mut Counter) {
    let held: Held = Held { target: counter, value: 1u64 };
    consume(held);
}
"#;
    let result = judged(text).expect("ordinary inferred values must reach the receiving checker");
    assert!(matches!(result, RouteVerdict::Accepted(_)), "{result:?}");
}

#[test]
fn receiving_rejects_missing_reordered_or_relocated_cleanup_requirements() {
    // Use fresh producer input, not a prior discharge's derived knowledge.
    // Generated call rows are formed once before any untrusted mutation.
    let typed = prepared(&program(1, "Resource", DECOMPOSE, ""));
    let mut unjudged = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[],
            subjects: &[None],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
    .expect("the actual source produces fresh receiving input");
    crate::owned_cleanup::materialize(&mut unjudged).unwrap();
    let mut baseline = unjudged.clone();
    let mut activity = crate::ProofSearchActivity::default();
    let verified = crate::discharge(&mut baseline, &mut activity)
        .expect("fresh untampered input must discharge");
    for machine in verified.machines.values() {
        assert!(machine.verdicts.values().all(Result::is_ok), "{verified:?}");
        assert!(
            machine.uses.iter().all(|used| used.verdict.is_ok()),
            "{verified:?}"
        );
        assert!(
            machine.windows.iter().all(|window| window.verdict.is_ok()),
            "{verified:?}"
        );
        assert!(machine.conflicts.is_empty(), "{verified:?}");
    }
    let (machine, invocation, call) = unjudged
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
        let mut changed = unjudged.clone();
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
        let mut activity = crate::ProofSearchActivity::default();
        let refused = crate::discharge(&mut changed, &mut activity)
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
        let result = judged(&program(1, receiver, body, effects));
        let refusal = match result {
            Err(crate::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal,
            other => panic!("{receiver}: expected {expected}, got {other:?}"),
        };
        assert!(
            refusal.detail.contains(expected),
            "{receiver}: expected {expected}, got {refusal:?}"
        );
        assert!(
            matches!(
                refusal.reason,
                RefusalReason::Unsupported | RefusalReason::ForeignIdentity
            ),
            "{receiver}: {refusal:?}"
        );
    }
}
