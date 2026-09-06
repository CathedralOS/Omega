//! Forged cleanup and transfer inputs must fail independent ownership replay.

use abstract_operations::AbstractOperation;
use terminal_psi::{StructuralAccess, TerminalAffineCleanupAction};

use super::fixtures::*;
use crate::tests::refresh_function_derivatives;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

#[test]
fn general_partial_affine_rejects_forged_residuals_after_metadata_refresh() {
    let baseline = mixed_unit();
    validate_psi_optimization_unit(&baseline).unwrap();
    for mutation in 0..9 {
        let mut changed = baseline.clone();
        let root = changed.functions[0].structural_parameters[0].place;
        let root_type = changed.functions[0].structural_parameters[0].structural_type;
        let other_place = changed.functions[1].structural_parameters[0].place;
        let actions = cleanup_actions(&mut changed);
        match mutation {
            0 => {
                actions.pop();
            }
            1 => actions.reverse(),
            2 => actions.push(actions[0].clone()),
            3 => {
                let TerminalAffineCleanupAction::DiscardResidual(residual) = &mut actions[0] else {
                    unreachable!()
                };
                residual.place = other_place;
            }
            4 => {
                let TerminalAffineCleanupAction::DiscardResidual(residual) = &mut actions[0] else {
                    unreachable!()
                };
                residual.structural_type = root_type;
            }
            5 => {
                let TerminalAffineCleanupAction::DiscardResidual(residual) = &mut actions[0] else {
                    unreachable!()
                };
                residual.path = vec![field("rows"), index(0), field("left")];
            }
            6 => actions.insert(0, TerminalAffineCleanupAction::DiscardRoot(root)),
            7 => {
                // Expanding a live row to leaves preserves coverage but loses
                // the canonical maximal-subtree evidence.
                let TerminalAffineCleanupAction::DiscardResidual(mut leaf) = actions.remove(2)
                else {
                    unreachable!()
                };
                let TerminalAffineCleanupAction::DiscardResidual(token) = &actions[0] else {
                    unreachable!()
                };
                leaf.structural_type = token.structural_type;
                leaf.path.push(field("right"));
                actions.insert(
                    2,
                    TerminalAffineCleanupAction::DiscardResidual(leaf.clone()),
                );
                *leaf.path.last_mut().unwrap() = field("left");
                actions.insert(3, TerminalAffineCleanupAction::DiscardResidual(leaf));
            }
            8 => {
                actions.clear();
                actions.push(TerminalAffineCleanupAction::DiscardRoot(root));
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        let result = validate_psi_optimization_unit(&changed);
        let rejected = if mutation == 3 {
            // Another function's place is rejected before cleanup replay.
            matches!(
                result,
                Err(OptimizationUnitValidationError::UnknownPlace { .. })
            )
        } else {
            matches!(
                result,
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            )
        };
        assert!(rejected, "forged residual mutation {mutation}: {result:?}");
    }
}

#[test]
fn general_partial_affine_rejects_forged_empty_complements() {
    let mut missing = mixed_unit();
    cleanup_actions(&mut missing).clear();
    refresh_function_derivatives(&mut missing, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing),
        Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
    ));
    let moves = (0..5)
        .map(|value| (vec![index(value)], 1))
        .collect::<Vec<_>>();
    let baseline = unit(vec![record(1, &[]), array(2, 1, 5)], 2, &moves, &[]);
    validate_psi_optimization_unit(&baseline).unwrap();
    for residual in [false, true] {
        let mut changed = baseline.clone();
        let parameter = changed.functions[0].structural_parameters[0].clone();
        let token_type = changed.functions[1].structural_parameters[0].structural_type;
        cleanup_actions(&mut changed).push(if residual {
            TerminalAffineCleanupAction::DiscardResidual(terminal_psi::StructuralAffineDiscard {
                place: parameter.place,
                path: vec![index(4)],
                structural_type: token_type,
            })
        } else {
            TerminalAffineCleanupAction::DiscardRoot(parameter.place)
        });
        refresh_function_derivatives(&mut changed, 0);
        assert!(matches!(
            validate_psi_optimization_unit(&changed),
            Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
        ));
    }
}

#[test]
fn general_partial_affine_rejects_huge_dimensions_before_enumeration() {
    for length in [u64::MAX, u64::MAX / 2, 1 << 40] {
        for residuals in [vec![], vec![(vec![index(0)], 1)]] {
            assert!(matches!(
                validate_psi_optimization_unit(&unit(
                    vec![record(1, &[]), array(2, 1, length)],
                    2,
                    &[(vec![index(length - 1)], 1)],
                    &residuals,
                )),
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ));
        }
        assert!(
            matches!(
                validate_psi_optimization_unit(&unit(
                    vec![record(1, &[]), array(2, 1, length), array(3, 2, 2)],
                    3,
                    &[(vec![index(0), index(0)], 1)],
                    &[(vec![index(1)], 2)],
                )),
                Err(OptimizationUnitValidationError::CurrentCleanupMismatch { .. })
            ),
            "an emitted outer residual leaves zero budget for the huge inner row"
        );
    }
}

#[test]
fn general_partial_affine_rejects_overlapping_and_whole_root_moves() {
    let types = vec![record(1, &[]), array(2, 1, 3), array(3, 2, 2)];
    for moves in [
        vec![(vec![index(0)], 2), (vec![index(0), index(1)], 1)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0)], 2)],
        vec![(vec![index(0), index(1)], 1), (vec![index(0), index(1)], 1)],
    ] {
        assert!(matches!(
            validate_psi_optimization_unit(&unit(types.clone(), 3, &moves, &[])),
            Err(OptimizationUnitValidationError::CurrentProjectedMoveOverlap { .. })
        ));
    }
    assert!(matches!(
        validate_psi_optimization_unit(&unit(
            types,
            3,
            &[(vec![index(0), index(1)], 1), (vec![], 3)],
            &[],
        )),
        Err(OptimizationUnitValidationError::CurrentWholePlacePartiallyMoved { .. })
    ));
}

#[test]
fn general_partial_affine_rejects_source_type_access_and_path_drift() {
    let baseline = mixed_unit();
    for mutation in 0..5 {
        let mut changed = baseline.clone();
        let other_place = changed.functions[1].structural_parameters[0].place;
        let AbstractOperation::CallUnit {
            structural_arguments,
            ..
        } = &mut changed.functions[0].blocks[0].nodes[0].operation
        else {
            unreachable!()
        };
        match mutation {
            0 => structural_arguments[0].place = other_place,
            1 => structural_arguments[0].access = StructuralAccess::SharedBorrow,
            2 => structural_arguments[0].path[1] = index(3),
            3 => structural_arguments[0].path[2] = field("absent"),
            4 => {
                structural_arguments[0].path.pop();
            }
            _ => unreachable!(),
        }
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "call mutation {mutation}"
        );
    }
}
