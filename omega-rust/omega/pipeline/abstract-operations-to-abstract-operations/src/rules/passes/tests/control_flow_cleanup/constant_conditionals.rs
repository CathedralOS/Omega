//! Constant-conditional folding, pruning, and corruption rejection.

use super::*;

#[test]
fn constant_conditional_fold_binds_selected_edge_fact_and_fuel() {
    for constant in [false, true] {
        let unit = constant_conditional_same_target_unit(constant);
        let contract = ConstantConditionalFoldRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = ConstantConditionalFoldRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].consumed_facts().len(), 1);
        let optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
            candidates[0].patch()
        else {
            unreachable!()
        };
        assert_eq!(patch.constant, constant);
        let realized = candidates[0]
            .provenance()
            .iter()
            .find(|row| row.disposition.is_realized())
            .expect("conditional fold carries selected-edge custody");
        let proven_unreachable = candidates[0]
            .provenance()
            .iter()
            .find(|row| !row.disposition.is_realized())
            .expect("conditional fold carries rejected-edge custody");
        let realized_site = PsiRealizationSite::Edge {
            machine: patch.location.machine,
            edge: patch.selected_edge,
        };
        let unreachable_site = PsiRealizationSite::Edge {
            machine: patch.location.machine,
            edge: patch.rejected_edge,
        };
        assert_eq!(
            realized.disposition,
            ProvenanceDisposition::RealizedAt(realized_site)
        );
        assert_eq!(
            realized.sources,
            [optimization_unit::PsiProvenance::Edge(patch.selected_edge)]
        );
        assert_eq!(
            proven_unreachable.disposition,
            ProvenanceDisposition::ProvenUnreachableAt(unreachable_site)
        );
        assert_eq!(
            proven_unreachable.sources,
            [optimization_unit::PsiProvenance::Edge(patch.rejected_edge)]
        );
        let accepted = validate_constant_conditional_candidate(&unit, &candidates[0]).unwrap();
        assert_eq!(accepted.provenance(), candidates[0].provenance());
        assert_eq!(
            accepted.validator(),
            optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.constant-conditional-fold.v4"
            )
        );
        let node = &accepted.unit().functions[0].blocks[0].nodes[1];
        assert!(matches!(
            node.operation,
            AbstractOperation::Jump { psi_edge, .. } if psi_edge == patch.selected_edge
        ));
        assert_eq!(
            node.successors[0].provenance,
            [optimization_unit::PsiProvenance::Edge(patch.selected_edge)]
        );
        assert!(node.provenance.is_empty());
        assert!(node.fuel.is_empty());
        assert_eq!(node.successors[0].fuel.len(), 1);
        assert_eq!(
            node.successors[0].fuel[0].site,
            optimization_unit::PsiProvenance::Edge(patch.selected_edge)
        );
    }
}

#[test]
fn constant_conditional_fold_atomically_prunes_the_unreachable_branch_region() {
    let unit = propagated_block_parameter_unit(true);
    let contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = ConstantConditionalFoldRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .expect("constant branch produces an atomic prune candidate");
    assert_eq!(
        candidate.affected_blocks(),
        [
            id(602, BlockId::new),
            id(604, BlockId::new),
            id(605, BlockId::new),
        ]
    );
    assert_eq!(
        candidate
            .provenance()
            .iter()
            .filter(|row| row.disposition.is_realized())
            .count(),
        3
    );
    assert_eq!(
        candidate
            .provenance()
            .iter()
            .filter(|row| !row.disposition.is_realized())
            .count(),
        3
    );
    let accepted = validate_constant_conditional_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(
        output.functions[0]
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        [
            id(602, BlockId::new),
            id(603, BlockId::new),
            id(605, BlockId::new),
        ]
    );
    assert_eq!(output.functions[0].facts.len(), 2);
    assert_eq!(output.functions[0].blocks[2].nodes[0].effect.input, 4);
    assert_eq!(output.functions[0].blocks[2].nodes[1].effect.output, 6);
    assert_eq!(accepted.provenance(), candidate.provenance());
}

#[test]
fn constant_conditional_fold_atomically_refreshes_current_root_service_reach() {
    let unit = constant_conditional_dead_service_unit();
    validate_psi_optimization_unit(&unit)
        .expect("dead-branch service belongs to the source revision root reach");
    assert_eq!(unit.root_service_reach.concrete, [id(620, ServiceId::new)]);
    let contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = ConstantConditionalFoldRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .expect("constant branch produces an atomic prune candidate");
    let accepted = validate_constant_conditional_candidate(&unit, &candidate)
        .expect("accepted fold refreshes its derived root reach");
    assert!(accepted.unit().root_service_reach.concrete.is_empty());
    assert!(
        accepted
            .unit()
            .root_service_reach
            .installation_dependencies
            .is_empty()
    );
    validate_psi_optimization_unit(accepted.unit())
        .expect("fold output has exact current-revision root reach");
}

#[test]
fn constant_conditional_pruning_is_symmetric_and_rebases_all_later_blocks() {
    let unit = propagated_block_parameter_unit(false);
    let contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = ConstantConditionalFoldRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(
        candidate.affected_blocks(),
        [
            id(602, BlockId::new),
            id(603, BlockId::new),
            id(604, BlockId::new),
            id(605, BlockId::new),
        ]
    );
    assert_eq!(
        candidate
            .provenance()
            .iter()
            .filter(|row| row.disposition.is_realized())
            .count(),
        4
    );
    assert_eq!(
        candidate
            .provenance()
            .iter()
            .filter(|row| !row.disposition.is_realized())
            .count(),
        3
    );
    let accepted = validate_constant_conditional_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(
        output.functions[0]
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        [
            id(602, BlockId::new),
            id(604, BlockId::new),
            id(605, BlockId::new),
        ]
    );
    assert_eq!(output.functions[0].facts.len(), 2);
    assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 2);
    assert_eq!(output.functions[0].blocks[2].nodes[1].effect.output, 6);
}

#[test]
fn constant_conditional_validator_rejects_edge_and_fuel_corruption() {
    let unit = constant_conditional_same_target_unit(true);
    let contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = ConstantConditionalFoldRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) = candidate.patch()
    else {
        unreachable!()
    };
    let condition_fact = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
        .unwrap();
    assert!(matches!(
        PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.provenance()[..1].to_vec(),
            condition_fact,
            -1,
            patch,
        ),
        Err(optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
    ));

    let mut duplicate_source = candidate.provenance().to_vec();
    let source = duplicate_source[0].sources[0];
    let fuel = duplicate_source[0].fuel[0];
    duplicate_source[0].sources.push(source);
    duplicate_source[0].fuel.push(fuel);
    assert!(matches!(
        PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            duplicate_source,
            condition_fact,
            -1,
            patch,
        ),
        Err(optimization_unit::PsiRewriteCandidateError::NonCanonicalProvenance)
    ));

    let mut zero_fuel = candidate.provenance().to_vec();
    zero_fuel[1].fuel[0].units = 0;
    assert!(matches!(
        PsiRewriteCandidate::new_constant_conditional(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            zero_fuel,
            condition_fact,
            -1,
            patch,
        ),
        Err(optimization_unit::PsiRewriteCandidateError::FuelProvenanceMismatch)
    ));

    let selected_site = PsiRealizationSite::Edge {
        machine: patch.location.machine,
        edge: patch.selected_edge,
    };
    let rejected_site = PsiRealizationSite::Edge {
        machine: patch.location.machine,
        edge: patch.rejected_edge,
    };
    let mut swapped_provenance = candidate.provenance().to_vec();
    for row in &mut swapped_provenance {
        if row.input == selected_site {
            row.disposition = ProvenanceDisposition::ProvenUnreachableAt(selected_site);
        } else if row.input == rejected_site {
            row.disposition = ProvenanceDisposition::RealizedAt(rejected_site);
        }
    }
    let swapped = PsiRewriteCandidate::new_constant_conditional(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        swapped_provenance,
        condition_fact,
        -1,
        ConstantConditionalRewrite {
            selected_edge: patch.rejected_edge,
            rejected_edge: patch.selected_edge,
            ..patch
        },
    )
    .unwrap();
    assert!(matches!(
        validate_constant_conditional_candidate(&unit, &swapped),
        Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
    ));

    let mut provenance = candidate.provenance().to_vec();
    provenance[0].fuel[0].units += 1;
    let wrong_fuel = PsiRewriteCandidate::new_constant_conditional(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        condition_fact,
        -1,
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_constant_conditional_candidate(&unit, &wrong_fuel),
        Err(OptimizationUnitValidationError::CandidateFuelMismatch)
    ));

    let mut provenance = candidate.provenance().to_vec();
    provenance[1].fuel[0].units += 1;
    let wrong_unreachable_fuel = PsiRewriteCandidate::new_constant_conditional(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        condition_fact,
        -1,
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_constant_conditional_candidate(&unit, &wrong_unreachable_fuel),
        Err(OptimizationUnitValidationError::CandidateFuelMismatch)
    ));
}

#[test]
fn constant_conditional_validator_rejects_incomplete_prune_custody_and_region() {
    let unit = propagated_block_parameter_unit(true);
    let contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = ConstantConditionalFoldRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) = candidate.patch()
    else {
        unreachable!()
    };
    let condition_fact = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::unary_operand)
        .unwrap();
    let dead_block = id(604, BlockId::new);
    let rebased_merge = id(605, BlockId::new);

    let mut incomplete_provenance = candidate.provenance().to_vec();
    let removed = incomplete_provenance
        .iter()
        .position(|row| {
            !row.disposition.is_realized()
                && matches!(
                    row.disposition.site(),
                    PsiRealizationSite::Node(location) if location.block == dead_block
                )
        })
        .expect("dead nodes carry unreachable custody");
    incomplete_provenance.remove(removed);
    let incomplete_custody = PsiRewriteCandidate::new_constant_conditional(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        incomplete_provenance,
        condition_fact,
        -1,
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_constant_conditional_candidate(&unit, &incomplete_custody),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    ));

    let incomplete_region = PsiRewriteCandidate::new_constant_conditional(
        unit.identity,
        contract,
        candidate
            .affected_blocks()
            .iter()
            .copied()
            .filter(|block| *block != rebased_merge)
            .collect(),
        candidate.provenance().to_vec(),
        condition_fact,
        -1,
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_constant_conditional_candidate(&unit, &incomplete_region),
        Err(OptimizationUnitValidationError::CandidateReachabilityMismatch)
    ));
}
