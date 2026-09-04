use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
    OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::super::id;

pub(crate) fn redundant_block_parameter_unit(redundant: bool) -> PsiOptimizationUnit {
    let machine = id(701, MachineId::new);
    let entry = id(702, BlockId::new);
    let merge = id(703, BlockId::new);
    let condition = id(704, ValueId::new);
    let shared = id(705, ValueId::new);
    let alternate = id(706, ValueId::new);
    let parameter = id(707, ValueId::new);
    let result = id(708, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let binding = |argument| ValueBinding {
        parameter,
        argument,
        scalar_type,
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([22; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![
                    AbstractParameter {
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    },
                    AbstractParameter {
                        value: shared,
                        scalar_type,
                    },
                    AbstractParameter {
                        value: alternate,
                        scalar_type,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: merge,
                        parameters: vec![AbstractParameter {
                            value: parameter,
                            scalar_type,
                        }],
                        operation_offset: 1,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(709, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(shared)],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(710, EdgeId::new),
                            target: merge,
                            bindings: vec![binding(if redundant { shared } else { alternate })],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::ExactIntegerAdd {
                        psi_operation: id(711, OperationId::new),
                        obligation: id(713, ObligationId::new),
                        result,
                        scalar_type: integer,
                        left: parameter,
                        right: alternate,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(712, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
