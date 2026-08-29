//! Shared terminal-jump fusion and exact custody.

use super::*;

#[test]
fn shared_terminal_jump_fusion_clones_one_path_and_retains_exact_custody() {
    let threaded = shared_terminal_unit();
    let contract = SharedJumpFusionRule::contract();
    let mut manager = crate::AnalysisManager::new(&threaded);
    let products = manager
        .require_all(&threaded, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let AnalysisProduct::PostDominators(post_dominators) = products
        .iter()
        .find(|product| product.kind() == AnalysisKind::PostDominators)
        .expect("shared-terminal fusion requires post-dominators")
    else {
        unreachable!()
    };
    let function_post_dominators = &post_dominators.functions[0].1;
    for predecessor in [id(923, BlockId::new), id(924, BlockId::new)] {
        assert!(
            function_post_dominators
                .iter()
                .find(|(block, _)| *block == predecessor)
                .unwrap()
                .1
                .contains(&id(926, BlockId::new))
        );
    }
    let without_post_dominators = products
        .iter()
        .filter(|product| product.kind() != AnalysisKind::PostDominators)
        .cloned()
        .collect::<Vec<_>>();
    assert!(matches!(
        SharedJumpFusionRule.propose(&threaded, RuleAnalysisView::new(&without_post_dominators),),
        Err(RuleProposalError::MissingAnalysis(
            AnalysisKind::PostDominators
        ))
    ));
    let candidates = SharedJumpFusionRule
        .propose(&threaded, RuleAnalysisView::new(&products))
        .unwrap();
    assert_eq!(candidates.len(), 2);
    let candidate = candidates
        .iter()
        .find(|candidate| {
            matches!(
                candidate.patch(),
                PsiRewritePatch::FuseSharedTerminalJump(patch)
                    if patch.predecessor.block == id(923, BlockId::new)
            )
        })
        .expect("left incoming path has an exact fusion candidate");
    let target_before = threaded.functions[0]
        .blocks
        .iter()
        .find(|block| block.id == id(926, BlockId::new))
        .unwrap()
        .clone();
    let accepted = validate_shared_jump_fusion_candidate(&threaded, candidate).unwrap();
    let output = accepted.unit();
    let clone = &output.functions[0]
        .blocks
        .iter()
        .find(|block| block.id == id(923, BlockId::new))
        .unwrap()
        .nodes[0];
    assert!(matches!(clone.operation, O::ReturnUnit { .. }));
    assert_eq!(
        clone.provenance,
        [
            PsiProvenance::Edge(id(936, EdgeId::new)),
            PsiProvenance::Edge(id(933, EdgeId::new)),
        ]
    );
    assert_eq!(
        output.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == id(926, BlockId::new))
            .unwrap(),
        &target_before
    );
    let terminal_input = PsiRealizationSite::Node(NodeLocation {
        machine: id(921, MachineId::new),
        block: id(926, BlockId::new),
        node: 0,
    });
    assert_eq!(
        accepted
            .provenance()
            .iter()
            .filter(|row| row.input == terminal_input)
            .count(),
        2
    );

    let mut nonterminal_duplicate = output.clone();
    let duplicated = PsiProvenance::Edge(id(936, EdgeId::new));
    let nonterminal = &mut nonterminal_duplicate.functions[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == id(923, BlockId::new))
        .unwrap()
        .nodes[0];
    nonterminal.provenance.push(duplicated);
    nonterminal
        .fuel
        .push(omega_optimization_unit::FuelSettlement {
            site: duplicated,
            units: 1,
        });
    nonterminal_duplicate.identity =
        recompute_psi_optimization_unit_identity(&nonterminal_duplicate);
    assert_eq!(
        validate_psi_optimization_unit(&nonterminal_duplicate),
        Err(OptimizationUnitValidationError::DuplicateProvenance(
            duplicated
        ))
    );

    let PsiRewritePatch::FuseSharedTerminalJump(patch) = candidate.patch() else {
        unreachable!()
    };
    let mut incomplete = candidate.provenance().to_vec();
    incomplete
        .retain(|row| row.input != terminal_input || row.disposition.site() != terminal_input);
    let forged = PsiRewriteCandidate::new_shared_jump_fusion(
        threaded.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.substitutions().to_vec(),
        incomplete,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_shared_jump_fusion_candidate(&threaded, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );

    let legacy_contract = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(
            b"omega.psi-rule.shared-terminal-jump-fusion.v1",
        ),
        contract.pass(),
        1,
        omega_optimization_core::AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::OwnershipFrontiers,
        ]),
        contract.invalidated_analyses(),
        contract.safety_class(),
    )
    .unwrap();
    let legacy_candidate = PsiRewriteCandidate::new_shared_jump_fusion(
        threaded.identity,
        legacy_contract,
        candidate.affected_blocks().to_vec(),
        candidate.substitutions().to_vec(),
        candidate.provenance().to_vec(),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_shared_jump_fusion_candidate(&threaded, &legacy_candidate),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );
}
