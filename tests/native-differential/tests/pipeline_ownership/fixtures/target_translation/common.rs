use super::*;

pub(super) fn scalar_terminal_artifact(
    result_type: ScalarType,
    parameter_types: Vec<ScalarType>,
    operation: Option<OperationKind>,
    crash: Option<CrashCause>,
    obligation: Option<ObligationId>,
) -> (Vec<u8>, Vec<u8>) {
    assert!(operation.is_none() || crash.is_none());
    let machine = MachineId::new(30_001).unwrap();
    let entry = BlockId::new(30_002).unwrap();
    let computed = ValueId::new(30_003).unwrap();
    let function_result = ValueId::new(30_004).unwrap();
    let edge = EdgeId::new(30_006).unwrap();
    let parameters = parameter_types
        .into_iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: ValueId::new(30_100 + index as u64).unwrap(),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let returned = operation
        .as_ref()
        .map(|_| computed)
        .or_else(|| parameters.last().map(|parameter| parameter.id));
    let operations = operation
        .map(|kind| Operation {
            id: OperationId::new(30_005).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: computed,
                scalar_type: result_type,
            }),
            kind,
        })
        .into_iter()
        .collect();
    let (terminator, crash_routes) = match (crash, returned) {
        (Some(cause), None) => (
            Terminator::Crash {
                edge,
                cause,
                site_guard: Vec::new(),
                frontier_lower_bound: Vec::new(),
            },
            vec![CrashRouteBucket {
                cause,
                alternatives: vec![CrashRouteGuard::Truth],
            }],
        ),
        (None, Some(value)) => (
            Terminator::Return {
                edge,
                value,
                cleanup_actions: Vec::new(),
            },
            Vec::new(),
        ),
        _ => panic!("scalar fixture must return a value or crash"),
    };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters,
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: function_result,
                scalar_type: result_type,
            }),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                operations,
                terminator,
            }],
            contract: MachineContract {
                id: ContractId::new(30_007).unwrap(),
                crash_routes,
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = obligation.map_or_else(ProofBundle::default, |obligation| {
        let reconstructed = reconstruct_operation_obligations(&module).unwrap();
        assert_eq!(reconstructed.len(), 1);
        let goal = reconstructed[0].obligation.proposition.clone();
        module.machines[0].contract.requires.push(goal.clone());
        ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(30_008).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::Assumption { index: 0 },
                    },
                }),
            }],
        }
    });
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
    )
}

pub(super) fn parameter_types(scalar_type: ScalarType, count: usize) -> Vec<ScalarType> {
    assert!(count > 0, "parameter fixture must be nonempty");
    vec![scalar_type; count]
}

pub(super) fn parameter_value(index: usize) -> ValueId {
    ValueId::new(30_100 + index as u64).unwrap()
}
