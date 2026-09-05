//! Real Terminal fixture for constant wrapping integer shift-left materialization custody.

use super::super::*;

pub(crate) fn wrapping_integer_shift_left_immediate_return_artifact(
    value_type: IntegerType,
    count_type: IntegerType,
    value: IntegerValue,
    count: IntegerValue,
) -> (Vec<u8>, Vec<u8>) {
    let machine = MachineId::new(83_001).unwrap();
    let entry = BlockId::new(83_002).unwrap();
    let value_result = ValueId::new(83_004).unwrap();
    let count_result = ValueId::new(83_006).unwrap();
    let shift_result = ValueId::new(83_008).unwrap();
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
                id: ValueId::new(83_010).unwrap(),
                scalar_type: ScalarType::Integer(value_type),
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
                        id: OperationId::new(83_003).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_result,
                            scalar_type: ScalarType::Integer(value_type),
                        }),
                        kind: OperationKind::IntegerConstant { value },
                    },
                    Operation {
                        id: OperationId::new(83_005).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: count_result,
                            scalar_type: ScalarType::Integer(count_type),
                        }),
                        kind: OperationKind::IntegerConstant { value: count },
                    },
                    Operation {
                        id: OperationId::new(83_007).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: shift_result,
                            scalar_type: ScalarType::Integer(value_type),
                        }),
                        kind: OperationKind::WrappingIntegerShiftLeft {
                            value: value_result,
                            count: count_result,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    edge: EdgeId::new(83_009).unwrap(),
                    value: shift_result,
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(83_011).unwrap(),
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
