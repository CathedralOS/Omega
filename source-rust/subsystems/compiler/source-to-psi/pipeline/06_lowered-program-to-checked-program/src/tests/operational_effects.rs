//! Blocking is an interface promise and an independently acknowledged call.

use super::*;

mod guest_discard;

const BLOCKING: &str = r#"
public machine may_wait() -> u32 blocks { 7 }
public machine wrapped() -> u32 blocks { block may_wait() }
machine main() crashes Trap {
    let answer: u32 = block wrapped();
    transition answer == 7 {
        true -> done(),
        _ -> bad()
    }
    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#;

fn invocation(text: &str) -> Checked<crate::lowering::Elaboration> {
    elaborate(
        boundary_calls::typed(text),
        terminal_codec::TerminalPublication::Invocation,
        &[],
    )
}

fn elaborate(
    typed: crate::source_state::TypedTrees,
    publication: terminal_codec::TerminalPublication,
    subjects: &[Option<semantic_vocabulary::PackageKeyIdentity>],
) -> Checked<crate::lowering::Elaboration> {
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap()
        .symbol;
    let frozen: crate::source_state::FrozenTypedTrees = typed.into();
    crate::lowering::elaborate_for(
        &frozen,
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects,
            purpose: source_provenance::DependencyScope::Product,
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
    let (wrapped, _) = accepted
        .elaboration
        .names
        .iter()
        .find(|(_, names)| names.machine == "wrapped")
        .expect("the checked closure includes the private wrapper");
    let wrapper = accepted.elaboration.module.machine(wrapped).unwrap();
    assert_eq!(wrapper.declared_blocking, None);
    assert!(wrapper.published_may_block);
    assert!(!wrapper.published_may_suspend);
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
        true -> done(),
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
    assert_eq!(module.execution.suspension_call_sites.len(), 2);
    assert_eq!(module.execution.suspension_call_plans.len(), 2);
    assert_eq!(module.execution.suspension_call_plan_count, 2);
    let frontiers: Vec<usize> = module
        .execution.suspension_call_plans
        .iter()
        .map(|plan| plan.live_values.len())
        .collect();
    assert!(frontiers.contains(&0), "{frontiers:?}");
    assert!(frontiers.contains(&1), "{frontiers:?}");
    let kept = module
        .execution.suspension_call_plans
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

#[test]
fn a_pending_host_call_resumes_the_same_borrow_and_outstanding_return() {
    use std::cell::Cell;
    use terminal_interpreter::{
        TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecutionResult,
        TerminalExecutionStatus, TerminalImportAdapter, TerminalImportSignature,
        TerminalInstantiation, TerminalProgram, TerminalRuntimeSupplyRequest, TerminalScalarValue,
    };
    struct Gate<'a> {
        signature: TerminalImportSignature,
        calls: &'a Cell<usize>,
        ready: &'a Cell<bool>,
    }
    impl TerminalImportAdapter for Gate<'_> {
        fn implemented_signature(&self) -> TerminalImportSignature {
            self.signature.clone()
        }
    }
    impl TerminalEffectHandler for Gate<'_> {
        fn poll_host_computation(
            &mut self,
            _: &TerminalEffect,
        ) -> Result<bool, TerminalEffectRejection> {
            Ok(self.calls.get() == 0 || self.ready.get())
        }
        fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
            self.calls.set(self.calls.get() + 1);
            Ok(())
        }
    }

    let source = r"
boundary trait Gate { machine visit() reaches none crashes none suspends; }
machine initial() -> u8 reaches Gate invokes Gate suspends {
    suspend Gate::visit();
    3
}
machine later() -> u8 reaches Gate invokes Gate suspends {
    suspend Gate::visit();
    7
}
machine main() -> bool reaches Gate invokes Gate suspends {
    let mutable cell: u8 = suspend initial();
    let held: @mutable u8 = @mutable cell;
    let value: u8 = suspend later();
    let initial_value: u8 = $held;
    $held = value;
    initial_value == 3 && cell == 7
}";
    let checked = accepted(source).publish_invocation().unwrap();
    let program = TerminalProgram::borrow(checked.artifact.view()).unwrap();
    let imports: Vec<_> = program.imports().collect();
    let [gate] = imports.as_slice() else {
        panic!("one callable host slot expected")
    };
    let supplies = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            host_imports: &[gate.id],
            installation: None,
            target_machines: None,
            target_layouts: None,
            native_code_layouts: None,
            x86_flags: None,
            memory_ordering: None,
            entry: None,
            external_completions: &[],
        })
        .unwrap();
    let calls = Cell::new(0);
    let ready = Cell::new(false);
    let mut instance = program
        .instantiate(TerminalInstantiation {
            installation: Some(&supplies),
            ..Default::default()
        })
        .unwrap();
    instance
        .bind_import(
            gate.id,
            Gate {
                signature: TerminalImportSignature::from(*gate),
                calls: &calls,
                ready: &ready,
            },
        )
        .unwrap();
    let mut meter = terminal_work::WorkMeter::with_budget(10_000);
    assert!(matches!(
        instance.invoke(&mut meter).unwrap(),
        TerminalExecutionStatus::HostComputationPending(_)
    ));
    assert_eq!(calls.get(), 1);
    assert!(instance.caller_sites().next().is_some());
    ready.set(true);
    assert!(matches!(
        instance.invoke(&mut meter).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    ));
    assert_eq!(calls.get(), 2);
}

/// A crossing is the checker's to reconstruct, not the producer's to
/// assert: "Matching a producer's site and plan is not enough: deleting both
/// must not erase a required crossing", and "Missing, duplicate, redirected,
/// or understated rows reject."
#[test]
fn a_missing_or_altered_crossing_is_not_accepted() {
    let refusal = |alter: &dyn Fn(&mut crate::lowering::Elaboration)| {
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
            module.execution.suspension_call_sites.clear();
            module.execution.suspension_call_plans.clear();
            module.execution.suspension_call_plan_count = 0;
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
            for plan in &mut elaboration.module.execution.suspension_call_plans {
                plan.live_values.clear();
                plan.live_value_count = 0;
            }
        }),
        "a suspension plan is not the live frontier of its call"
    );
    // A site commits to another frontier.
    assert_eq!(
        refusal(&|elaboration| {
            elaboration.module.execution.suspension_call_sites[0].frontier_commitment = [0; 32];
        }),
        "a suspension site does not commit to the live frontier of its call"
    );
    // An operation that cannot suspend claims a crossing.
    assert_eq!(
        refusal(&|elaboration| {
            let crossing = elaboration.module.execution.suspension_call_plans[0].crossing;
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
        true -> block again(false),
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
        "../../tests/fixtures/calls/run/operational_contracts/main.omg"
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
        let typed = crate::prepare_fixture(boundary_calls::typed(&text));
        let implementation = typed
            .trees()
            .machines()
            .iter()
            .find(|row| row.name.as_str() == "implementation")
            .unwrap()
            .symbol;
        let package = typed
            .trees()
            .symbols
            .symbol_product_package_identity(implementation)
            .expect("the checked fixture has its actual package identity");
        // The implementation body and its refinement are separate obligations.
        // Check both in the same compilation instead of a bodyless logical form.
        crate::judge_compilation(
            crate::lowering::prepare_compilation(
                &typed,
                crate::lowering::Compilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[],
                    subjects: &[Some(package)],
                    purpose: source_provenance::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .expect("fixture must lower to checker input")
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
        Err(crate::PublicationError::GenericCheckForm(_))
    ));
    assert!(matches!(
        check_form.publish_invocation(),
        Err(crate::PublicationError::GenericCheckForm(_))
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
    let program =
        terminal_interpreter::TerminalProgram::borrow(publication.artifact.view()).unwrap();
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
            transition value { true -> second(false), _ -> done() }
            machine done() {}
        }
        public machine second(value: bool) reaches Output {
            transition value { true -> first(false), _ -> done() }
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
    let typed = crate::prepare_fixture(boundary_calls::typed(text));
    let entry = typed
        .trees()
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap()
        .symbol;
    // A real selection owes its implementation witness. Use the same complete
    // preparation/judgment route as the public reach-ceiling source control.
    crate::judge_compilation(
        crate::lowering::prepare_compilation(
            &typed,
            crate::lowering::Compilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("fixture must lower to checker input")
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

#[test]
fn a_tail_call_keeps_its_borrowed_formal_contract() {
    let original = invocation(
        r#"
        machine main(bytes: @[u8], pending: bool) blocks {
            transition pending {
                true -> block main(bytes, false),
                false -> done()
            }
            machine done() {}
        }
    "#,
    )
    .unwrap();
    crate::operational::judge(&original).unwrap();
    for change in 0..3 {
        let mut forged = original.clone();
        let machine = &mut forged.module.machines[0];
        let target = machine
            .blocks
            .iter()
            .find_map(|block| {
                let Terminator::Jump { edge, target, .. } = block.terminator else {
                    return None;
                };
                machine
                    .call_formations
                    .iter()
                    .any(|row| {
                        row.site == terminal_psi::CallSite::Edge(edge) && row.tail_callee.is_some()
                    })
                    .then_some(target)
            })
            .unwrap();
        let parameter = &mut machine
            .blocks
            .iter_mut()
            .find(|block| block.id == target)
            .unwrap()
            .structural_parameters[0];
        match change {
            0 => parameter.access = terminal_psi::StructuralAccess::MutableBorrow,
            1 => parameter.multiplicity = terminal_psi::StructuralMultiplicity::Affine,
            _ => parameter.position += 1,
        }
        let refusal = crate::operational::judge(&forged).unwrap_err();
        assert!(
            refusal
                .detail
                .contains("does not carry its callable's formals"),
            "{}",
            refusal.detail
        );
    }
}

#[test]
fn equally_typed_live_borrows_retain_their_own_referents() {
    use terminal_psi::{
        TerminalModule, TerminalSuspensionLoanBacking, TerminalSuspensionPlace,
        TerminalSuspensionStorage, TerminalSuspensionValueLoans,
    };

    // A consumer can trace both live permissions using only the published module.
    fn backing_links(
        module: &TerminalModule,
    ) -> (
        usize,
        [arena::Handle<terminal_psi::TerminalSuspensionLoan>; 2],
    ) {
        let (index, plan) = module
            .execution
            .suspension_call_plans
            .iter()
            .enumerate()
            .find(|(_, plan)| {
                plan.live_values
                    .iter()
                    .filter(|row| {
                        row.storage == TerminalSuspensionStorage::Parameter
                            && matches!(row.place, TerminalSuspensionPlace::Structural(_))
                    })
                    .count()
                    == 2
            })
            .expect("both entry borrows live across the wait");
        let machine = module
            .machines
            .iter()
            .find(|body| body.operation(plan.operation).is_some())
            .unwrap();
        let [first, second] = machine.structural_parameters.as_slice() else {
            panic!("two record parameters")
        };
        assert_eq!(first.structural_type, second.structural_type);
        assert_ne!(first.place, second.place);
        let links = [first, second].map(|parameter| {
            let row = plan.live_values.iter().find(|row| matches!(&row.place, TerminalSuspensionPlace::Structural(arg) if arg.place == parameter.place)).unwrap();
            let TerminalSuspensionValueLoans::Retained { links } = row.loans else { panic!("missing loan: {row:?}") };
            let [link] = plan.loans.value_links.span_or_empty(links) else { panic!("expected a single ingress loan") };
            let TerminalSuspensionLoanBacking::IncomingReferent { argument } = &plan.loans.occurrences.get(*link).backing else { panic!("lost incoming referent") };
            assert_eq!(argument.place, parameter.place);
            assert_eq!(argument.access, parameter.access);
            assert!(argument.path.is_empty());
            *link
        });
        (index, links)
    }

    let source = r#"
data Cell { mark: u32; }
public machine may_wait() -> u32 suspends { 7u32 }
machine main(left: @Cell, right: @Cell) -> bool suspends {
    let answer: u32 = suspend may_wait();
    left.mark == right.mark && answer == 7u32
}
"#;
    let mut lowered = invocation(source).unwrap();
    let (index, [first, second]) = backing_links(&lowered.module);
    crate::operational::judge(&lowered).expect("exact incoming referents");
    let plan = &mut lowered.module.execution.suspension_call_plans[index];
    let first_backing = plan.loans.occurrences.get(first).backing.clone();
    plan.loans.occurrences.get_mut(first).backing =
        plan.loans.occurrences.get(second).backing.clone();
    plan.loans.occurrences.get_mut(second).backing = first_backing;
    let site = lowered
        .module
        .execution
        .suspension_call_sites
        .iter_mut()
        .find(|site| site.crossing == plan.crossing)
        .unwrap();
    site.frontier_commitment =
        terminal_psi::suspension_frontier_commitment(&lowered.module.identities, plan);
    let refused = crate::operational::judge(&lowered).unwrap_err();
    assert_eq!(
        refused.detail,
        "a suspension plan is not the live frontier of its call"
    );
}

#[test]
fn a_local_borrow_keeps_its_backing_storage_in_the_public_plan() {
    assert_local_borrow_retains_backing(
        r#"
data Cell { mark: u32; }
public machine may_wait() -> u32 suspends { 7u32 }
machine main() -> bool suspends {
    let mutable cell: Cell = Cell { mark: 3u32 };
    let held: @mutable Cell = @mutable cell;
    let seven: u32 = suspend may_wait();
    held.mark = seven;
    cell.mark == 7u32
}
"#,
        &[],
    );
}

#[test]
fn storing_a_scalar_through_a_loan_keeps_its_backing_in_the_public_plan() {
    assert_local_borrow_retains_backing(
        r#"
public machine may_wait() -> u32 suspends { 7u32 }
machine main() -> bool suspends {
    let mutable amount: u32 = 3u32;
    let held: @mutable u32 = @mutable amount;
    let seven: u32 = suspend may_wait();
    $held = seven;
    amount == 7u32
}
"#,
        &[],
    );
}

#[test]
fn a_fixed_array_element_loan_retains_its_exact_index_in_the_public_plan() {
    assert_local_borrow_retains_backing(
        r#"
public machine may_wait() -> u32 suspends { 7u32 }
machine main() -> bool suspends {
    let mutable amounts: [u32; 2] = [3u32, 9u32];
    let held: @mutable u32 = @mutable amounts[0u64];
    let seven: u32 = suspend may_wait();
    $held = seven;
    amounts[0u64] == 7u32
}
"#,
        &[terminal_psi::StructuralPathSegment::FixedIndex(0)],
    );
}

fn assert_local_borrow_retains_backing(source: &str, path: &[terminal_psi::StructuralPathSegment]) {
    let lowered = invocation(source).unwrap();
    crate::operational::judge(&lowered).unwrap();
    let module = &lowered.module;
    let [plan] = module.execution.suspension_call_plans.as_slice() else {
        panic!("one wait")
    };
    let captured = plan
        .loans
        .occurrences
        .iter()
        .find_map(|(_, loan)| {
            let terminal_psi::TerminalSuspensionLoanCapture::Reference {
                formation,
                carrier,
                source,
                source_path,
                ..
            } = loan.capture
            else {
                return None;
            };
            let terminal_psi::TerminalSuspensionLoanBacking::RetainedOwner { live_value } =
                loan.backing
            else {
                return None;
            };
            assert_eq!(plan.loans.paths.span_or_empty(source_path), path);
            Some((formation, carrier, source, live_value, loan.access))
        })
        .expect("the local reference formation names its retained backing");
    let (formation, carrier, source, index, access) = captured;
    assert_eq!(access, terminal_psi::StructuralAccess::MutableBorrow);
    assert_ne!(source, carrier);
    assert!(
        matches!(&plan.live_values[index as usize].place, terminal_psi::TerminalSuspensionPlace::Structural(arg) if arg.place == source)
    );
    let operation = module
        .machines
        .iter()
        .find_map(|machine| machine.operation(formation))
        .unwrap();
    assert_eq!(operation.result.as_structural().unwrap().place, carrier);
    assert!(
        matches!(&operation.kind, terminal_psi::OperationKind::EstablishReference { source: arg, .. } if arg.place == source)
    );
}

#[test]
fn a_later_delegation_does_not_change_a_loan_at_an_earlier_wait() {
    let source = r#"
data Cell { mark: u32; }
public machine may_wait() -> u32 suspends { 7u32 }
machine observe(cell: @Cell) -> u32 { cell.mark }
machine main() -> bool suspends {
    let cell: Cell = Cell { mark: 3u32 };
    let held: @Cell = @cell;
    let first: u32 = suspend may_wait();
    let mark: u32 = observe(held);
    let second: u32 = suspend may_wait();
    held.mark == mark && first == second
}
"#;
    let lowered = invocation(source).unwrap();
    crate::operational::judge(&lowered).unwrap();
    let plans = &lowered.module.execution.suspension_call_plans;
    let [first, second] = plans.as_slice() else {
        panic!("two ordinary suspension calls")
    };
    let has_local_loan = |plan: &terminal_psi::TerminalSuspensionCallPlan| {
        plan.loans.occurrences.iter().any(|(_, loan)| {
            matches!(
                loan.capture,
                terminal_psi::TerminalSuspensionLoanCapture::Reference { .. }
            )
        })
    };
    assert!(
        has_local_loan(first),
        "the later call has not borrowed held at the first wait"
    );
    assert!(
        !has_local_loan(second),
        "a prior possible delegation needs separate child-lifetime evidence"
    );
}

#[test]
fn a_literal_established_after_the_wait_has_no_home_in_its_frontier() {
    let source = r#"
public machine may_wait() -> u32 suspends { 7u32 }
machine read(first: @[u8], second: @[u8]) {}
machine main() suspends {
    let before: @[u8] = "before";
    let result: u32 = suspend may_wait();
    let after: @[u8] = "after";
    read(before, after);
}
"#;
    let lowered = invocation(source).unwrap();
    crate::operational::judge(&lowered).unwrap();
    let module = &lowered.module;
    let [plan] = module.execution.suspension_call_plans.as_slice() else {
        panic!("one ordinary wait")
    };
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.operation(plan.operation).is_some())
        .unwrap();
    let has_literal = |literal: &[u8]| {
        let place = machine
            .operations()
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                    destination,
                    bytes,
                    ..
                } if bytes == literal => Some(*destination),
                _ => None,
            })
            .expect("the checked machine contains both literal establishments");
        plan.live_values.iter().any(|value| {
            matches!(&value.place,
            terminal_psi::TerminalSuspensionPlace::Structural(argument) if argument.place == place)
        })
    };
    assert!(has_literal(b"before"));
    assert!(!has_literal(b"after"));
}
