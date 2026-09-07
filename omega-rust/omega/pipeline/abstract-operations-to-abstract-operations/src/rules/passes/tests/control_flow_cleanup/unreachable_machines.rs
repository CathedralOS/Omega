//! Whole-machine reachability pruning and root discovery.

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
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    unit.functions[2].attachment = Some(StructuralTypeId::new(9_002).unwrap());
    unit.provider_candidates
        .push(terminal_psi::ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(9_006).unwrap(),
            requirement_identity: "root-test-requirement".into(),
            provider_identity: "root-test-provider".into(),
            candidate_identity: "root-test-candidate".into(),
            candidate: MachineId::new(102).unwrap(),
            signature: terminal_psi::ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: terminal_psi::ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    unit.functions[1].blocks[0].nodes[0].operation = O::ReturnUnit {
        psi_edge: EdgeId::new(9_003).unwrap(),
        cleanup_actions: vec![terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
            terminal_psi::NominalAffineCleanup {
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
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    unit.functions[6].blocks[0].nodes[0].operation = O::CallUnit {
        psi_operation: OperationId::new(9_008).unwrap(),
        callee: MachineId::new(103).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
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
