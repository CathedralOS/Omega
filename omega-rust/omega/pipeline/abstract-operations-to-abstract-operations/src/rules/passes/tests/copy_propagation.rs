//! Copy-propagation tests.

use super::*;

#[test]
fn redundant_block_parameter_rule_binds_both_exact_conditional_edges() {
    let unit = redundant_block_parameter_unit(true);
    let contract = RedundantBlockParameterRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidates = RedundantBlockParameterRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let witness = candidates[0].redundant_block_parameter_witness().unwrap();
    assert_eq!(witness.incoming.len(), 2);
    assert_eq!(witness.incoming[0].source, witness.incoming[1].source);
    assert_ne!(witness.incoming[0].edge, witness.incoming[1].edge);
    assert!(candidates[0].consumed_facts().is_empty());

    let accepted = validate_redundant_block_parameter_candidate(&unit, &candidates[0]).unwrap();
    assert_eq!(
        accepted.validator(),
        optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.redundant-block-parameter.v2"
        )
    );
    let output = accepted.unit();
    assert!(output.functions[0].blocks[1].parameters.is_empty());
    let O::Conditional {
        when_true,
        when_false,
        ..
    } = &output.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    assert!(when_true.bindings.is_empty());
    assert!(when_false.bindings.is_empty());
    let O::ExactIntegerAdd {
        obligation, left, ..
    } = output.functions[0].blocks[1].nodes[0].operation
    else {
        unreachable!()
    };
    assert_eq!(left, unit.functions[0].parameters[1].value);
    assert_eq!(obligation, id(713, ObligationId::new));
    assert_eq!(output.functions[0].facts, unit.functions[0].facts);
    for (before, after) in unit.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .zip(
            output.functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.nodes),
        )
    {
        assert_eq!(after.provenance, before.provenance);
        assert_eq!(after.fuel, before.fuel);
        assert_eq!(after.effect, before.effect);
        assert_eq!(after.ownership, before.ownership);
    }
}

#[test]
fn differing_bindings_decline_and_incomplete_edge_witness_rejects() {
    let unit = redundant_block_parameter_unit(false);
    let contract = RedundantBlockParameterRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        RedundantBlockParameterRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let unit = redundant_block_parameter_unit(true);
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = RedundantBlockParameterRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let optimization_unit::PsiRewritePatch::RemoveRedundantBlockParameter(patch) =
        candidate.patch()
    else {
        unreachable!()
    };
    let incomplete = PsiRewriteCandidate::new_redundant_block_parameter(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.provenance().to_vec(),
        RedundantBlockParameterWitness {
            incoming: candidate
                .redundant_block_parameter_witness()
                .unwrap()
                .incoming[..1]
                .to_vec(),
        },
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_ne!(incomplete.identity(), candidate.identity());
    assert_eq!(
        validate_redundant_block_parameter_candidate(&unit, &incomplete),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );
}

#[test]
fn redundant_block_parameter_validation_rejects_contract_cost_witness_and_provenance_corruption() {
    let unit = redundant_block_parameter_unit(true);
    let base = RedundantBlockParameterRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, base.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = RedundantBlockParameterRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let PsiRewritePatch::RemoveRedundantBlockParameter(patch) = candidate.patch() else {
        unreachable!()
    };
    let forge = |contract, provenance, witness, cost| {
        PsiRewriteCandidate::new_redundant_block_parameter(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            provenance,
            witness,
            cost,
            patch,
        )
        .unwrap()
    };
    let contracts = [
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.unknown-redundant-block-parameter.v1",
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
            optimization_core::AnalysisSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
                AnalysisKind::ScalarConstants,
            ]),
            base.invalidated_analyses(),
            base.safety_class(),
        )
        .unwrap(),
        OptimizationRuleContract::new(
            base.identity(),
            base.pass(),
            base.version(),
            base.required_analyses(),
            optimization_core::AnalysisInvalidationSet::new([
                AnalysisKind::ControlFlowGraph,
                AnalysisKind::UseDefinition,
                AnalysisKind::EffectSummaries,
            ]),
            base.safety_class(),
        )
        .unwrap(),
        OptimizationRuleContract::new(
            base.identity(),
            base.pass(),
            base.version(),
            base.required_analyses(),
            base.invalidated_analyses(),
            OptimizationSafetyClass::ExactOperationSemantics,
        )
        .unwrap(),
    ];
    for contract in contracts {
        let forged = forge(
            contract,
            candidate.provenance().to_vec(),
            candidate
                .redundant_block_parameter_witness()
                .unwrap()
                .clone(),
            candidate.predicted_cost_delta(),
        );
        assert_eq!(
            validate_redundant_block_parameter_candidate(&unit, &forged),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        );
    }

    let wrong_cost = forge(
        base,
        candidate.provenance().to_vec(),
        candidate
            .redundant_block_parameter_witness()
            .unwrap()
            .clone(),
        0,
    );
    assert_eq!(
        validate_redundant_block_parameter_candidate(&unit, &wrong_cost),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    );

    let mut wrong_witness = candidate
        .redundant_block_parameter_witness()
        .unwrap()
        .clone();
    wrong_witness.incoming[0].argument = unit.functions[0].parameters[2].value;
    let wrong_witness = forge(
        base,
        candidate.provenance().to_vec(),
        wrong_witness,
        candidate.predicted_cost_delta(),
    );
    assert_eq!(
        validate_redundant_block_parameter_candidate(&unit, &wrong_witness),
        Err(OptimizationUnitValidationError::CandidateIncomingBindingMismatch)
    );

    let mut incomplete_provenance = candidate.provenance().to_vec();
    incomplete_provenance.pop();
    let incomplete_provenance = forge(
        base,
        incomplete_provenance,
        candidate
            .redundant_block_parameter_witness()
            .unwrap()
            .clone(),
        candidate.predicted_cost_delta(),
    );
    assert_eq!(
        validate_redundant_block_parameter_candidate(&unit, &incomplete_provenance),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
}
