//! A guarded-pair edge whose argument is the selected case's `[copy]`
//! payload field: `subject.Case::field` transfers through the edge's proven
//! membership into the target's owned custody. The independent rejoin checks
//! the minted plan against the authored guard, the declared case member, and
//! the exact operand expression.
use super::super::edges;
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan, CheckedTrees,
    admission,
};

fn fixture() -> (CheckedTrees, CheckedComposedUnitControlMachinePlan) {
    let source = r#"
        data Info [copy] { pid: u32; }
        data OpenResult { case Opened(info: Info); case Failed(error: Info); }
        data Root { result: OpenResult; hit: i32; }
        machine Root::run(&mut self) {
            transition self.result {
                OpenResult::Opened { info } -> have(info)
                OpenResult::Failed { error } -> failed()
            }
            state have(&mut self, info: Info) { self.hit = 70; }
            state failed(&mut self) { self.hit = 71; }
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
                    &state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. }
                        if when_true.transfers.iter().any(|transfer| matches!(
                            transfer.source,
                            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                                ..
                            }
                        ))
                )
            })
        })
        .expect("the payload-bearing case pair retains its minted transfer")
        .clone();
    (checked, plan)
}

fn payload_edge<'a>(
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
) -> (
    &'a checked_trees::state::State,
    &'a super::super::CheckedComposedUnitControlStatePlan,
    &'a checked_trees::statement::TableTransition,
    &'a checked_trees::CheckedStructuralControlSuccessorPlan,
) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .expect("the fixture machine");
    let (state, successor) = plan
        .states
        .iter()
        .find_map(|state| {
            let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
                &state.terminator
            else {
                return None;
            };
            when_true
                .transfers
                .iter()
                .any(|transfer| {
                    matches!(
                        transfer.source,
                        checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                            ..
                        }
                    )
                })
                .then_some((state, when_true))
        })
        .expect("the payload edge state");
    let source = checked
        .machine_states(machine)
        .iter()
        .find(|source| source.symbol == state.state)
        .expect("the payload edge source state");
    let checked_trees::statement::StatementNode::Transition(transition) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(successor.statement_ordinal as usize)
        .expect("the authored transition")
    else {
        panic!("the successor ordinal names a transition")
    };
    (source, state, transition, successor)
}

#[test]
fn case_payload_edge_rejoins_the_exact_checked_plan() {
    let (checked, plan) = fixture();
    let (source, state, transition, successor) = payload_edge(&checked, &plan);
    edges::validate(
        &checked,
        &plan,
        source,
        state,
        transition,
        successor,
        successor.statement_ordinal as usize,
    )
    .expect("exact case-payload edge admission");
}

#[test]
fn case_payload_edge_rejects_source_drift() {
    let (checked, plan) = fixture();
    for mutation in 0..7 {
        let mut changed = plan.clone();
        let state = changed
            .states
            .iter_mut()
            .find(|state| {
                matches!(
                    &state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. }
                        if when_true.transfers.iter().any(|transfer| matches!(
                            transfer.source,
                            checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                                ..
                            }
                        ))
                )
            })
            .expect("the payload edge state");
        let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
            &mut state.terminator
        else {
            unreachable!()
        };
        let checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
            subject,
            case_identity,
            field_identity,
            path,
        } = &mut when_true
            .transfers
            .iter_mut()
            .find(|transfer| {
                matches!(
                    transfer.source,
                    checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. }
                )
            })
            .expect("the payload transfer")
            .source
        else {
            panic!("the fixture carries a case-payload transfer")
        };
        match mutation {
            0 => case_identity.push_str("-forged"),
            1 => field_identity.push_str("-forged"),
            2 => subject.access = checked_trees::CheckedStructuralAccess::Owned,
            3 => subject.path.clear(),
            4 => subject.type_identity.push_str("-forged"),
            5 => {
                subject.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: u32::MAX,
                    }
            }
            _ => path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                "extra".to_owned(),
            )),
        }
        let (source, state, transition, successor) = payload_edge(&checked, &changed);
        assert!(
            edges::validate(
                &checked,
                &changed,
                source,
                state,
                transition,
                successor,
                successor.statement_ordinal as usize,
            )
            .is_err(),
            "case-payload mutation {mutation}"
        );
    }
}

#[test]
fn case_payload_edge_rejects_target_drift() {
    let (checked, plan) = fixture();
    let mut changed = plan.clone();
    let state = changed
        .states
        .iter_mut()
        .find(|state| {
            matches!(
                &state.terminator,
                CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. }
                    if when_true.transfers.iter().any(|transfer| matches!(
                        transfer.source,
                        checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload {
                            ..
                        }
                    ))
            )
        })
        .expect("the payload edge state");
    let CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, .. } =
        &mut state.terminator
    else {
        unreachable!()
    };
    when_true
        .transfers
        .iter_mut()
        .find(|transfer| {
            matches!(
                transfer.source,
                checked_trees::CheckedStructuralControlTransferSourcePlan::CasePayload { .. }
            )
        })
        .expect("the payload transfer")
        .target_parameter_index += 1;
    let (source, state, transition, successor) = payload_edge(&checked, &changed);
    assert!(
        edges::validate(
            &checked,
            &changed,
            source,
            state,
            transition,
            successor,
            successor.statement_ordinal as usize,
        )
        .is_err()
    );
}

#[test]
fn case_payload_body_markers_stay_the_residual() {
    // The `__arm_destructure` locals a payload-bearing case pair mints have
    // no body-accounting lane under a Conditional terminator — marker
    // validation is ClosedSum-only — so the composed plan still does not
    // lower end to end. Naming the wall explicitly.
    let (checked, plan) = fixture();
    let error = match admission::admit(&checked, &plan) {
        Err(error) => error,
        Ok(_) => panic!("the marker residual stands"),
    };
    assert!(
        format!("{error}").contains("dropped or added a body effect"),
        "the structural destructure markers are the residual: {error}"
    );
}
