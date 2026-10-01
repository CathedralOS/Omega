use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan, CheckedTrees,
    admission,
};

#[test]
fn computed_case_return_replays_predecessor_disposal_once() {
    use terminal_psi::{TerminalAffineCleanupAction, Terminator};

    let checked = crate::front_end::checked_program(
        "data Message { case Captured(value: i32); case Empty; }
         data Counter { value: i32; }
         machine snapshot(counter: &mut Counter) -> Message {
             Message::Captured { value: counter.value }
         }
         machine route(counter: &mut Counter) -> i32 {
             transition snapshot(counter) {
                 Message::Captured { value } -> consume(counter, value)
                 _ -> (72)
             }
             state consume(counter: &mut Counter, value: i32) -> i32 {
                 counter.value = value;
                 value
             }
         }",
    );
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("route"))
        .expect("case consumption precedes the private scalar return");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay sees the predecessor's disposal");

    let (machine_position, return_block, subject) = lowered
        .semantic_module
        .machines
        .iter()
        .enumerate()
        .find_map(|(position, machine)| {
            machine.blocks.iter().find_map(|block| {
                let Terminator::StructuralCase { source, cases } = &block.terminator else {
                    return None;
                };
                cases.iter().find_map(|case| {
                    let target = machine
                        .blocks
                        .iter()
                        .find(|block| block.id == case.target)?;
                    let Terminator::Return {
                        cleanup_actions, ..
                    } = &target.terminator
                    else {
                        return None;
                    };
                    assert!(
                        !cleanup_actions
                            .contains(&TerminalAffineCleanupAction::DiscardRoot(*source))
                    );
                    Some((position, target.id, *source))
                })
            })
        })
        .expect("a closed-case edge reaches its private scalar return");
    for duplicate_subject in [false, true] {
        let mut forged = lowered.semantic_module.clone();
        if duplicate_subject {
            let block = forged.machines[machine_position]
                .blocks
                .iter_mut()
                .find(|block| block.id == return_block)
                .unwrap();
            let Terminator::Return {
                cleanup_actions, ..
            } = &mut block.terminator
            else {
                unreachable!();
            };
            cleanup_actions.insert(0, TerminalAffineCleanupAction::DiscardRoot(subject));
        } else {
            for block in &mut forged.machines[machine_position].blocks {
                if let Terminator::StructuralCase { cases, .. } = &mut block.terminator {
                    for case in cases.iter_mut().filter(|case| case.target == return_block) {
                        case.trivial_affine_discards.clear();
                    }
                }
            }
        }
        assert!(
            matches!(
                terminal_verifier::verify_module(
                    &forged,
                    &lowered.proof_bundle,
                    &proof_admission::AdmissionProfile::default(),
                ),
                Err(terminal_verifier::VerificationError::Module(_))
            ),
            "neither duplicate disposal nor omission of predecessor disposal is legal"
        );
    }
}

fn fixture() -> (CheckedTrees, CheckedComposedUnitControlMachinePlan) {
    let source = r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console { machine read_byte() -> ByteRead reaches Console; }
        machine read_one(out: &mut [u8]) reaches Console {
            transition out.len > 0 { true -> read(out) false -> done() }
            state read(out: &mut [u8]) {
                let observed: ByteRead = Console::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> store(out, value)
                    ByteRead::Eof -> done()
                }
            }
            state store(out: &mut [u8], value: i32 [0..=255]) { out[0] = value as u8; }
            state done() {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| {
            plan.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
        })
        .expect("general source graph retains its inspected result")
        .clone();
    (checked, plan)
}

#[test]
fn closed_case_graph_rejoins_source_and_rejects_drift() {
    let (checked, original) = fixture();
    admission::admit(&checked, &original).expect("exact checked source admission");
    for mutation in 0..10 {
        let mut changed = original.clone();
        let state = changed
            .states
            .iter_mut()
            .find(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .unwrap();
        let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } =
            &mut state.terminator
        else {
            panic!("selected case state");
        };
        match mutation {
            0 => cases.reverse(),
            1 => cases[0].case_identity.push_str("-forged"),
            2 => cases[0].payloads[0].field_identity.push_str("-forged"),
            3 => match &mut cases[0].outcome {
                checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                    successor.transfers.clear()
                }
                _ => unreachable!(),
            },
            4 => {
                subject.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: u32::MAX,
                    }
            }
            5 => state.operations.clear(),
            6 => cases[0].payloads[0].target_scalar_parameter_index += 1,
            7 => match &mut cases[0].outcome {
                checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                    successor.statement_ordinal += 1
                }
                _ => unreachable!(),
            },
            8 => match &mut cases[0].outcome {
                checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) => {
                    successor.trivial_affine_discard_parameter_positions.push(0)
                }
                _ => unreachable!(),
            },
            9 => {
                let payload = cases[0].payloads[0].clone();
                cases[0].payloads.push(payload);
            }
            _ => unreachable!(),
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "case mutation {mutation}"
        );
    }
}

/// A closed-sum dispatch on an affine owned parameter consumes it. The case
/// test moves nothing, so each case edge's cleanup evidence names the
/// subject among the parameters dying there; the edge rejoin takes that row
/// as the dispatch's consumption, and the edge plan may not name it again.
#[test]
fn owned_parameter_dispatch_takes_its_subject_cleanup_row() {
    let source = r#"
        data Event { case Add(amount: i32 in Wrapping); case Bonus; }
        data Tally [copy] { total: i32 in Wrapping; }
        data Main { tally: Tally; }
        machine Main::apply(main: &mut Main, event: Event) {
            transition event {
                Event::Add { amount } -> add(main, amount)
                Event::Bonus -> bonus(main)
            }
            state add(main: &mut Main, amount: i32 in Wrapping) {
                main.tally.total = main.tally.total + amount;
            }
            state bonus(main: &mut Main) { main.tally.total = main.tally.total + 10; }
        }
    "#;
    fn dispatch(
        plan: &mut CheckedComposedUnitControlMachinePlan,
    ) -> &mut checked_trees::CheckedComposedUnitControlStatePlan {
        plan.states
            .iter_mut()
            .find(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .expect("the owned subject dispatches as a closed sum")
    }
    let checked = crate::front_end::checked_program(source);
    let mut plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| {
            plan.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
        })
        .expect("the dispatching machine keeps its composed plan")
        .clone();
    admission::admit(&checked, &plan).expect("the dispatch consumes its owned subject");
    let state = dispatch(&mut plan);
    let dispatch_state = state.state;
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } = &state.terminator
    else {
        unreachable!()
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        subject.source
    else {
        panic!("the subject is the owned parameter");
    };
    let position = state.structural_parameters[parameter_index as usize].position;
    assert_eq!(position, 1, "`event` follows the borrowed `main`");
    assert_eq!(cases.len(), 2);
    let evidence = checked
        .facts
        .flow
        .terminal_structural_control_cleanups
        .for_state(plan.machine, dispatch_state)
        .expect("both case edges retain cleanup evidence");
    assert_eq!(evidence.edges.len(), 2);
    assert!(
        evidence
            .edges
            .iter()
            .all(|edge| edge.trivial_affine_discard_parameter_positions == [position]),
        "the subject dies on every case edge and nothing else does"
    );
    for (mutation, refusal) in [
        (
            "named again",
            "Unit graph successor discards disagree with cleanup evidence",
        ),
        (
            "evidence omits",
            "Unit case edge cleanup evidence omits its dispatched subject",
        ),
        (
            "evidence adds",
            "Unit graph successor discards disagree with cleanup evidence",
        ),
    ] {
        let mut changed_plan = plan.clone();
        let mut changed = checked.clone();
        let rows = changed
            .facts
            .flow
            .terminal_structural_control_cleanups
            .states
            .iter_mut()
            .find(|row| row.machine == plan.machine && row.state == dispatch_state)
            .unwrap();
        match mutation {
            "named again" => {
                let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
                    &mut dispatch(&mut changed_plan).terminator
                else {
                    unreachable!()
                };
                let checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(successor) =
                    &mut cases[0].outcome
                else {
                    panic!("both arms name states");
                };
                successor
                    .trivial_affine_discard_parameter_positions
                    .push(position);
            }
            "evidence omits" => rows.edges[0]
                .trivial_affine_discard_parameter_positions
                .clear(),
            "evidence adds" => rows.edges[1]
                .trivial_affine_discard_parameter_positions
                .push(0),
            _ => unreachable!(),
        }
        let error = admission::admit(&changed, &changed_plan)
            .err()
            .expect("the subject is disposed exactly once, by the dispatch");
        assert_eq!(
            error,
            crate::LoweringError::Unsupported(refusal),
            "{mutation}"
        );
    }
}

#[test]
fn closed_case_graph_rejects_missing_or_substituted_local_cleanup() {
    for substitute_root in [false, true] {
        let (mut checked, plan) = fixture();
        admission::admit(&checked, &plan).expect("exact checked source admission");
        let state = plan
            .states
            .iter()
            .find(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .unwrap();
        let handles = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == plan.machine
                    && event.state_symbol == state.state
                    && event.source == language_semantics::PermissionEventSource::StateExit
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        assert!(!handles.is_empty(), "result-local cleanup evidence exists");
        for handle in handles {
            let event = checked.facts.flow.ownership.permissions.get_mut(handle);
            if substitute_root {
                event.root = facts::PlaceRoot::Unknown;
            } else {
                event.source = language_semantics::PermissionEventSource::StateEntry;
            }
        }
        assert!(admission::admit(&checked, &plan).is_err());
    }
}

#[test]
fn owned_result_edges_and_return_rejoin_actual_permission_rows() {
    use language_semantics::{PermissionEventKind, PermissionEventSource, PermissionProvenance};
    let source = r#"
        data Kind { case Missing; case Other; }
        machine make() -> Kind { Kind::Missing }
        machine route(choose: bool) {
            let kind: Kind = make();
            transition choose { true -> first(kind) false -> second(kind) }
            state first(kind: Kind) {}
            state second(kind: Kind) {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .unwrap()
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap()
        .clone();
    admission::admit(&checked, &plan).expect("actual result transfers and parameter returns");
    let transfers = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine && event.kind == PermissionEventKind::Transfer
        })
        .map(|(handle, event)| (handle, event.clone()))
        .collect::<Vec<_>>();
    assert_eq!(transfers.len(), 2);
    for (handle, original) in transfers {
        for mutation in [
            "missing",
            "duplicate",
            "stale path",
            "extra transfer",
            "origin",
        ] {
            let mut changed = checked.clone();
            let permissions = &mut changed.facts.flow.ownership.permissions;
            match mutation {
                "missing" => permissions.get_mut(handle).machine_symbol = Default::default(),
                "duplicate" => {
                    permissions.insert(original.clone());
                }
                "stale path" => {
                    permissions.get_mut(handle).segments =
                        arena::HandleSpan::from_parts(arena::Handle::invalid(), 1)
                }
                "extra transfer" => {
                    let mut extra = original.clone();
                    extra.source = PermissionEventSource::Statement { statement_index: 0 };
                    permissions.insert(extra);
                }
                "origin" => permissions.get_mut(handle).provenance = PermissionProvenance::Unknown,
                _ => unreachable!(),
            }
            assert!(
                admission::admit(&changed, &plan).is_err(),
                "{mutation} must reject without rebuilding plans"
            );
        }
    }
    let drops = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine
                && event.state_symbol != plan.states[0].state
                && event.kind == PermissionEventKind::AffineDrop
        })
        .map(|(handle, event)| (handle, event.clone()))
        .collect::<Vec<_>>();
    assert_eq!(drops.len(), 2);
    for (handle, original) in drops {
        for mutation in ["missing", "duplicate", "stale path", "root"] {
            let mut changed = checked.clone();
            let permissions = &mut changed.facts.flow.ownership.permissions;
            match mutation {
                "missing" => permissions.get_mut(handle).machine_symbol = Default::default(),
                "duplicate" => {
                    permissions.insert(original.clone());
                }
                "stale path" => {
                    permissions.get_mut(handle).segments =
                        arena::HandleSpan::from_parts(arena::Handle::invalid(), 1)
                }
                "root" => permissions.get_mut(handle).root = facts::PlaceRoot::Unknown,
                _ => unreachable!(),
            }
            assert!(
                admission::admit(&changed, &plan).is_err(),
                "return {mutation} must reject"
            );
        }
    }
}

#[test]
fn closed_case_markers_survive_multi_operation_prefix() {
    // Prefix statements may plan several operations at one authored
    // statement (a construction nested in a field store, a call's own
    // continuation): the marker window is the trailing run of
    // `__arm_destructure#V=` locals ahead of the dispatch, not a count of
    // operation rows. Regression for "Unit case body roster drifted" —
    // `bindings + operations` overran the dispatch position.
    let source = r#"
        data Point { x: u64; }
        machine Point::new(x: u64) -> Point { Point { x: x } }
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console { machine read_byte() -> ByteRead reaches Console; }
        data Driver { first: Point; second: Point; }
        machine Driver::run(&mut self) reaches Console {
            transition { _ -> load() }
            state load(&mut self) {
                self.first = Point::new(1);
                self.second = Point::new(2);
                let observed: ByteRead = Console::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> record(value)
                    ByteRead::Eof -> done()
                }
            }
            state record(&mut self, value: i32 [0..=255]) { self.second.x = value as u64; }
            state done(&mut self) {}
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| {
            plan.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
        })
        .expect("general source graph retains its inspected result")
        .clone();
    admission::admit(&checked, &plan).expect("marker run rejoins after multi-operation prefix");
}

/// A dispatch over a call: the parser copies `pick(second)` into both arms'
/// guards and payload reads; the plan evaluates it once.
const COMPUTED_SUBJECT: &str = r#"
    data Pick { case First(v: u64); case Second(v: u64); }
    machine pick(second: bool) -> Pick {
        transition second { true -> (Pick::Second { v: 2 }) _ -> (Pick::First { v: 1 }) }
    }
    machine flip(value: bool) -> bool {
        transition value { true -> (false) _ -> (true) }
    }
    machine route(second: bool) {
        transition pick(second) {
            Pick::First { v } -> first(v)
            Pick::Second { v } -> second_arm(v)
        }
        state first(v: u64) {}
        state second_arm(v: u64) {}
    }
"#;

fn computed_subject_plan(
    source: &str,
) -> (CheckedTrees, Option<CheckedComposedUnitControlMachinePlan>) {
    let checked = crate::front_end::checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("dispatching machine")
        .symbol;
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .cloned();
    (checked, plan)
}

fn computed_dispatch(
    plan: &mut CheckedComposedUnitControlMachinePlan,
) -> Option<&mut checked_trees::CheckedComposedUnitControlStatePlan> {
    plan.states.iter_mut().find(|state| {
        matches!(
            &state.terminator,
            CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. }
                if matches!(
                    subject.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
                )
        )
    })
}

#[test]
fn computed_case_subject_is_one_call_every_arm_reads() {
    let (checked, plan) = computed_subject_plan(COMPUTED_SUBJECT);
    let mut plan = plan.expect("a computed dispatch keeps its composed plan");
    admission::admit(&checked, &plan).expect("one subject call rejoins every arm");
    let state = computed_dispatch(&mut plan).expect("the call subject dispatches as a closed sum");
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } = &state.terminator
    else {
        unreachable!()
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
        binding_ordinal,
    } = subject.source
    else {
        unreachable!()
    };
    assert_eq!(cases.len(), 2);
    let calls = state
        .operations
        .iter()
        .filter_map(|operation| operation.call())
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "the arms read one evaluation");
    assert_eq!(calls[0].coordinate.statement_index, 0);
    assert_eq!(calls[0].coordinate.call_ordinal, 0);
    assert_eq!(
        calls[0]
            .structural_result()
            .map(|result| result.binding_ordinal),
        Some(binding_ordinal)
    );
    for mutation in 0..4 {
        let mut changed = plan.clone();
        let state = computed_dispatch(&mut changed).unwrap();
        let position = state
            .operations
            .iter()
            .position(|operation| operation.call().is_some())
            .unwrap();
        match mutation {
            0 => {
                state.operations.remove(position);
            }
            1 => {
                let duplicate = state.operations[position].clone();
                state.operations.push(duplicate);
            }
            2 => match &mut state.operations[position] {
                super::CheckedUnitEffectOperationPlan::Call(call) => {
                    call.coordinate.statement_index += 1;
                }
                _ => unreachable!(),
            },
            3 => {
                let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, .. } =
                    &mut state.terminator
                else {
                    unreachable!()
                };
                subject.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    };
            }
            _ => unreachable!(),
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "computed subject mutation {mutation}"
        );
    }
}

#[test]
fn computed_case_subject_with_a_call_operand_is_not_read_once() {
    // Every arm's copy repeats `flip(second)`; this route plans no dispatch
    // over a subject whose operands invoke calls of their own.
    let source =
        COMPUTED_SUBJECT.replace("transition pick(second)", "transition pick(flip(second))");
    let (_, plan) = computed_subject_plan(&source);
    assert!(
        plan.is_none_or(|mut plan| computed_dispatch(&mut plan).is_none()),
        "a subject with a call operand must not dispatch from one planned call"
    );
}

const COMPUTED_BOUNDARY_SUBJECT: &str = r#"
    data Pick { case First(v: u64); case Second(v: u64); }
    boundary trait Choice {
        machine pick(second: bool) -> Pick reaches Choice blocks;
        machine other(second: bool) -> Pick reaches Choice blocks;
    }
    boundary trait Observer { machine mark(value: u64) reaches Observer; }
    machine route(second: bool) reaches Choice, Observer {
        transition block Choice::pick(second) {
            Pick::First { v } -> first(v)
            Pick::Second { v } -> second_arm(v)
        }
        state first(v: u64) { Observer::mark(v); }
        state second_arm(v: u64) { Observer::mark(v); }
    }
"#;

#[test]
fn computed_boundary_subject_keeps_one_call_and_selected_payloads() {
    let (checked, plan) = computed_subject_plan(COMPUTED_BOUNDARY_SUBJECT);
    let mut plan = plan.expect("a boundary result is an ordinary completed dispatch subject");
    admission::admit(&checked, &plan).expect("boundary subject retains normal call admission");
    let state = computed_dispatch(&mut plan).expect("one closed-sum dispatch");
    let calls = state
        .operations
        .iter()
        .filter_map(|operation| operation.call())
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "guard and payload copies cannot repeat the boundary effect"
    );
    assert!(calls[0].is_boundary());
    assert!(calls[0].claim_transfers().is_empty());
    assert_eq!(calls[0].discard_result_on_return(), Some(false));
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } = &state.terminator
    else {
        panic!("the boundary result selects its exact case");
    };
    assert_eq!(cases.len(), 2);
    for case in cases {
        assert_eq!(
            case.payloads.len(),
            1,
            "only the selected case supplies its scalar payload"
        );
        assert_eq!(case.payloads[0].field_identity, "v");
    }
    let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("route"))
        .expect("computed boundary result lowers through the ordinary call emitter");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("Terminal independently verifies the result and case consumption");
}

#[test]
fn computed_boundary_subject_rejects_target_occurrence_result_and_receipt_substitution() {
    let (checked, plan) = computed_subject_plan(COMPUTED_BOUNDARY_SUBJECT);
    let plan = plan.expect("boundary computed subject plan");
    let other = checked
        .traits()
        .iter()
        .flat_map(|owner| checked.trait_machine_signatures(owner))
        .find(|signature| signature.name.as_str() == "other")
        .expect("same-signature alternate boundary")
        .symbol;
    for mutation in 0..7 {
        let mut changed = plan.clone();
        let state = computed_dispatch(&mut changed).unwrap();
        let position = state
            .operations
            .iter()
            .position(|operation| operation.call().is_some())
            .unwrap();
        if mutation == 0 {
            state.operations.push(state.operations[position].clone());
        } else {
            let call = state.operations[position].call_mut().unwrap();
            match mutation {
                1 => call.coordinate.call_ordinal = 1,
                2 => {
                    call.target.machine = other;
                    call.target.state = other;
                }
                3 => call.target.contract_report_fingerprint ^= 1,
                4 => {
                    call.claim_transfers_mut()
                        .push(checked_trees::CheckedUnitClaimTransferPlan {
                            claim_identity: language_semantics::PermissionClaimIdentity::Unknown,
                            argument_index: 0,
                        });
                }
                5 | 6 => {
                    let (result, _) = state.operations[position]
                        .discardable_structural_result_mut()
                        .expect("boundary retains its structural result");
                    if mutation == 5 {
                        result.binding_ordinal = u32::MAX;
                    } else {
                        result.type_identity.push_str("-substituted");
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "boundary subject mutation {mutation}"
        );
    }
}
