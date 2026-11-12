use super::*;

fn predecessor() -> Machine {
    machine(
        vec![u32_type()],
        vec![returns(strict(
            Strict::Subtract,
            slot(0, u32_type()),
            unsigned(1),
        ))],
    )
}

fn guarded_calls(other: u128) -> Machine {
    machine(
        vec![u32_type()],
        vec![
            Statement::Arm {
                guard: Some(expression(
                    ExpressionKind::Strict {
                        operation: Strict::Less,
                        left: Box::new(unsigned(0)),
                        right: Box::new(slot(0, u32_type())),
                    },
                    Some(ScalarType::Boolean),
                )),
                target: Target::Value(call(2, vec![slot(0, u32_type())])),
                source: nowhere(),
            },
            returns(call(2, vec![unsigned(other)])),
        ],
    )
}

fn with_private_inputs(
    machines: Vec<Machine>,
    private: &[u64],
) -> structure_elaboration::Elaboration {
    let mut elaboration = lower(machines).unwrap();
    elaboration.private_inputs = private.iter().copied().map(id).collect();
    elaboration
}

#[test]
fn every_private_arrival_establishes_the_inferred_input() {
    let RouteVerdict::Accepted(accepted) = judge(Ok(with_private_inputs(
        vec![guarded_calls(1), predecessor()],
        &[2],
    )))
    .unwrap() else {
        panic!("both callers establish a positive input")
    };
    assert_eq!(
        accepted.elaboration.module.machines[1]
            .contract
            .requires
            .len(),
        1
    );
    assert!(!verdicts(&accepted.elaboration, &accepted.discharge, 1, requirement).is_empty());
    for (machine, result) in accepted.discharge.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            if let Ok(Discharged::Certificate(evidence)) = verdict {
                assert_kernel_accepted(&accepted.elaboration, machine.get(), obligation, evidence);
            }
        }
    }
}

#[test]
fn one_unsafe_private_arrival_is_not_hidden_by_a_guarded_arrival() {
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(with_private_inputs(
        vec![guarded_calls(0), predecessor()],
        &[2],
    )))
    .unwrap() else {
        panic!("the zero arrival cannot justify subtracting one")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(2) && safety(&row.kind))
    );
}

#[test]
fn private_inputs_cross_an_omitted_helper_chain() {
    let mut entry = guarded_calls(1);
    // Both entry arms call machine 2, which forwards to the predecessor.
    let forward = machine(
        vec![u32_type()],
        vec![returns(call(3, vec![slot(0, u32_type())]))],
    );
    entry.requires.clear();
    assert!(matches!(
        judge(Ok(with_private_inputs(
            vec![entry, forward, predecessor()],
            &[2, 3]
        )))
        .unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

#[test]
fn a_fixed_input_boundary_does_not_receive_its_callers_stronger_fact() {
    let entry = machine(vec![], vec![returns(call(2, vec![unsigned(2)]))]);
    let forward = machine(
        vec![u32_type()],
        vec![returns(call(3, vec![slot(0, u32_type())]))],
    );
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(with_private_inputs(
        vec![entry, forward, predecessor()],
        &[3],
    )))
    .unwrap() else {
        panic!("the complete forwarding contract permits zero")
    };
    assert!(
        unproved
            .iter()
            .any(|row| row.machine == id(3) && safety(&row.kind))
    );
}

#[test]
fn a_private_call_without_normal_return_still_owes_its_inferred_input() {
    let mut bodies = IdTable::default();
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![returns(call(2, vec![slot(0, u32_type())]))],
    });
    bodies.push(Body {
        slots: vec![u32_type().into()],
        statements: vec![
            Statement::Evaluate(strict(Strict::Subtract, slot(0, u32_type()), unsigned(1))),
            Statement::Arm {
                guard: None,
                target: Target::Crash(semantic_vocabulary::CrashCause::Trap),
                source: nowhere(),
            },
        ],
    });
    let mut signatures = IdTable::default();
    for requires in [vec![at_least(1, 1)], Vec::new()] {
        signatures.push(Signature {
            parameters: vec![u32_type().into()],
            erased: Vec::new(),
            result: Some(u32_type().into()),
            requires,
            ensures: Vec::new(),
            crashes: CrashCeiling::Authored(trap_when(vec![terminal_psi::CrashRouteGuard::Truth])),
            termination: Termination::Silent,
        });
    }
    let mut elaboration =
        structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &IdTable::default())
            .unwrap();
    elaboration.private_inputs = vec![id(2)];
    let RouteVerdict::Accepted(accepted) = judge(Ok(elaboration)).unwrap() else {
        panic!("the non-returning call has a positive input")
    };
    assert!(!verdicts(&accepted.elaboration, &accepted.discharge, 1, requirement).is_empty());
}
