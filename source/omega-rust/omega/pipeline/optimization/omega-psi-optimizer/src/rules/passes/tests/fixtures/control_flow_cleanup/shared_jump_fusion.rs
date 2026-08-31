use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractSuccessor,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{BlockId, EdgeId, FuelScheduleIdentity, MachineId, ScalarType, ValueId};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::super::id;

pub(crate) fn shared_terminal_unit() -> PsiOptimizationUnit {
    let machine = id(921, MachineId::new);
    let entry = id(922, BlockId::new);
    let left_block = id(923, BlockId::new);
    let right_block = id(924, BlockId::new);
    let target = id(926, BlockId::new);
    let condition = id(927, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([38; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: target,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(931, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(932, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(933, EdgeId::new),
                        target,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(934, EdgeId::new),
                        target,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(936, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
