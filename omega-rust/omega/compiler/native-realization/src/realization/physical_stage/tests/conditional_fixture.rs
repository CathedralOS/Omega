//! A separately authored Terminal product with direct scalar-return arms.
//! This does not claim the source frontend emits this form: its current scalar
//! conditional lowering uses a common return block and edge arguments.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Comparison {
    Equal,
    LessThan,
    LessOrEqual,
}

pub(super) fn artifact(comparison: Comparison, sign: IntegerSign) -> (Vec<u8>, Vec<u8>) {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let parameter = ScalarType::Integer(IntegerType::new(sign, 64).unwrap());
    let value = |id, scalar_type| ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type,
    };
    let successor = |id, block| SuccessorEdge {
        edge: EdgeId::new(id).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let leaf = |block, operation, result, edge, literal| Block {
        id: BlockId::new(block).unwrap(),
        parameters: Vec::new(),
        operations: vec![Operation {
            id: OperationId::new(operation).unwrap(),
            result: OperationResult::Scalar(value(result, integer)),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(literal),
            },
        }],
        terminator: Terminator::Return {
            edge: EdgeId::new(edge).unwrap(),
            value: ValueId::new(result).unwrap(),
            cleanup_actions: Vec::new(),
        },
    };
    let machine = TerminalMachine {
        id: MachineId::new(1).unwrap(),
        attachment: None,
        parameters: vec![value(1, parameter), value(2, parameter)],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(6, integer)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![
            Block {
                id: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(1).unwrap(),
                    result: OperationResult::Scalar(value(3, ScalarType::Boolean)),
                    kind: match comparison {
                        Comparison::Equal => OperationKind::IntegerEqual {
                            left: ValueId::new(1).unwrap(),
                            right: ValueId::new(2).unwrap(),
                        },
                        Comparison::LessThan => OperationKind::IntegerLessThan {
                            left: ValueId::new(1).unwrap(),
                            right: ValueId::new(2).unwrap(),
                        },
                        Comparison::LessOrEqual => OperationKind::IntegerLessOrEqual {
                            left: ValueId::new(1).unwrap(),
                            right: ValueId::new(2).unwrap(),
                        },
                    },
                }],
                terminator: Terminator::Conditional {
                    condition: ValueId::new(3).unwrap(),
                    when_true: successor(1, 2),
                    when_false: successor(2, 3),
                },
            },
            leaf(2, 2, 4, 3, 0x1122_3344_5566_7788),
            leaf(3, 3, 5, 4, 0),
        ],
        contract: MachineContract {
            id: ContractId::new(1).unwrap(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
            crash_routes: Vec::new(),
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    };
    (
        terminal_codec::encode_module(&module).unwrap(),
        terminal_codec::encode_proof_bundle(&ProofBundle::default()).unwrap(),
    )
}
