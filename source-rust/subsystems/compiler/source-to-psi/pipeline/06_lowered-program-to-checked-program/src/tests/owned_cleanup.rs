//! The actual by-value consumer owes its authored contract at disposal.
use super::*;
use semantic_vocabulary::InvocationSite;
use terminal_psi::TerminalAffineCleanupAction;

fn program(valid: u64, receiver: &str, body: &str, effects: &str) -> String {
    format!(
        r#"
data Counter {{ value: u64; }}
data Resource<'view> dropped by dispose {{ dropped: @'view mutable Counter; valid: u64; value: u64; }}
machine dispose(resource: {receiver})
requires resource.valid == 1u64
requires resource.value <= 30u64
{effects}
{{ {body} }}
machine caller(counter: @mutable Counter) -> u64
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

fn prepared(text: &str) -> crate::preparation::PreparedTypedTrees {
    crate::preparation::prepare_typed_trees(super::boundary_calls::typed(text), &[], &[])
        .expect("ordinary typed preparation must preserve this source program")
}

fn judged(text: &str) -> Result<RouteVerdict, crate::CompilationRefusal> {
    super::entryless::judged_typed_result(prepared(text).trees(), false)
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
data Held<'view> { target: @'view mutable Counter; value: u64; }
machine consume(held: Held) {
    let { target, value } = move held;
    target.value = value;
}
machine caller(counter: @mutable Counter) {
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
    let mut unjudged = crate::lowering::elaborate_for(
        typed.trees(),
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[],
            subjects: &[None],
            purpose: source_provenance::DependencyScope::Product,
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
        ("@mutable Resource", "", "", "incompatible value parameter"),
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

#[test]
fn complete_patterns_end_linear_wrappers_without_forgetting_components() {
    let accepted = [
        include_str!(
            "../../tests/fixtures/ownership/run/consuming_record_pattern_cleanup/main.omg"
        ),
        "data Zero [linear] {} machine consume(value: Zero) { let {} = move value; }",
        "data Ticket [linear] { number: u64; } machine consume(value: Ticket) -> u64 { let { number } = move value; number }",
        "data Zero [linear] {} data Pair [linear] { left: Zero; right: Zero; } machine consume(value: Pair) { let { left, right } = move value; let {} = move left; let {} = move right; }",
    ];
    for source in accepted {
        let result = judged(source).expect("complete consuming patterns must reach the checker");
        assert!(
            matches!(result, RouteVerdict::Accepted(_)),
            "{source}: {result:?}"
        );
    }
    let refused = [
        (
            include_str!(
                "../../tests/fixtures/ownership/fail/consuming_record_pattern_reuse/main.omg"
            ),
            "decomposition source is not a complete live value",
        ),
        (
            "data Zero [linear] {} machine consume(value: Zero) { let {} = move value; let {} = move value; }",
            "decomposition source is not a complete live value",
        ),
        (
            "data Ticket [linear] { number: u64; } machine consume(value: Ticket) { let {} = move value; }",
            "record pattern that omits a field",
        ),
        (
            "data Zero [linear] {} data Pair [linear] { left: Zero; right: Zero; } machine consume(value: Pair) { let { left, right } = move value; let { left as again, right as also } = move value; let {} = move left; let {} = move right; let {} = move again; let {} = move also; }",
            "decomposition source is not a complete live value",
        ),
        (
            "data Zero [linear] {} data Pair [linear] { left: Zero; right: Zero; } machine consume(value: Pair) { let { left, right } = move value; let {} = move left; }",
            "a linear value is still held when this path leaves its storage",
        ),
    ];
    for (source, expected) in refused {
        // A linear value left unconsumed is the program's error: the
        // compiler rejects it rather than refusing it as unsupported.
        let detail = match judged(source) {
            Err(crate::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
            Ok(RouteVerdict::Rejected { violations, .. }) if violations.len() == 1 => {
                violations[0].statement.clone()
            }
            other => panic!("expected ownership refusal {expected}, got {other:?}: {source}"),
        };
        assert!(
            detail.contains(expected),
            "expected {expected}, got {detail}: {source}"
        );
    }
}

#[test]
fn linear_cleanup_transfers_the_receipt_claim_exactly_once() {
    let source = "data Ticket [linear] dropped by finish { number: u64; }
        machine finish(ticket: Ticket) { let { number } = move ticket; }
        machine build_entry() { let ticket: Ticket = Ticket { number: 7u64 }; }";
    let result = super::build_purpose::judge_build(source)
        .expect("a linear cleanup should reach the checker");
    let RouteVerdict::Accepted(accepted) = result else {
        panic!("{result:?}")
    };
    let module = &accepted.elaboration.module;
    let (machine, edge, ordinal, transfers) = module
        .machines
        .iter()
        .find_map(|machine| {
            machine
                .module_invocations()
                .find_map(|call| match call.site {
                    InvocationSite::Cleanup {
                        edge,
                        action_ordinal,
                    } => Some((
                        machine.id,
                        edge,
                        action_ordinal as usize,
                        call.inputs.claim_transfers,
                    )),
                    _ => None,
                })
        })
        .expect("the receipt must reach its selected consumer");
    assert_eq!(transfers.len(), 1);
    assert_eq!(transfers[0].argument_index, 0);
    assert!(transfers[0].path.is_empty());
    for duplicate in [false, true] {
        let mut forged = module.clone();
        let actions = forged
            .machines
            .iter_mut()
            .find(|row| row.id == machine)
            .unwrap()
            .blocks
            .iter_mut()
            .flat_map(|block| block.terminator.cleanup_rosters_mut())
            .find(|(id, _)| *id == edge)
            .unwrap()
            .1;
        let TerminalAffineCleanupAction::InvokeOwned(action) = &mut actions[ordinal] else {
            unreachable!()
        };
        if duplicate {
            action
                .claim_transfers
                .push(action.claim_transfers[0].clone());
        } else {
            action.claim_transfers.clear();
        }
        let error = terminal_semantics::plain_affine::verify_claims_in(&forged, machine)
            .expect_err("each incoming claim must reach the cleanup exactly once");
        assert!(
            format!("{error:?}").contains("cleanup transfers differ"),
            "{error:?}"
        );
    }
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    assert!(matches!(
        instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    ));
    let no_consumer = source.replace(" dropped by finish", "");
    assert!(matches!(judged(&no_consumer),
        Ok(RouteVerdict::Rejected { ref violations, .. })
            if violations.len() == 1
                && violations[0].statement.contains("a linear value is still held")));
}

#[test]
fn unmatched_sum_disposes_only_active_case_and_rejects_unproved_or_linear_cases() {
    let source = |lease_header: &str, release_requires: &str, choose_a: bool| {
        let construct = if choose_a {
            "Choice::A { left: Token { id: 1u64 }, right: Lease { counter: @mutable counter, valid: 1u64, value: 7u64 } }"
        } else {
            "Choice::B { left: Token { id: 1u64 }, right: Token { id: 2u64 } }"
        };
        format!(
            r#"
data Counter {{ value: u64; }}
data Token {{ id: u64; }}
data Lease<'view> {lease_header} {{ counter: @'view mutable Counter; valid: u64; value: u64; }}
data Choice<'view> {{
    case A(left: Token, right: Lease<'view>);
    case B(left: Token, right: Token);
}}
machine release(lease: Lease)
{release_requires}
{{
    let {{ counter, valid as _, value }} = move lease;
    counter.value = value;
}}
machine discard(x: Choice) {{}}
machine build_entry() -> u64 {{
    let mutable counter: Counter = Counter {{ value: 10u64 }};
    let choice: Choice = {construct};
    discard(choice);
    counter.value
}}
"#
        )
    };

    for (choose_a, expected) in [(true, 7u64), (false, 10u64)] {
        let text = source("dropped by release", "", choose_a);
        let result = super::build_purpose::judge_build(&text)
            .expect("unmatched sum disposal must reach the checker");
        let RouteVerdict::Accepted(accepted) = result else {
            panic!("unmatched sum disposal must be admitted: {result:?}");
        };
        let discard_machine = accepted
            .elaboration
            .module
            .machines
            .iter()
            .find(|machine| {
                accepted
                    .elaboration
                    .names
                    .get(machine.id)
                    .is_some_and(|name| name.machine == "discard")
            })
            .expect("discard machine must exist");
        let mut invoked_cases = 0usize;
        let mut plain_cases = 0usize;
        for block in &discard_machine.blocks {
            if let terminal_psi::Terminator::StructuralCase { cases, .. } = &block.terminator {
                for arm in cases {
                    let invokes = arm
                        .cleanup_actions
                        .iter()
                        .filter(|action| {
                            matches!(action, TerminalAffineCleanupAction::InvokeOwned(_))
                        })
                        .count();
                    let discards = arm
                        .cleanup_actions
                        .iter()
                        .filter(|action| {
                            matches!(action, TerminalAffineCleanupAction::DiscardResidual(_))
                        })
                        .count();
                    let retires = arm
                        .cleanup_actions
                        .iter()
                        .filter(|action| {
                            matches!(action, TerminalAffineCleanupAction::RetireHome(_))
                        })
                        .count();
                    assert_eq!(retires, 1, "each case arm must retire the sum home once");
                    if invokes == 1 && discards == 1 {
                        invoked_cases += 1;
                    } else if invokes == 0 && discards == 2 {
                        plain_cases += 1;
                    } else {
                        panic!(
                            "unexpected cleanup roster on arm: {:?}",
                            arm.cleanup_actions
                        );
                    }
                }
            }
        }
        assert_eq!(
            (invoked_cases, plain_cases),
            (1, 1),
            "case A must invoke release on right and discard left; case B must discard both"
        );

        let publication = accepted.publish_invocation().unwrap();
        let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
        let mut instance = program.instantiate(Default::default()).unwrap();
        let status = instance
            .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
            .unwrap();
        assert!(
            matches!(
                status,
                terminal_interpreter::TerminalExecutionStatus::Complete(
                    terminal_interpreter::TerminalExecutionResult::Scalar(
                        terminal_interpreter::TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(value),
                            ..
                        }
                    )
                ) if value == u128::from(expected)
            ),
            "Psi execution must agree for choose_a={choose_a}: {status:?}"
        );
    }

    // Negative twin 1: case A's cleanup machine has a precondition that `discard`
    // cannot prove on an arbitrary `Choice`.
    let unproved_requires = judged(&source(
        "dropped by release",
        "requires lease.valid == 1u64",
        true,
    ))
    .expect("unproved cleanup precondition must reach the checker");
    assert!(
        matches!(
            unproved_requires,
            RouteVerdict::Rejected { ref unproved, .. }
                if unproved
                    .iter()
                    .any(|row| matches!(row.kind, ObligationKind::CallRequirement { clause: 0, .. }))
        ),
        "{unproved_requires:?}"
    );

    // Negative twin 2: case A holds a linear payload with no declared cleanup.
    let linear_without_drop = judged(&source("[linear]", "", true));
    let detail = match linear_without_drop {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Rejected { violations, .. }) if violations.len() == 1 => {
            violations[0].statement.clone()
        }
        other => panic!("expected linear refusal, got {other:?}"),
    };
    assert!(
        detail.contains("a linear value is still held"),
        "expected unconsumed linear refusal, got {detail}"
    );
}
