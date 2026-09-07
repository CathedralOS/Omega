//! Residual transactions cannot disappear through scalar control rewrites.

use super::*;

#[test]
fn control_rewrites_do_not_offer_to_erase_residual_edge_cleanup() {
    let cases: Vec<(PsiOptimizationUnit, Box<dyn PsiOptimizationRule>)> = vec![
        (
            linear_empty_block_unit(),
            Box::new(LinearEmptyBlockThreadRule),
        ),
        (
            path_qualified_empty_block_unit(),
            Box::new(PathQualifiedEmptyBlockThreadRule),
        ),
        (linear_empty_block_unit(), Box::new(AdjacentBlockMergeRule)),
        (
            non_adjacent_merge_unit(false),
            Box::new(NonAdjacentBlockMergeRule),
        ),
        (shared_terminal_unit(), Box::new(SharedJumpFusionRule)),
    ];
    for (mut unit, rule) in cases {
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, rule.contract().required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            !rule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty()
        );

        // Hostile operation mutation against otherwise favorable analysis:
        // proposal must refuse to erase this transaction before relying on the
        // original scalar-only ownership facts. This is not an admitted program.
        for node in unit
            .functions
            .iter_mut()
            .flat_map(|function| &mut function.blocks)
            .flat_map(|block| &mut block.nodes)
        {
            if let O::Jump {
                residual_affine_discards,
                ..
            } = &mut node.operation
            {
                *residual_affine_discards = vec![terminal_psi::StructuralAffineDiscard {
                    place: id(9_801, PlaceId::new),
                    path: vec![terminal_psi::StructuralPathSegment::Field(
                        "remaining".into(),
                    )],
                    structural_type: id(9_802, StructuralTypeId::new),
                }];
                for edge in &mut node.successors {
                    edge.residual_affine_discards = residual_affine_discards.clone();
                }
            }
        }
        assert!(
            rule.propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .is_empty(),
            "rule {:?} must retain residual cleanup on its own edge",
            rule.contract().identity()
        );
    }
}
