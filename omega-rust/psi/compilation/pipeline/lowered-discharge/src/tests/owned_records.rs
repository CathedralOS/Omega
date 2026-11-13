use super::*;
use language_core::ReferenceAccess;
use structure_elaboration::Borrowed;
use terminal_psi::{StructuralMultiplicity, TerminalAffineCleanupAction, Terminator};

fn affine_records() -> IdTable<StructuralTypeId, RecordType> {
    let mut types = records();
    for raw in [1, 2] {
        types.get_mut(id(raw)).unwrap().multiplicity = StructuralMultiplicity::Affine;
    }
    types
}

/// A whole move must preserve the scalar fact used by the bounded call.
/// The negative control borrows the old home after exactly that move.
fn moved_record(read_old_home: bool) -> structure_elaboration::Elaboration {
    elaborate_over(
        vec![
            Shaped {
                parameters: Vec::new(),
                locals: vec![point(), point()],
                result: u32_type().into(),
                requires: Vec::new(),
                statements: vec![
                    bind(0, point_of(unsigned(5), unsigned(7))),
                    bind(1, place(0, point())),
                    returns(call(
                        2,
                        vec![read(place(usize::from(!read_old_home), point()), 0)],
                    )),
                ],
            },
            bounded(),
        ],
        &affine_records(),
    )
}

#[test]
fn a_whole_move_preserves_contents_and_retires_both_homes() {
    let elaboration = moved_record(false);
    assert!(!elaboration.module.structural_types[0].copyable);
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
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, TerminalAffineCleanupAction::RetireHome(_)))
    );
    assert!(matches!(
        judge(Ok(elaboration)).unwrap(),
        RouteVerdict::Accepted(_)
    ));
}

#[test]
fn the_old_home_cannot_be_read_after_a_whole_move() {
    let verdict = judge(Ok(moved_record(true))).unwrap();
    let RouteVerdict::Rejected { unestablished, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert!(
        unestablished
            .iter()
            .any(|failure| failure.detail.contains("after it moved"))
    );
}

#[test]
fn receiving_refuses_a_missing_affine_discard() {
    let mut elaboration = moved_record(false);
    for block in &mut elaboration.module.machines[0].blocks {
        if let Terminator::Return {
            cleanup_actions, ..
        } = &mut block.terminator
        {
            cleanup_actions
                .retain(|action| !matches!(action, TerminalAffineCleanupAction::DiscardRoot(_)));
        }
    }
    let refusal = judge(Ok(elaboration)).unwrap_err();
    assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
    assert!(refusal.detail.contains("normal cleanup"));
}

#[test]
fn a_copy_parent_cannot_erase_its_affine_child() {
    let mut elaboration = moved_record(false);
    elaboration.module.structural_types[1].copyable = true;
    let verdict = judge(Ok(elaboration)).unwrap();
    let RouteVerdict::Rejected { unestablished, .. } = verdict else {
        panic!("{verdict:?}");
    };
    assert!(unestablished.iter().any(|failure| {
        failure
            .detail
            .contains("copy data contains non-copyable storage")
    }));
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
            Statement::Evaluate(Expression {
                kind: ExpressionKind::Call {
                    callee: id(2),
                    arguments: vec![borrowed],
                },
                result: None,
                source: nowhere(),
            }),
        ],
    });
    bodies.push(Body {
        slots: vec![point()],
        statements: vec![
            Statement::Evaluate(read(place(0, point()), 0)),
            Statement::Arm {
                guard: None,
                target: Target::Crash(semantic_vocabulary::CrashCause::Trap),
                source: nowhere(),
            },
        ],
    });
    for borrowed in [false, true] {
        signatures.push(Signature {
            parameters: if borrowed { vec![point()] } else { Vec::new() },
            borrowed: if borrowed {
                vec![Borrowed {
                    position: 0,
                    access: ReferenceAccess::Mutable,
                    home: None,
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
        });
    }
    let elaboration =
        structure_elaboration::lower_bodies(id(1), &bodies, &signatures, &affine_records())
            .unwrap();
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
