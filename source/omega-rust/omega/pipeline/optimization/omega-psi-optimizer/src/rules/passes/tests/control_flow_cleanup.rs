//! Control-flow cleanup tests.

use super::*;

#[test]
fn unreachable_private_machine_pruning_is_atomic_canonical_and_idempotent() {
    let mut unit = linear_empty_block_unit();
    let mut private = unit.functions[0].clone();
    private.machine = MachineId::new(99).unwrap();
    unit.functions.push(private);
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();

    let call_graph = compute_analysis(&unit, AnalysisKind::CallGraph).unwrap();
    let candidates = UnreachablePrivateMachinePruneRule
        .propose(&unit, RuleAnalysisView::new(&[call_graph]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].affected_machines(),
        [MachineId::new(99).unwrap()]
    );
    assert!(candidates[0].provenance().iter().all(|row| {
        row.input.machine() == MachineId::new(99).unwrap()
            && row.disposition == ProvenanceDisposition::ProvenUnreachableAt(row.input)
    }));

    let accepted = validate_unreachable_private_machines_candidate(&unit, &candidates[0]).unwrap();
    assert_eq!(accepted.unit().functions.len(), 1);
    assert_eq!(
        accepted.unit().pruned_machines,
        [PrunedMachineCustody {
            machine: MachineId::new(99).unwrap(),
            source_ordinal: 1,
        }]
    );
    assert_eq!(
        accepted.unit().accepted_obligation_facts,
        unit.accepted_obligation_facts
    );
    assert_eq!(
        accepted.unit().ownership_frontier_facts,
        unit.ownership_frontier_facts
    );

    let call_graph = compute_analysis(accepted.unit(), AnalysisKind::CallGraph).unwrap();
    assert!(
        UnreachablePrivateMachinePruneRule
            .propose(accepted.unit(), RuleAnalysisView::new(&[call_graph]))
            .unwrap()
            .is_empty()
    );

    let PsiRewritePatch::PruneUnreachablePrivateMachines(patch) = candidates[0].patch() else {
        unreachable!("pruning rule emits its typed patch")
    };
    let mut incomplete = candidates[0].provenance().to_vec();
    incomplete.pop();
    let forged = PsiRewriteCandidate::new_unreachable_private_machines(
        unit.identity,
        UnreachablePrivateMachinePruneRule::contract(),
        incomplete,
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(
        validate_unreachable_private_machines_candidate(&unit, &forged),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    );
}

#[test]
fn private_machine_roots_include_calls_attachments_cleanup_and_prune_recursive_islands() {
    let mut unit = linear_empty_block_unit();
    let template = unit.functions[0].clone();
    for machine in [99, 100, 101, 102, 103, 104] {
        let mut function = template.clone();
        function.machine = MachineId::new(machine).unwrap();
        unit.functions.push(function);
    }
    unit.functions[0].blocks[0].nodes[0].operation = O::CallUnit {
        psi_operation: OperationId::new(9_001).unwrap(),
        callee: MachineId::new(99).unwrap(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
    };
    unit.functions[2].attachment = Some(StructuralTypeId::new(9_002).unwrap());
    unit.provider_candidates
        .push(psi_terminal::ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(9_006).unwrap(),
            requirement_identity: "root-test-requirement".into(),
            provider_identity: "root-test-provider".into(),
            candidate_identity: "root-test-candidate".into(),
            candidate: MachineId::new(102).unwrap(),
            signature: psi_terminal::ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: psi_terminal::ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    unit.functions[1].blocks[0].nodes[0].operation = O::ReturnUnit {
        psi_edge: EdgeId::new(9_003).unwrap(),
        cleanup_actions: vec![psi_terminal::TerminalAffineCleanupAction::InvokeNominal(
            psi_terminal::NominalAffineCleanup {
                place: PlaceId::new(9_004).unwrap(),
                structural_type: StructuralTypeId::new(9_005).unwrap(),
                cleanup_machine: MachineId::new(101).unwrap(),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        )],
    };
    unit.functions[5].blocks[0].nodes[0].operation = O::CallUnit {
        psi_operation: OperationId::new(9_007).unwrap(),
        callee: MachineId::new(104).unwrap(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
    };
    unit.functions[6].blocks[0].nodes[0].operation = O::CallUnit {
        psi_operation: OperationId::new(9_008).unwrap(),
        callee: MachineId::new(103).unwrap(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
    };

    let analysis = compute_analysis(&unit, AnalysisKind::CallGraph).unwrap();
    let AnalysisProduct::CallGraph(call_graph) = analysis else {
        unreachable!("requested call graph analysis")
    };
    assert_eq!(
        rule_unreachable_private_machine_complement(&unit, &call_graph),
        [MachineId::new(103).unwrap(), MachineId::new(104).unwrap()]
    );
}

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
        let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
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
            [omega_optimization_unit::PsiProvenance::Edge(
                patch.selected_edge
            )]
        );
        assert_eq!(
            proven_unreachable.disposition,
            ProvenanceDisposition::ProvenUnreachableAt(unreachable_site)
        );
        assert_eq!(
            proven_unreachable.sources,
            [omega_optimization_unit::PsiProvenance::Edge(
                patch.rejected_edge
            )]
        );
        let accepted = validate_constant_conditional_candidate(&unit, &candidates[0]).unwrap();
        assert_eq!(accepted.provenance(), candidates[0].provenance());
        assert_eq!(
            accepted.validator(),
            omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
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
            [omega_optimization_unit::PsiProvenance::Edge(
                patch.selected_edge
            )]
        );
        assert!(node.provenance.is_empty());
        assert!(node.fuel.is_empty());
        assert_eq!(node.successors[0].fuel.len(), 1);
        assert_eq!(
            node.successors[0].fuel[0].site,
            omega_optimization_unit::PsiProvenance::Edge(patch.selected_edge)
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
fn linear_empty_block_thread_composes_bindings_and_realizes_both_edges() {
    let unit = linear_empty_block_unit();
    let contract = LinearEmptyBlockThreadRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = LinearEmptyBlockThreadRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .expect("linear jump block is threadable");
    assert_eq!(
        candidate.affected_blocks(),
        [
            id(902, BlockId::new),
            id(903, BlockId::new),
            id(904, BlockId::new),
        ]
    );
    assert_eq!(candidate.provenance().len(), 3);
    assert!(
        candidate
            .provenance()
            .iter()
            .all(|row| row.disposition.is_realized())
    );
    assert_eq!(
        candidate
            .provenance()
            .iter()
            .filter(|row| {
                matches!(row.input, PsiRealizationSite::Edge { .. })
                    && row.disposition.site()
                        == PsiRealizationSite::Edge {
                            machine: id(901, MachineId::new),
                            edge: id(911, psi_core::EdgeId::new),
                        }
            })
            .count(),
        2
    );

    let accepted = validate_linear_empty_block_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        omega_optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.linear-empty-block-thread.v2"
        )
    );
    let output = accepted.unit();
    assert_eq!(
        output.functions[0]
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        [id(902, BlockId::new), id(904, BlockId::new)]
    );
    let O::Jump {
        psi_edge,
        target,
        bindings,
        ..
    } = &output.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    assert_eq!(*psi_edge, id(911, EdgeId::new));
    assert_eq!(*target, id(904, BlockId::new));
    assert_eq!(bindings[0].argument, id(906, ValueId::new));
    assert_eq!(bindings[1].argument, id(905, ValueId::new));
    assert!(output.functions[0].blocks[0].nodes[0].provenance.is_empty());
    assert!(output.functions[0].blocks[0].nodes[0].fuel.is_empty());
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].successors[0]
            .provenance
            .len(),
        2
    );
    assert_eq!(
        output.functions[0].blocks[0].nodes[0].successors[0]
            .fuel
            .len(),
        2
    );
    assert_eq!(output.functions[0].blocks[1].nodes[0].effect.input, 1);
    assert_eq!(output.functions[0].blocks[1].nodes[0].effect.output, 2);
}

#[test]
fn linear_empty_block_validator_rejects_incomplete_fused_custody() {
    let unit = linear_empty_block_unit();
    let contract = LinearEmptyBlockThreadRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = LinearEmptyBlockThreadRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .unwrap();
    let omega_optimization_unit::PsiRewritePatch::ThreadLinearEmptyBlock(patch) = candidate.patch()
    else {
        unreachable!()
    };
    let mut provenance = candidate.provenance().to_vec();
    let incoming = provenance
        .iter()
        .find(|row| {
            row.input
                == PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.incoming_edge,
                }
        })
        .expect("incoming occurrence is present")
        .clone();
    let outgoing = provenance
        .iter_mut()
        .find(|row| {
            row.input
                == PsiRealizationSite::Edge {
                    machine: patch.predecessor.machine,
                    edge: patch.outgoing_edge,
                }
        })
        .expect("outgoing occurrence is present");
    outgoing.sources = incoming.sources;
    outgoing.fuel = incoming.fuel;
    let incomplete = PsiRewriteCandidate::new_linear_empty_block(
        unit.identity,
        contract,
        candidate.affected_blocks().to_vec(),
        provenance,
        -3,
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_linear_empty_block_candidate(&unit, &incomplete),
        Err(OptimizationUnitValidationError::CandidateProvenanceMismatch)
    ));
}

#[test]
fn path_qualified_empty_block_thread_fans_out_only_on_incoming_edge_antichain() {
    let unit = path_qualified_empty_block_unit();
    let contract = PathQualifiedEmptyBlockThreadRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidate = PathQualifiedEmptyBlockThreadRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap()
        .pop()
        .expect("two mutually exclusive incoming edges are threadable");
    let PsiRewritePatch::ThreadPathQualifiedEmptyBlock(patch) = candidate.patch() else {
        unreachable!()
    };
    let outgoing_site = PsiRealizationSite::Edge {
        machine: patch.empty.machine,
        edge: patch.outgoing_edge,
    };
    let fanout = candidate
        .provenance()
        .iter()
        .filter(|row| row.input == outgoing_site)
        .collect::<Vec<_>>();
    assert_eq!(fanout.len(), 2);
    assert_ne!(fanout[0].disposition.site(), fanout[1].disposition.site());
    assert!(fanout.iter().all(|row| row.disposition.is_realized()));

    let accepted = validate_path_qualified_empty_block_candidate(&unit, &candidate).unwrap();
    assert_eq!(
        accepted.validator(),
        OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.path-qualified-empty-block-thread.v1"
        )
    );
    let function = &accepted.unit().functions[0];
    assert_eq!(function.blocks.len(), 4);
    assert!(
        !function
            .blocks
            .iter()
            .any(|block| block.id == patch.empty.block)
    );
    for edge_id in [id(933, EdgeId::new), id(934, EdgeId::new)] {
        let edge = function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter())
            .flat_map(|node| node.successors.iter())
            .find(|edge| edge.psi_edge == edge_id)
            .expect("incoming edge survives");
        assert_eq!(edge.target, patch.target);
        assert_eq!(
            edge.provenance,
            [
                PsiProvenance::Edge(edge_id),
                PsiProvenance::Edge(patch.outgoing_edge),
            ]
        );
    }

    let mut coexecuted = accepted.unit().clone();
    let source = PsiProvenance::Edge(patch.outgoing_edge);
    coexecuted.functions[0].blocks[0].nodes[0].successors[0]
        .provenance
        .push(source);
    coexecuted.functions[0].blocks[0].nodes[0].successors[0]
        .fuel
        .push(omega_optimization_unit::FuelSettlement {
            site: source,
            units: 1,
        });
    coexecuted.identity = recompute_psi_optimization_unit_identity(&coexecuted);
    assert_eq!(
        omega_optimization_validation::validate_psi_optimization_unit(&coexecuted),
        Err(OptimizationUnitValidationError::CoExecutableProvenanceOccurrences(source))
    );
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
    let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
        candidate.patch()
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
        Err(omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch)
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
        Err(omega_optimization_unit::PsiRewriteCandidateError::NonCanonicalProvenance)
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
        Err(omega_optimization_unit::PsiRewriteCandidateError::FuelProvenanceMismatch)
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
    let omega_optimization_unit::PsiRewritePatch::FoldConstantConditional(patch) =
        candidate.patch()
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
