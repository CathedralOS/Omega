//! Real Terminal fixture for constant integer equality materialization custody.

use super::*;

pub(crate) fn integer_equal_immediate_return_artifact(
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(70_001).unwrap();
    let entry = BlockId::new(70_002).unwrap();
    let left = ValueId::new(70_004).unwrap();
    let right = ValueId::new(70_006).unwrap();
    let equal = ValueId::new(70_008).unwrap();
    let module = TerminalModule {
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
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(70_010).unwrap(),
                scalar_type: ScalarType::Boolean,
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
                operations: vec![
                    Operation {
                        id: OperationId::new(70_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: left,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: left_value },
                    },
                    Operation {
                        id: OperationId::new(70_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: right,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: right_value },
                    },
                    Operation {
                        id: OperationId::new(70_007).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: equal,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: OperationKind::IntegerEqual { left, right },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(70_009).unwrap(),
                    value: equal,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(70_011).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    (
        psi_terminal_codec::encode_module(&module).unwrap(),
        psi_terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}
