//! Adjacent and non-adjacent block-merge behavior.

use super::*;

#[test]
fn adjacent_block_merge_substitutes_parameters_and_rehomes_edge_custody() {
    let unit = propagated_block_parameter_unit(true);
    let fold_contract = ConstantConditionalFoldRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, fold_contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let fold = ConstantConditionalFoldRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let folded = validate_constant_conditional_candidate(&unit, &fold)
        .unwrap()
        .into_unit();

    let contract = AdjacentBlockMergeRule::contract();
    let mut manager = crate::AnalysisManager::new(&folded);
    let products = manager
        .require_all(&folded, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidates = AdjacentBlockMergeRule
        .propose(&folded, RuleAnalysisView::new(&products))
        .unwrap();
    let candidate = candidates
        .iter()
        .find(|candidate| {
            matches!(
                candidate.patch(),
                PsiRewritePatch::MergeAdjacentBlock(patch)
                    if patch.predecessor.block == id(603, BlockId::new)
                        && patch.target == id(605, BlockId::new)
            )
        })
        .expect("selected arm can merge with its unique adjacent target");
    assert_eq!(candidate.substitutions().len(), 1);
    let accepted = validate_adjacent_block_merge_candidate(&folded, candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(output.functions[0].blocks.len(), 2);
    let merged = &output.functions[0].blocks[1];
    assert_eq!(merged.nodes.len(), 3);
    assert!(matches!(
        merged.nodes[1].operation,
        AbstractOperation::IntegerBitwiseNot { operand, .. }
            if operand == id(607, ValueId::new)
    ));
    assert_eq!(
        merged.nodes[1].provenance,
        [
            PsiProvenance::Operation(id(618, OperationId::new)),
            PsiProvenance::Edge(id(615, EdgeId::new)),
        ]
    );

    let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
        unreachable!()
    };
    let mut corrupted_provenance = candidate.provenance().to_vec();
    let incoming = PsiRealizationSite::Edge {
        machine: patch.predecessor.machine,
        edge: patch.incoming_edge,
    };
    let row = corrupted_provenance
        .iter_mut()
        .find(|row| row.input == incoming)
        .unwrap();
    row.disposition = ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
        machine: patch.predecessor.machine,
        block: patch.target,
        node: 0,
    }));
    corrupted_provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let corrupted = PsiRewriteCandidate::new_adjacent_block_merge(
        folded.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.substitutions().to_vec(),
        corrupted_provenance,
        candidate.ownership_frontier_witness().unwrap().clone(),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_adjacent_block_merge_candidate(&folded, &corrupted),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
}

#[test]
fn adjacent_block_merge_fuses_a_direct_terminal_exit_without_erasing_it() {
    let unit = linear_empty_block_unit();
    let contract = AdjacentBlockMergeRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = AdjacentBlockMergeRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("the adjacent return target is the sole eligible merge");
    assert!(candidate.consumed_facts().is_empty());
    let accepted = validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(output.functions[0].blocks.len(), 2);
    let terminal = &output.functions[0].blocks[1].nodes[0];
    assert!(matches!(terminal.operation, O::ReturnUnit { .. }));
    assert_eq!(
        terminal.provenance,
        [
            PsiProvenance::Edge(id(913, EdgeId::new)),
            PsiProvenance::Edge(id(912, EdgeId::new)),
        ]
    );
    let incoming = PsiRealizationSite::Edge {
        machine: id(901, MachineId::new),
        edge: id(912, EdgeId::new),
    };
    assert!(accepted.provenance().iter().any(|row| {
        row.input == incoming
            && row.disposition
                == ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
                    machine: id(901, MachineId::new),
                    block: id(903, BlockId::new),
                    node: 0,
                }))
    }));
}

#[test]
fn adjacent_block_merge_carries_exact_ownership_frontier_custody() {
    let mut unit = linear_empty_block_unit();
    let machine = id(901, MachineId::new);
    let incoming = id(912, EdgeId::new);
    let target = id(904, BlockId::new);
    let snapshot = OwnershipFrontierSnapshot {
        claims: Vec::new(),
        owned_places: Vec::new(),
        partial_custody: Vec::new(),
    };
    unit.ownership_frontier_facts = [
        OwnershipFrontierSite::EdgeEntry(id(911, EdgeId::new)),
        OwnershipFrontierSite::EdgeExit(id(911, EdgeId::new)),
        OwnershipFrontierSite::EdgeEntry(incoming),
        OwnershipFrontierSite::EdgeExit(incoming),
        OwnershipFrontierSite::BlockEntry(target),
    ]
    .into_iter()
    .map(|site| OwnershipFrontierFact::new(unit.psi, machine, site, snapshot.clone()))
    .collect();
    unit.ownership_frontier_facts
        .sort_by_key(|fact| (fact.machine, fact.site));
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();

    let contract = AdjacentBlockMergeRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = AdjacentBlockMergeRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .into_iter()
        .find(|candidate| {
            matches!(
                candidate.patch(),
                PsiRewritePatch::MergeAdjacentBlock(patch)
                    if patch.incoming_edge == incoming && patch.target == target
            )
        })
        .expect("ownership-certified adjacent merge is proposed");
    assert_eq!(candidate.consumed_facts().len(), 3);
    assert!(
        candidate
            .consumed_facts()
            .iter()
            .all(|fact| matches!(fact, OptimizationFactReference::OwnershipFrontier(_)))
    );
    validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();

    let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
        unreachable!()
    };
    let missing_custody = PsiRewriteCandidate::new_adjacent_block_merge(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.substitutions().to_vec(),
        candidate.provenance().to_vec(),
        OwnershipFrontierWitness { rows: Vec::new() },
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_adjacent_block_merge_candidate(&unit, &missing_custody),
        Err(OptimizationUnitValidationError::CandidateObservationMismatch)
    );

    let mut forged_witness = candidate.ownership_frontier_witness().unwrap().clone();
    forged_witness.rows[0].fact =
        omega_optimization_core::OwnershipFrontierFactIdentity::from_canonical_bytes(
            b"forged-adjacent-merge-ownership-fact",
        );
    let forged_custody = PsiRewriteCandidate::new_adjacent_block_merge(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.substitutions().to_vec(),
        candidate.provenance().to_vec(),
        forged_witness,
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_adjacent_block_merge_candidate(&unit, &forged_custody),
        Err(OptimizationUnitValidationError::CandidateObservationMismatch)
    );

    let mut reordered_witness = candidate.ownership_frontier_witness().unwrap().clone();
    reordered_witness.rows.reverse();
    assert_eq!(
        PsiRewriteCandidate::new_adjacent_block_merge(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            candidate.provenance().to_vec(),
            reordered_witness,
            candidate.predicted_cost_delta(),
            patch,
        ),
        Err(PsiRewriteCandidateError::NonCanonicalOwnershipFrontierWitness)
    );
}

#[test]
fn adjacent_conditional_merge_fans_incoming_custody_to_exact_arms() {
    let unit = adjacent_conditional_merge_unit();
    let contract = AdjacentBlockMergeRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let [candidate] = AdjacentBlockMergeRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .try_into()
        .expect("only the adjacent conditional target is eligible");
    let accepted = validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();
    let output = accepted.unit();
    assert_eq!(output.functions[0].blocks.len(), 3);
    let node = &output.functions[0].blocks[0].nodes[0];
    assert!(matches!(
        node.operation,
        AbstractOperation::Conditional { condition, .. }
            if condition == id(1_106, ValueId::new)
    ));
    for (edge, direct) in [
        (&node.successors[0], id(1_111, EdgeId::new)),
        (&node.successors[1], id(1_112, EdgeId::new)),
    ] {
        assert_eq!(
            edge.provenance,
            [
                PsiProvenance::Edge(direct),
                PsiProvenance::Edge(id(1_110, EdgeId::new)),
            ]
        );
    }
    let incoming = PsiRealizationSite::Edge {
        machine: id(1_101, MachineId::new),
        edge: id(1_110, EdgeId::new),
    };
    assert_eq!(
        accepted
            .provenance()
            .iter()
            .filter(|row| row.input == incoming)
            .count(),
        2
    );

    let PsiRewritePatch::MergeAdjacentBlock(patch) = candidate.patch() else {
        unreachable!()
    };
    let mut corrupted_provenance = candidate.provenance().to_vec();
    corrupted_provenance
        .iter_mut()
        .find(|row| {
            row.input == incoming
                && row.disposition.site()
                    == (PsiRealizationSite::Edge {
                        machine: id(1_101, MachineId::new),
                        edge: id(1_112, EdgeId::new),
                    })
        })
        .unwrap()
        .disposition = ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
        machine: id(1_101, MachineId::new),
        block: id(1_103, BlockId::new),
        node: 0,
    }));
    corrupted_provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    let corrupted = PsiRewriteCandidate::new_adjacent_block_merge(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        candidate.substitutions().to_vec(),
        corrupted_provenance,
        candidate.ownership_frontier_witness().unwrap().clone(),
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_adjacent_block_merge_candidate(&unit, &corrupted),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
}

#[test]
fn non_adjacent_merge_supports_both_roster_directions_and_global_uses() {
    for target_before_predecessor in [false, true] {
        let unit = non_adjacent_merge_unit(target_before_predecessor);
        validate_psi_optimization_unit(&unit).unwrap();
        let contract = NonAdjacentBlockMergeRule::contract();
        let mut manager = crate::AnalysisManager::new(&unit);
        let products = manager
            .require_all(&unit, contract.required_analyses())
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidates = NonAdjacentBlockMergeRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap();
        let candidate = candidates
            .iter()
            .find(|candidate| {
                matches!(
                    candidate.patch(),
                    PsiRewritePatch::MergeNonAdjacentBlock(patch)
                        if patch.target == id(1_504, BlockId::new)
                )
            })
            .expect("predecessor-to-target merge is proposed in either roster direction");
        assert_eq!(
            candidate.affected_blocks(),
            [
                id(1_503, BlockId::new),
                id(1_504, BlockId::new),
                id(1_505, BlockId::new),
                id(1_506, BlockId::new),
            ]
        );
        assert!(
            AdjacentBlockMergeRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
                .iter()
                .all(|row| !matches!(
                    row.patch(),
                    PsiRewritePatch::MergeAdjacentBlock(patch)
                        if patch.target == id(1_504, BlockId::new)
                ))
        );

        let accepted = validate_non_adjacent_block_merge_candidate(&unit, candidate).unwrap();
        let output = accepted.unit();
        assert_eq!(output.functions[0].blocks.len(), 4);
        assert!(
            output.functions[0]
                .blocks
                .iter()
                .all(|block| block.id != id(1_504, BlockId::new))
        );
        let predecessor = output.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == id(1_506, BlockId::new))
            .unwrap();
        assert_eq!(predecessor.nodes.len(), 3);
        assert!(matches!(
            predecessor.nodes[1].operation,
            O::BooleanNot {
                operand,
                result,
                ..
            } if operand == id(1_520, ValueId::new)
                && result == id(1_510, ValueId::new)
        ));
        assert_eq!(
            predecessor.nodes[1].definitions[0].site,
            omega_optimization_unit::ValueDefinitionSite::Node {
                block: id(1_506, BlockId::new),
                node: 1,
            }
        );
        let descendant = output.functions[0]
            .blocks
            .iter()
            .find(|block| block.id == id(1_503, BlockId::new))
            .unwrap();
        assert!(matches!(
            descendant.nodes[0].operation,
            O::BooleanEqual { left, right, .. }
                if left == id(1_520, ValueId::new)
                    && right == id(1_510, ValueId::new)
        ));
        let incoming = PsiRealizationSite::Edge {
            machine: id(1_501, MachineId::new),
            edge: id(1_519, EdgeId::new),
        };
        assert!(accepted.provenance().iter().any(|row| {
            row.input == incoming
                && row.disposition
                    == ProvenanceDisposition::RealizedAt(PsiRealizationSite::Node(NodeLocation {
                        machine: id(1_501, MachineId::new),
                        block: id(1_506, BlockId::new),
                        node: 1,
                    }))
        }));

        let PsiRewritePatch::MergeNonAdjacentBlock(patch) = candidate.patch() else {
            unreachable!()
        };
        let mut incomplete = candidate.provenance().to_vec();
        let omitted = incomplete
            .iter()
            .position(|row| row.input != incoming)
            .expect("fixture has non-incoming custody");
        incomplete.remove(omitted);
        let corrupted = PsiRewriteCandidate::new_non_adjacent_block_merge(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            candidate.substitutions().to_vec(),
            incomplete,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert_eq!(
            validate_non_adjacent_block_merge_candidate(&unit, &corrupted),
            Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
        );
    }
}

#[test]
fn adjacent_merge_rewrites_target_parameter_uses_in_dominated_successors() {
    let mut unit = non_adjacent_merge_unit(false);
    let sibling = unit.functions[0].blocks.remove(2);
    unit.functions[0].blocks.insert(3, sibling);
    let mut effect = 0u64;
    for block in &mut unit.functions[0].blocks {
        for node in &mut block.nodes {
            node.effect = omega_optimization_unit::EffectLink {
                input: effect,
                output: effect + 1,
            };
            effect += 1;
        }
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();

    let contract = AdjacentBlockMergeRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = AdjacentBlockMergeRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .into_iter()
        .find(|candidate| {
            matches!(
                candidate.patch(),
                PsiRewritePatch::MergeAdjacentBlock(patch)
                    if patch.target == id(1_504, BlockId::new)
            )
        })
        .expect("forward-adjacent parameterized target is merged");
    let accepted = validate_adjacent_block_merge_candidate(&unit, &candidate).unwrap();
    let descendant = accepted.unit().functions[0]
        .blocks
        .iter()
        .find(|block| block.id == id(1_503, BlockId::new))
        .unwrap();
    assert!(matches!(
        descendant.nodes[0].operation,
        O::BooleanEqual { left, right, .. }
            if left == id(1_520, ValueId::new)
                && right == id(1_510, ValueId::new)
    ));
}
