//! Exact contract custody across every control-flow cleanup rule.

use super::*;

#[derive(Clone, Copy)]
enum Validator {
    ConstantConditional,
    LinearEmptyBlock,
    PathQualifiedEmptyBlock,
    AdjacentBlockMerge,
    SharedJumpFusion,
    UnreachablePrivateMachines,
    NonAdjacentBlockMerge,
}

fn propose_one(unit: &PsiOptimizationUnit, rule: &dyn PsiOptimizationRule) -> PsiRewriteCandidate {
    let contract = rule.contract();
    let mut manager = crate::AnalysisManager::new(unit);
    let products = manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    rule.propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            panic!(
                "contract fixture must produce its named rule: {:?}",
                contract.identity()
            )
        })
}

fn relabel(
    unit: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    predicted_cost_delta: i64,
) -> PsiRewriteCandidate {
    let affected = candidate.affected_blocks().to_vec();
    let substitutions = candidate.substitutions().to_vec();
    let provenance = candidate.provenance().to_vec();
    match candidate.patch() {
        PsiRewritePatch::FoldConstantConditional(patch) => {
            PsiRewriteCandidate::new_constant_conditional(
                unit.identity,
                contract,
                affected,
                provenance,
                candidate
                    .scalar_evaluation_witness()
                    .and_then(IntegerEvaluationWitness::unary_operand)
                    .unwrap(),
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::ThreadLinearEmptyBlock(patch) => {
            PsiRewriteCandidate::new_linear_empty_block(
                unit.identity,
                contract,
                affected,
                provenance,
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) => {
            PsiRewriteCandidate::new_path_qualified_empty_block(
                unit.identity,
                contract,
                affected,
                provenance,
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::MergeAdjacentBlock(patch) => {
            PsiRewriteCandidate::new_adjacent_block_merge(
                unit.identity,
                contract,
                affected,
                substitutions,
                provenance,
                candidate.ownership_frontier_witness().unwrap().clone(),
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::FuseSharedTerminalJump(patch) => {
            PsiRewriteCandidate::new_shared_jump_fusion(
                unit.identity,
                contract,
                affected,
                substitutions,
                provenance,
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::PruneUnreachablePrivateMachines(patch) => {
            PsiRewriteCandidate::new_unreachable_private_machines(
                unit.identity,
                contract,
                provenance,
                predicted_cost_delta,
                patch,
            )
        }
        PsiRewritePatch::MergeNonAdjacentBlock(patch) => {
            PsiRewriteCandidate::new_non_adjacent_block_merge(
                unit.identity,
                contract,
                affected,
                substitutions,
                provenance,
                predicted_cost_delta,
                patch,
            )
        }
        patch => panic!("unexpected control-flow contract fixture patch: {patch:?}"),
    }
    .unwrap()
}

fn rejects_contract(
    validator: Validator,
    unit: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) {
    let error = match validator {
        Validator::ConstantConditional => {
            validate_constant_conditional_candidate(unit, candidate).unwrap_err()
        }
        Validator::LinearEmptyBlock => {
            validate_linear_empty_block_candidate(unit, candidate).unwrap_err()
        }
        Validator::PathQualifiedEmptyBlock => {
            validate_path_qualified_empty_block_candidate(unit, candidate).unwrap_err()
        }
        Validator::AdjacentBlockMerge => {
            validate_adjacent_block_merge_candidate(unit, candidate).unwrap_err()
        }
        Validator::SharedJumpFusion => {
            validate_shared_jump_fusion_candidate(unit, candidate).unwrap_err()
        }
        Validator::UnreachablePrivateMachines => {
            validate_unreachable_private_machines_candidate(unit, candidate).unwrap_err()
        }
        Validator::NonAdjacentBlockMerge => {
            validate_non_adjacent_block_merge_candidate(unit, candidate).unwrap_err()
        }
    };
    assert_eq!(
        error,
        OptimizationUnitValidationError::CandidateAnalysisContractMismatch
    );
}

#[test]
fn every_control_flow_validator_rejects_cross_rule_unknown_superset_and_cost_relabels() {
    let contracts = [
        ConstantConditionalFoldRule::contract(),
        LinearEmptyBlockThreadRule::contract(),
        PathQualifiedEmptyBlockThreadRule::contract(),
        AdjacentBlockMergeRule::contract(),
        SharedJumpFusionRule::contract(),
        UnreachablePrivateMachinePruneRule::contract(),
        NonAdjacentBlockMergeRule::contract(),
    ];

    let constant_unit = constant_conditional_same_target_unit(true);
    let linear_unit = linear_empty_block_unit();
    let path_unit = path_qualified_empty_block_unit();
    let adjacent_unit = linear_empty_block_unit();
    let shared_unit = shared_terminal_unit();
    let mut unreachable_unit = linear_empty_block_unit();
    let mut private = unreachable_unit.functions[0].clone();
    private.machine = MachineId::new(99).unwrap();
    unreachable_unit.functions.push(private);
    unreachable_unit.identity = recompute_psi_optimization_unit_identity(&unreachable_unit);
    let non_adjacent_unit = non_adjacent_merge_unit(false);

    let cases = [
        (
            constant_unit.clone(),
            propose_one(&constant_unit, &ConstantConditionalFoldRule),
            Validator::ConstantConditional,
        ),
        (
            linear_unit.clone(),
            propose_one(&linear_unit, &LinearEmptyBlockThreadRule),
            Validator::LinearEmptyBlock,
        ),
        (
            path_unit.clone(),
            propose_one(&path_unit, &PathQualifiedEmptyBlockThreadRule),
            Validator::PathQualifiedEmptyBlock,
        ),
        (
            adjacent_unit.clone(),
            propose_one(&adjacent_unit, &AdjacentBlockMergeRule),
            Validator::AdjacentBlockMerge,
        ),
        (
            shared_unit.clone(),
            propose_one(&shared_unit, &SharedJumpFusionRule),
            Validator::SharedJumpFusion,
        ),
        (
            unreachable_unit.clone(),
            propose_one(&unreachable_unit, &UnreachablePrivateMachinePruneRule),
            Validator::UnreachablePrivateMachines,
        ),
        (
            non_adjacent_unit.clone(),
            propose_one(&non_adjacent_unit, &NonAdjacentBlockMergeRule),
            Validator::NonAdjacentBlockMerge,
        ),
    ];

    for (index, (unit, candidate, validator)) in cases.iter().enumerate() {
        for (contract_index, contract) in contracts.iter().copied().enumerate() {
            if contract_index == index {
                continue;
            }
            rejects_contract(
                *validator,
                unit,
                &relabel(unit, candidate, contract, candidate.predicted_cost_delta()),
            );
        }

        let base = contracts[index];
        let corrupt_contracts = [
            OptimizationRuleContract::new(
                OptimizationRuleIdentity::from_canonical_bytes(
                    b"omega.psi-rule.unknown-control-flow-cleanup.v1",
                ),
                base.pass(),
                base.version(),
                base.required_analyses(),
                base.invalidated_analyses(),
                base.safety_class(),
            )
            .unwrap(),
            OptimizationRuleContract::new(
                base.identity(),
                base.pass(),
                base.version(),
                omega_optimization_core::AnalysisSet::new(
                    base.required_analyses()
                        .iter()
                        .chain([AnalysisKind::ValueRanges]),
                ),
                base.invalidated_analyses(),
                base.safety_class(),
            )
            .unwrap(),
            OptimizationRuleContract::new(
                base.identity(),
                base.pass(),
                base.version(),
                base.required_analyses(),
                omega_optimization_core::AnalysisInvalidationSet::new(
                    base.invalidated_analyses()
                        .iter()
                        .chain([AnalysisKind::Dominators]),
                ),
                base.safety_class(),
            )
            .unwrap(),
            OptimizationRuleContract::new(
                base.identity(),
                base.pass(),
                base.version(),
                base.required_analyses(),
                base.invalidated_analyses(),
                match base.safety_class() {
                    OptimizationSafetyClass::ExactOperationSemantics => {
                        OptimizationSafetyClass::StructuralIdentity
                    }
                    _ => OptimizationSafetyClass::ExactOperationSemantics,
                },
            )
            .unwrap(),
        ];
        for contract in corrupt_contracts {
            rejects_contract(
                *validator,
                unit,
                &relabel(unit, candidate, contract, candidate.predicted_cost_delta()),
            );
        }
        rejects_contract(*validator, unit, &relabel(unit, candidate, base, 0));
    }
}
