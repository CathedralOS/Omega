use super::*;

pub(super) fn scalar_terminal_artifact(
    result_type: ScalarType,
    parameter_types: Vec<ScalarType>,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(30_001).unwrap();
    let entry = BlockId::new(30_002).unwrap();
    let function_result = ValueId::new(30_004).unwrap();
    let edge = EdgeId::new(30_006).unwrap();
    let parameters = parameter_types
        .into_iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(30_100 + index as u64).unwrap(),
            scalar_type,
        })
        .collect::<Vec<_>>();
    let terminator = Terminator::Return {
        edge,
        value: parameters
            .last()
            .expect("parameter fixture must be nonempty")
            .id,
        cleanup_actions: Vec::new(),
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_range_invariants: Vec::new(),
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
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            parameters,
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
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
                structural_parameters: Vec::new(),
                id: entry,
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator,
            }],
            contract: MachineContract {
                id: ContractId::new(30_007).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}

pub(super) fn parameter_types(scalar_type: ScalarType, count: usize) -> Vec<ScalarType> {
    assert!(count > 0, "parameter fixture must be nonempty");
    vec![scalar_type; count]
}
