use super::*;
use crate::lowering::Borrowed;
use language_core::ReferenceAccess;
use terminal_psi::{TerminalAffineCleanupAction, Terminator};

/// The moved contents, rather than a caller guard, establish the call bound.
fn moved_record(first: u128) -> crate::lowering::Elaboration {
    elaborate_over(
        vec![
            Shaped {
                parameters: Vec::new(),
                locals: vec![point(), point()],
                result: u32_type().into(),
                requires: Vec::new(),
                statements: vec![
                    bind(0, point_of(unsigned(first), unsigned(7))),
                    bind(1, place(0, point())),
                    returns(call(2, vec![read(place(1, point()), 0)])),
                ],
            },
            bounded(),
        ],
        &affine_records(),
    )
}

#[test]
fn a_whole_move_preserves_the_fact_needed_by_a_bounded_call() {
    let elaboration = moved_record(5);
    assert!(!elaboration.module.types.structural_types[0].is_copyable());
    let caller = &elaboration.module.machines[0];
    assert!(
        caller
            .operations()
            .any(|operation| matches!(operation.kind, OperationKind::MoveStructuralValue { .. }))
    );
    let actions = caller
        .blocks
        .iter()
        .flat_map(|block| block.terminator.cleanup_rosters())
        .flat_map(|(_, actions)| actions)
        .collect::<Vec<_>>();
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)))
    );
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

#[test]
fn a_whole_move_does_not_establish_an_untrue_call_bound() {
    let RouteVerdict::Rejected { unproved, .. } = judge(Ok(moved_record(20))).unwrap() else {
        panic!("moving the record must not prove its field meets the call requirement");
    };
    assert!(
        unproved
            .iter()
            .any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. })),
        "the moved field still exceeds the callee's bound"
    );
}

#[test]
fn a_no_normal_borrow_keeps_the_callers_affine_owner_suspended() {
    let mut bodies = IdTable::default();
    let mut signatures = IdTable::default();
    let borrowed = of_type(
        ExpressionKind::Borrow {
            access: ReferenceAccess::Mutable,
            place: Box::new(place(0, point())),
            home: None,
        },
        point(),
    );
    bodies.push(Body {
        slots: vec![point()],
        statements: vec![
            bind(0, point_of(unsigned(73), unsigned(97))),
            Statement::Evaluate {
                expression: Expression {
                    call_context: None,
                    kind: ExpressionKind::Call {
                        required_contract: None,
                        static_reach_binding: None,
                        static_reach_arguments: Vec::new(),
                        proof_arguments: Vec::new(),
                        callee: id::<MachineId>(2).into(),
                        arguments: vec![borrowed],
                        designated: None,
                    },
                    result: None,
                    source: nowhere(),
                },
                discards_result: false,
            },
        ],
    });
    bodies.push(Body {
        slots: vec![point()],
        statements: vec![
            Statement::discard(read(place(0, point()), 0)),
            Statement::Arm {
                guard: ArmGuard::Always,
                target: Target::Crash(semantic_vocabulary::CrashCause::Trap),
                source: nowhere(),
            },
        ],
    });
    for borrowed in [false, true] {
        signatures.push(Signature {
            membership_ensures: Vec::new(),
            content_ensures: Vec::new(),
            operational: Default::default(),
            parameters: if borrowed { vec![point()] } else { Vec::new() },
            borrowed: if borrowed {
                vec![Borrowed {
                    position: 0,
                    access: ReferenceAccess::Mutable,
                    referent: crate::lowering::BorrowedReferent::Value,
                    home: None,
                    regions: Default::default(),
                }]
            } else {
                Vec::new()
            },
            erased: Vec::new(),
            result: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            crashes: CrashCeiling::Authored(trap_when(vec![terminal_psi::CrashRouteGuard::Truth])),
            termination: Termination::Silent,
            ranking: None,
            reaches: crate::lowering::Ceiling::Inferred,
            invokes: crate::lowering::Ceiling::Inferred,
        });
    }
    let mut elaboration =
        crate::lowering::lower_bodies(id(1), &bodies, &signatures, &affine_records()).unwrap();
    declare_body_models(&mut elaboration, &signatures);
    let caller = &elaboration.module.machines[0];
    assert!(
        caller
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Terminator::InvokeNoNormal { .. }))
    );
    assert!(caller.blocks.iter().all(|block| {
        block
            .terminator
            .cleanup_rosters()
            .all(|(_, actions)| actions.is_empty())
    }));
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}
