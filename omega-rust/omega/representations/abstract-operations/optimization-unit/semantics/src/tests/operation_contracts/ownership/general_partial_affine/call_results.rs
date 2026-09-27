//! Result roots retain producer identity and the exact partial ownership frontier.

use super::fixtures::{array, call_result_unit, cleanup_actions, field, index, record};
use crate::tests::fixtures::dominance::refresh_function_derivatives;
use optimization_unit::PsiOptimizationUnit;
use terminal_psi::TerminalAffineCleanupAction;

use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

fn mixed_result() -> PsiOptimizationUnit {
    call_result_unit(
        vec![
            record(1, &[]),
            array(2, 1, 3),
            array(3, 2, 2),
            record(4, &[("left", 1), ("grid", 3), ("tail", 1)]),
        ],
        4,
        &[
            (vec![field("grid"), index(1)], 2),
            (vec![field("grid"), index(0), index(1)], 1),
        ],
        &[
            (vec![field("tail")], 1),
            (vec![field("grid"), index(0), index(2)], 1),
            (vec![field("grid"), index(0), index(0)], 1),
            (vec![field("left")], 1),
        ],
    )
}

#[test]
fn call_result_partial_affine_rejects_residual_custody_drift() {
    let baseline = mixed_result();
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..7 {
        let mut changed = baseline.clone();
        let input = changed.functions[0].structural_parameters[0].place;
        let actions = cleanup_actions(&mut changed);
        let TerminalAffineCleanupAction::DiscardResidual(first) = &actions[0] else {
            unreachable!()
        };
        let root = first.place;
        match mutation {
            0 => {
                actions.pop();
            }
            1 => actions.reverse(),
            2 => actions.push(actions[0].clone()),
            3 => {
                let TerminalAffineCleanupAction::DiscardResidual(first) = &mut actions[0] else {
                    unreachable!()
                };
                first.place = input;
            }
            4 => {
                let TerminalAffineCleanupAction::DiscardResidual(first) = &mut actions[0] else {
                    unreachable!()
                };
                first.path = vec![field("grid"), index(1)];
            }
            5 => actions.insert(0, TerminalAffineCleanupAction::DiscardRoot(root)),
            6 => actions.clear(),
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        assert!(
            matches!(
                result,
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ),
            "residual mutation {mutation}: {result:?}"
        );
    }
}

#[test]
fn call_result_partial_affine_rejects_overlapping_moves() {
    for moves in [
        vec![(vec![index(0)], 2), (vec![index(0), index(1)], 1)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0)], 2)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0), index(1)], 1)],
    ] {
        let result = validate_psi_optimization_unit(&call_result_unit(
            vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)],
            3,
            &moves,
            &[],
        ));
        assert!(
            matches!(
                result,
                Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { .. })
            ),
            "{result:?}"
        );
    }
    let result = validate_psi_optimization_unit(&call_result_unit(
        vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)],
        3,
        &[(vec![index(0), index(1)], 1), (vec![], 3)],
        &[],
    ));
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved { .. })
        ),
        "{result:?}"
    );
}
