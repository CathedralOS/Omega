//! Real Terminal fixture for constant integer bitwise-AND materialization custody.

use super::super::*;

pub(crate) fn integer_bitwise_and_immediate_return_artifact(
    scalar_type: IntegerType,
    left_value: IntegerValue,
    right_value: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(73_001).unwrap();
    let entry = BlockId::new(73_002).unwrap();
    let left = ValueId::new(73_004).unwrap();
    let right = ValueId::new(73_006).unwrap();
    let bitwise_and = ValueId::new(73_008).unwrap();
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: ValueId::new(73_010).unwrap(),
                scalar_type: ScalarType::Integer(scalar_type),
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
                        id: OperationId::new(73_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: left,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: left_value },
                    },
                    Operation {
                        id: OperationId::new(73_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: right,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: right_value },
                    },
                    Operation {
                        id: OperationId::new(73_007).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: bitwise_and,
                            scalar_type: ScalarType::Integer(scalar_type),
                        }),
                        kind: OperationKind::IntegerBitwiseAnd { left, right },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(73_009).unwrap(),
                    value: bitwise_and,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(73_011).unwrap(),
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
