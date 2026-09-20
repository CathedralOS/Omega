//! Whole-machine reachability pruning and root discovery.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::UnreachablePrivateMachinePruneRule;
use super::super::{
    AnalysisKind, AnalysisProduct, BoundaryMachineId, EdgeId, MachineId, O, OperationId,
    OptimizationUnitValidationError, PlaceId, ProvenanceDisposition, PrunedMachineCustody,
    PsiRewriteCandidate, PsiRewritePatch, RuleAnalysisView, StructuralTypeId, compute_analysis,
    linear_empty_block_unit, recompute_psi_optimization_unit_identity,
    rule_unreachable_private_machine_complement, validate_psi_optimization_unit,
    validate_unreachable_private_machines_candidate,
};

fn stored_dynamic_dispatch(
    owner: MachineId,
    operation: OperationId,
    realization: MachineId,
) -> abstract_operations::AbstractStoredDynamicDispatch {
    let application = terminal_psi::ClosedConformanceApplication {
        owner,
        declaration_identity: "test::CarrierImplementsScanner".into(),
        telescope: Vec::new(),
        subject_identity: Some("test::Carrier".into()),
        trait_identity: "test::Scanner".into(),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: Vec::new(),
        rows: Vec::new(),
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    abstract_operations::AbstractStoredDynamicDispatch {
        stored: abstract_operations::AbstractStoredDynamicDescriptor {
            selection: terminal_psi::TerminalDynamicConformanceSelection {
                owner,
                ordinal: 0,
                source: terminal_psi::StructuralArgument {
                    place: PlaceId::new(9_100).unwrap(),
                    path: Vec::new(),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
                conformance_application_report_fingerprint: application.report_fingerprint,
                conformance_application_commitment: application.commitment,
            },
            descriptor: terminal_psi::TerminalStoredDynamicDescriptor {
                owner,
                ordinal: 5,
                establishment_operation: operation,
                selection_ordinal: 0,
                aggregate_type_identity: "test::Holder".into(),
                field_identity: "handler".into(),
            },
            application,
        },
        dispatch: terminal_psi::TerminalStoredDynamicDispatch {
            owner,
            operation,
            descriptor_ordinal: 5,
            declaring_trait_identity: "test::Scanner".into(),
            public_requirement_identity: "test::Scanner::scan()".into(),
            family_tuple: Vec::new(),
            requirement_identity: "test::Scanner::scan".into(),
            realization_identity: "test::Carrier::scan".into(),
            realization_callable_identity: "test::Carrier::scan::callable".into(),
            realization,
        },
    }
}

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

#[test]
fn private_machine_roots_include_stored_dynamic_dispatch_targets() {
    let mut unit = linear_empty_block_unit();
    let caller = unit.functions[0].machine;
    let stored_target = MachineId::new(99).unwrap();
    let stray = MachineId::new(100).unwrap();
    for machine in [stored_target, stray] {
        let mut private = unit.functions[0].clone();
        private.machine = machine;
        unit.functions.push(private);
    }
    let operation = OperationId::new(9_101).unwrap();
    let mut call = unit.functions[0].blocks[0].nodes[0].clone();
    call.operation = O::CallStoredDynamicScalar {
        psi_operation: operation,
        result: abstract_operations::AbstractResult {
            value: semantic_vocabulary::ValueId::new(9_102).unwrap(),
            scalar_type: semantic_vocabulary::ScalarType::Boolean,
        },
        dynamic_dispatch: stored_dynamic_dispatch(caller, operation, stored_target),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    call.definitions.clear();
    call.uses.clear();
    call.successors.clear();
    unit.functions[0].blocks[0].nodes.insert(0, call);
    unit.identity = recompute_psi_optimization_unit_identity(&unit);

    let analysis = compute_analysis(&unit, AnalysisKind::CallGraph).unwrap();
    let AnalysisProduct::CallGraph(call_graph) = analysis else {
        unreachable!("requested call graph analysis")
    };
    assert_eq!(
        rule_unreachable_private_machine_complement(&unit, &call_graph),
        [stray]
    );
}
