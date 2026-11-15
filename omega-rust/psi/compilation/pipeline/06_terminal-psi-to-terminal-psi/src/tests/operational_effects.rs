//! Blocking is an interface promise and an independently acknowledged call.

use super::*;

const BLOCKING: &str = r#"
public machine may_wait() -> u32 blocks { 7 }
public machine wrapped() -> u32 blocks { block may_wait() }
machine main() crashes Trap {
    let answer: u32 = block wrapped();
    transition answer == 7 {
        true -> done()
        _ -> bad()
    }
    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#;

fn invocation(text: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    elaborate(
        boundary_calls::typed(text),
        terminal_codec::TerminalPublication::Invocation,
        &[],
    )
}

fn elaborate(
    typed: typed_trees::TypedTrees,
    publication: terminal_codec::TerminalPublication,
    subjects: &[Option<semantic_vocabulary::PackageKeyIdentity>],
) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap()
        .symbol;
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    typed_trees_to_terminal_psi::elaborate_for(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects,
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        publication,
    )
}

#[test]
fn an_acknowledged_blocking_call_preserves_its_returned_value() {
    let verdict = judge(invocation(BLOCKING)).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

fn accepted(text: &str) -> Box<crate::AcceptedClosure> {
    match judge(invocation(text)).unwrap() {
        RouteVerdict::Accepted(accepted) => accepted,
        other => panic!("expected accepted: {other:?}"),
    }
}

fn violation(text: &str, expected: &str) {
    match judge(invocation(text)).unwrap() {
        RouteVerdict::Rejected { violations, .. } => assert!(
            violations
                .iter()
                .any(|violation| violation.message().contains(expected)),
            "{violations:?}"
        ),
        other => panic!("expected {expected}: {other:?}"),
    }
}

#[test]
fn permission_and_acknowledgement_are_independent() {
    violation(
        &BLOCKING.replace(
            "public machine wrapped() -> u32 blocks",
            "public machine wrapped() -> u32",
        ),
        "blocks ceiling is false",
    );
    violation(
        &BLOCKING.replace("{ block may_wait() }", "{ may_wait() }"),
        "requires a block acknowledgement",
    );
    violation(
        &BLOCKING.replace("{ block may_wait() }", "{ suspend may_wait() }"),
        "requires a block acknowledgement",
    );
    violation(
        &BLOCKING.replace(
            "machine main() crashes Trap",
            "machine main() blocks false crashes Trap",
        ),
        "blocks ceiling is false",
    );
}

#[test]
fn private_omission_infers_the_envelope() {
    let text = BLOCKING.replace(
        "public machine wrapped() -> u32 blocks",
        "machine wrapped() -> u32",
    );
    let accepted = accepted(&text);
    let root = &accepted.elaboration.module.machines[0];
    assert_eq!(root.declared_blocking, None);
    assert!(root.published_may_block);
    assert!(!root.published_may_suspend);
}

#[test]
fn redundant_markers_keep_the_result_and_create_no_crossing() {
    let text = BLOCKING.replace("block may_wait()", "suspend block may_wait()");
    let accepted = accepted(&text);
    assert!(
        accepted
            .elaboration
            .module
            .machines
            .iter()
            .all(|machine| !machine.published_may_suspend)
    );
    assert!(
        accepted
            .elaboration
            .module
            .operations()
            .all(|operation| operation.suspension_crossing.is_none())
    );
}

/// Two calls that may suspend, each an operation of its caller: `wrapped`
/// waits and returns what it got, and `main` keeps `kept` across its own
/// wait to compare afterwards.
const SUSPENDING: &str = r#"
public machine may_wait() -> u32 suspends { 7 }
public machine wrapped() -> u32 suspends {
    let waited: u32 = suspend may_wait();
    waited
}
machine main(kept: u32) crashes Trap {
    let answer: u32 = suspend wrapped();
    transition answer == kept {
        true -> done()
        _ -> bad()
    }
    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#;

/// calls_and_outcomes.md, "Suspension": "A suspension site binds the exact
/// call, checked crossing, callee, and commitment to the complete ordered
/// live frontier." Each call that may suspend is an operation with a
/// crossing, and the module states one site and one plan for it. What is
/// live across `main`'s wait is `kept`, which it compares afterwards; the
/// call's own result is not, since "the operation is incomplete and its
/// result is not established" while it is parked.
#[test]
fn a_call_that_may_suspend_is_a_crossing_with_its_live_frontier() {
    let accepted = accepted(SUSPENDING);
    let module = &accepted.elaboration.module;
    let crossings: Vec<_> = module
        .operations()
        .filter_map(|operation| operation.suspension_crossing)
        .collect();
    assert_eq!(crossings.len(), 2);
    assert_eq!(module.suspension_call_sites.len(), 2);
    assert_eq!(module.suspension_call_plans.len(), 2);
    assert_eq!(module.suspension_call_plan_count, 2);
    let frontiers: Vec<usize> = module
        .suspension_call_plans
        .iter()
        .map(|plan| plan.live_values.len())
        .collect();
    assert!(frontiers.contains(&0), "{frontiers:?}");
    assert!(frontiers.contains(&1), "{frontiers:?}");
    let kept = module
        .suspension_call_plans
        .iter()
        .flat_map(|plan| &plan.live_values)
        .next()
        .unwrap();
    assert!(matches!(
        kept.place,
        terminal_psi::TerminalSuspensionPlace::Scalar(_)
    ));
    assert_eq!(
        kept.storage,
        terminal_psi::TerminalSuspensionStorage::Parameter
    );
    // The marker is still owed where the envelope needs it.
    violation(
        &SUSPENDING.replace("= suspend may_wait();", "= block may_wait();"),
        "requires a suspend acknowledgement",
    );
}

/// A crossing is the checker's to reconstruct, not the producer's to
/// assert: "Matching a producer's site and plan is not enough: deleting both
/// must not erase a required crossing", and "Missing, duplicate, redirected,
/// or understated rows reject."
#[test]
fn a_missing_or_altered_crossing_is_not_accepted() {
    let refusal = |alter: &dyn Fn(&mut typed_trees_to_terminal_psi::Elaboration)| {
        let mut elaboration = invocation(SUSPENDING).unwrap();
        alter(&mut elaboration);
        let refused = crate::operational::judge(&elaboration).unwrap_err();
        assert_eq!(
            refused.reason,
            RefusalReason::ForeignIdentity,
            "{refused:?}"
        );
        refused.detail
    };
    // The site, the plan and the operation's crossing are all deleted.
    assert_eq!(
        refusal(&|elaboration| {
            let module = &mut elaboration.module;
            module.suspension_call_sites.clear();
            module.suspension_call_plans.clear();
            module.suspension_call_plan_count = 0;
            for machine in &mut module.machines {
                for block in &mut machine.blocks {
                    for operation in &mut block.operations {
                        operation.suspension_crossing = None;
                    }
                }
            }
        }),
        "a possibly suspending call carries no suspension crossing"
    );
    // A live value is left out of a plan.
    assert_eq!(
        refusal(&|elaboration| {
            for plan in &mut elaboration.module.suspension_call_plans {
                plan.live_values.clear();
                plan.live_value_count = 0;
            }
        }),
        "a suspension plan is not the live frontier of its call"
    );
    // A site commits to another frontier.
    assert_eq!(
        refusal(&|elaboration| {
            elaboration.module.suspension_call_sites[0].frontier_commitment = [0; 32];
        }),
        "a suspension site does not commit to the live frontier of its call"
    );
    // An operation that cannot suspend claims a crossing.
    assert_eq!(
        refusal(&|elaboration| {
            let crossing = elaboration.module.suspension_call_plans[0].crossing;
            let operation = elaboration
                .module
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.blocks)
                .flat_map(|block| &mut block.operations)
                .find(|operation| operation.suspension_crossing.is_none())
                .unwrap();
            operation.suspension_crossing = Some(crossing);
        }),
        "a suspension crossing is on an operation that cannot suspend"
    );
}

/// A call a machine ends with is lowered to its terminator, and a crossing
/// is a property of an operation, so a possibly suspending terminal call is
/// still refused by name.
#[test]
fn a_terminal_call_that_may_suspend_is_refused_by_name() {
    let text = BLOCKING
        .replace("blocks", "suspends")
        .replace("block ", "suspend ");
    match judge(invocation(&text)).unwrap() {
        RouteVerdict::FallBack(refused) => assert_eq!(
            refused.detail,
            "a possibly suspending terminal call needs a checked suspension crossing"
        ),
        RouteVerdict::Accepted(accepted) => {
            // The route lowered the final call as an operation: then it is
            // a crossing like any other.
            assert!(
                accepted
                    .elaboration
                    .module
                    .operations()
                    .any(|operation| operation.suspension_crossing.is_some())
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_nested_suspend_marker_does_not_gain_a_position_from_argument_lifting() {
    violation(
        r#"
        machine identity(value: u32) -> u32 { value }
        machine main() -> u32 { identity(suspend identity(7)) }
    "#,
        "suspend-marked call",
    );
    accepted(
        r#"
        machine identity(value: u32) -> u32 { value }
        machine main() -> u32 { identity(block identity(7)) }
    "#,
    );
}

const LOOP: &str = r#"
public machine again(value: bool) blocks {
    transition value {
        true -> block again(false)
        _ -> done()
    }
    machine done() {}
}
machine main() { block again(true); }
"#;

#[test]
fn frame_reuse_keeps_the_called_interface() {
    let accepted = accepted(LOOP);
    assert!(accepted.elaboration.module.machines.iter().any(|machine| {
        machine
            .call_formations
            .iter()
            .any(|row| row.tail_callee.is_some())
    }));
    violation(
        &LOOP.replace("true -> block again(false)", "true -> again(false)"),
        "requires a block acknowledgement",
    );
}

#[test]
fn malformed_or_forged_operational_metadata_is_not_a_program_rejection() {
    let mut elaboration = invocation(BLOCKING).unwrap();
    let machine = &mut elaboration.module.machines[0];
    machine.call_formations.push(machine.call_formations[0]);
    let refused = crate::operational::judge(&elaboration).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
    assert!(refused.detail.contains("strictly ordered"));
    let mut elaboration = invocation(BLOCKING).unwrap();
    elaboration.module.machines[0].published_may_block = false;
    assert!(
        crate::operational::judge(&elaboration)
            .unwrap_err()
            .detail
            .contains("published operational contract")
    );
    let mut elaboration = invocation(BLOCKING).unwrap();
    let row = &mut elaboration.module.machines[0].call_formations;
    row.truncate(1);
    row[0].site = terminal_psi::CallSite::Operation(id(999));
    assert!(
        crate::operational::judge(&elaboration)
            .unwrap_err()
            .detail
            .contains("no canonical call")
    );
}

#[test]
fn the_kept_runtime_witness_checks_before_native_realization() {
    accepted(include_str!(
        "../../../../../../../tests/fixtures/run/calls/operational_contracts/main.omg"
    ));
}

#[test]
fn a_boundary_uses_its_published_envelope_even_without_a_waiting_body() {
    let source = r#"
        boundary trait Probe { machine wait() -> u32 reaches none blocks crashes none; }
        machine main() -> u32 { block Probe::wait() }
    "#;
    accepted(source);
    violation(
        &source.replace("block Probe", "Probe"),
        "requires a block acknowledgement",
    );
    violation(
        &source.replace("machine main() -> u32", "public machine main() -> u32"),
        "blocks ceiling is false",
    );
}

#[test]
fn an_implementation_keeps_its_own_bound_and_owes_the_requirement_bound() {
    fn witness(
        requirement: &str,
        supplied: &str,
        body: &str,
    ) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
        let text = format!(
            r#"
            trait Probe {{ machine wait() -> u32 reaches none {requirement}; }}
            public machine helper() -> u32 blocks {{ 7 }}
            machine implementation() -> u32 satisfies Probe::wait {supplied} {{ {body} }}
        "#
        );
        let typed = boundary_calls::typed(&text);
        let implementation = typed
            .machines()
            .iter()
            .find(|row| row.name.as_str() == "implementation")
            .unwrap()
            .symbol;
        let package = typed
            .symbols
            .symbol_product_package_identity(implementation)
            .expect("the checked fixture has its actual package identity");
        // The implementation body and its refinement are separate obligations.
        // Check both in the same compilation instead of a bodyless logical form.
        crate::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &typed,
                typed_trees_to_terminal_psi::Compilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[],
                    subjects: &[Some(package)],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .program,
        )
    }
    let allowed = witness("blocks", "", "7");
    assert!(
        !allowed.is_empty()
            && allowed
                .iter()
                .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{allowed:?}"
    );
    let check_form = allowed
        .iter()
        .filter_map(|verdict| match verdict {
            Ok(RouteVerdict::Accepted(accepted)) => Some(accepted),
            _ => None,
        })
        .find(|accepted| !accepted.elaboration.check_form_envelopes.is_empty())
        .expect("the conformance proof retains its generated checking frame");
    assert!(matches!(
        check_form.publish(),
        Err(crate::PublicationError::GenericCheckForm)
    ));
    assert!(matches!(
        check_form.publish_invocation(),
        Err(crate::PublicationError::GenericCheckForm)
    ));
    assert!(crate::ceilings::judge(&check_form.elaboration).is_ok());
    let mut duplicate = check_form.elaboration.clone();
    let frame = duplicate.check_form_envelopes[0].clone();
    duplicate.check_form_envelopes.insert(0, frame);
    let refused = crate::ceilings::judge(&duplicate).unwrap_err();
    assert_eq!(
        refused.reason,
        RefusalReason::ForeignIdentity,
        "{refused:?}"
    );
    for refused in [
        witness("", "blocks", "7"),
        witness("blocks", "blocks false", "block helper()"),
    ] {
        assert!(
            refused
                .iter()
                .any(|verdict| matches!(verdict, Ok(RouteVerdict::Rejected { .. }))),
            "{refused:?}"
        );
        assert!(
            refused.iter().all(|verdict| match verdict {
                Ok(RouteVerdict::Accepted(_)) => true,
                Ok(RouteVerdict::Rejected {
                    formations,
                    violations,
                    unproved,
                    unestablished,
                    conflicts,
                    exceeded,
                }) => {
                    !violations.is_empty()
                        && violations.iter().all(|violation| {
                            violation.message().contains("blocks ceiling is false")
                        })
                        && formations.is_empty()
                        && unproved.is_empty()
                        && unestablished.is_empty()
                        && conflicts.is_empty()
                        && exceeded.is_empty()
                }
                _ => false,
            }),
            "{refused:?}"
        );
    }
}

#[test]
fn a_call_with_no_normal_return_keeps_its_acknowledgement() {
    let text = LOOP.replace("_ -> done()", "_ -> block again(false)");
    // This test uses a body-derived outcome, so declare the anonymous source
    // unit explicitly. A closure-only request supplies no local summary unit.
    let local = |text: &str| {
        judge(elaborate(
            boundary_calls::typed(text),
            terminal_codec::TerminalPublication::Invocation,
            &[None],
        ))
        .unwrap()
    };
    let RouteVerdict::Accepted(accepted) = local(&text) else {
        panic!("the acknowledged local call must be accepted")
    };
    assert!(
        accepted.elaboration.module.machines[0]
            .blocks
            .iter()
            .any(|block| matches!(
                block.terminator,
                terminal_psi::Terminator::InvokeNoNormal { .. }
            ))
    );
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(32))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
        ),
        "{status:?}"
    );
    let RouteVerdict::Rejected { violations, .. } =
        local(&text.replace("block again(true)", "again(true)"))
    else {
        panic!("non-return must not erase the call acknowledgement")
    };
    assert!(
        violations.iter().any(|violation| violation
            .message()
            .contains("requires a block acknowledgement")),
        "{violations:?}"
    );
}

#[test]
fn a_tail_call_keeps_its_conservative_service_interface_too() {
    let text = r#"
        boundary trait Output {}
        machine first(value: bool) reaches none {
            transition value { true -> second(false) _ -> done() }
            machine done() {}
        }
        public machine second(value: bool) reaches Output {
            transition value { true -> first(false) _ -> done() }
            machine done() {}
        }
        machine main() { first(true); }
    "#;
    match judge(invocation(text)).unwrap() {
        RouteVerdict::Rejected { exceeded, .. } => assert!(!exceeded.is_empty()),
        _ => panic!("tail jump must preserve the callee's service contract"),
    }
}

#[test]
fn canonical_roundtrip_keeps_bounds_and_source_acknowledgements() {
    let typed = super::build_purpose::frontier_of(BLOCKING, "machine build_entry() {}");
    let RouteVerdict::Accepted(accepted) = judge(elaborate(
        typed,
        terminal_codec::TerminalPublication::Portable,
        &[],
    ))
    .unwrap() else {
        panic!("portable source must be accepted");
    };
    let module = &accepted.elaboration.module;
    let bytes = terminal_codec::encode_module(module).unwrap();
    let decoded = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(&decoded, module);
    assert!(decoded.machines[0].published_may_block);
    assert!(
        decoded.machines[0]
            .call_formations
            .iter()
            .any(|row| row.acknowledgement.acknowledges_block)
    );
}

fn selected_verdicts(text: &str) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    let typed = boundary_calls::typed(text);
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap()
        .symbol;
    // A real selection owes its implementation witness. Use the same complete
    // preparation/judgment route as the public reach-ceiling source control.
    crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &typed,
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
        .program,
    )
}

#[test]
fn a_selected_call_keeps_its_public_blocking_envelope() {
    // The implementation does not wait, but callers still owe the public
    // possibility of blocking and the source acknowledgement of that call.
    const SOURCE: &str = r#"
        trait Probe { machine wait() -> u32 blocks; }
        machine implementation() -> u32 blocks false { 7 }
        Named satisfies Probe { Probe::wait = implementation; }
        machine main() -> u32 blocks { block Named::wait() }
    "#;
    let verdicts = selected_verdicts(SOURCE);
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Ok(RouteVerdict::Accepted(_)))),
        "{verdicts:?}"
    );
    let Ok(RouteVerdict::Accepted(accepted)) = &verdicts[0] else {
        unreachable!()
    };
    let module = &accepted.elaboration.module;
    let call = module.machines[0].module_invocations().next().unwrap();
    let public = module
        .invocation_machine_contract(call.inputs.callee, call.inputs.required_contract)
        .unwrap();
    assert!(call.inputs.required_contract.is_some());
    assert!(public.published_may_block);
    assert!(
        !module
            .machine(call.inputs.callee)
            .unwrap()
            .published_may_block
    );
    for (source, expected) in [
        (
            SOURCE.replace("blocks { block Named", "blocks false { block Named"),
            "caller's blocks ceiling is false",
        ),
        (
            SOURCE.replace("block Named::wait()", "Named::wait()"),
            "requires a block acknowledgement",
        ),
    ] {
        let verdicts = selected_verdicts(&source);
        assert!(
            matches!(&verdicts[0], Ok(RouteVerdict::Rejected { violations, .. })
            if violations.iter().any(|violation| violation.message().contains(expected))),
            "{verdicts:?}"
        );
    }
}

#[test]
fn a_selected_recursive_call_keeps_an_explicit_unavailable_boundary() {
    // Direct cycles keep their constant-stack lowering. A selected cycle
    // must retain its public call association before that lowering is served.
    let verdicts = selected_verdicts(
        r#"
        trait Step { machine again(); }
        Named satisfies Step { machine again() { Named::again() } }
        machine main() { Named::again(); }
    "#,
    );
    assert!(
        matches!(&verdicts[0], Ok(RouteVerdict::FallBack(refusal))
        if refusal.reason == RefusalReason::Unsupported
            && refusal.detail.contains("selected public contract on a call cycle")),
        "{verdicts:?}"
    );
}
