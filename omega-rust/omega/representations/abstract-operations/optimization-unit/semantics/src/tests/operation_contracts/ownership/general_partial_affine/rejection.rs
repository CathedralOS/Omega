//! Forged cleanup and transfer inputs must fail independent ownership replay.

use terminal_psi::TerminalAffineCleanupAction;

use super::fixtures::{cleanup_actions, field, index, mixed_unit};
use crate::tests::fixtures::dominance::refresh_function_derivatives;
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
