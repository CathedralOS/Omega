use super::super::super::{AbstractSuccessor, ValueBinding};
use super::super::{id, with_synthetic_accepted_obligations};
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
    OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Clone, Copy)]
pub(crate) enum PhiTranslatedRightArm {
    Matching,
    Missing,
    MismatchedType,
}

pub(crate) fn phi_translated_gvn_unit() -> PsiOptimizationUnit {
    phi_translated_gvn_fixture(PhiTranslatedRightArm::Matching, false, false)
}

pub(crate) fn proof_certified_phi_translated_gvn_unit() -> PsiOptimizationUnit {
    phi_translated_gvn_fixture(PhiTranslatedRightArm::Matching, true, false)
}

pub(crate) fn compatible_policy_phi_translated_gvn_unit() -> PsiOptimizationUnit {
    phi_translated_gvn_fixture(PhiTranslatedRightArm::Matching, false, true)
}

pub(crate) fn phi_translated_gvn_fixture(
    right_arm: PhiTranslatedRightArm,
    proof_certified: bool,
    compatible_policy: bool,
) -> PsiOptimizationUnit {
    let machine = id(1_701, MachineId::new);
    let join = id(1_702, BlockId::new);
    let left_block = id(1_703, BlockId::new);
    let entry = id(1_704, BlockId::new);
    let right_block = id(1_705, BlockId::new);
    let condition = id(1_706, ValueId::new);
    let left_input = id(1_707, ValueId::new);
    let right_input = id(1_708, ValueId::new);
    let join_input = id(1_709, ValueId::new);
    let redundant = id(1_710, ValueId::new);
    let left_leader = id(1_711, ValueId::new);
    let right_leader = id(1_712, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let wide = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let redundant_expression = if proof_certified || compatible_policy {
        AbstractOperation::ExactIntegerAdd {
            psi_operation: id(1_713, OperationId::new),
            obligation: id(1_721, ObligationId::new),
            result: redundant,
            scalar_type: integer,
            left: join_input,
            right: join_input,
        }
    } else {
        AbstractOperation::IntegerBitwiseNot {
            psi_operation: id(1_713, OperationId::new),
            result: redundant,
            scalar_type: integer,
            operand: join_input,
        }
    };
    let left_expression = if proof_certified {
        AbstractOperation::ExactIntegerAdd {
            psi_operation: id(1_715, OperationId::new),
            obligation: id(1_722, ObligationId::new),
            result: left_leader,
            scalar_type: integer,
            left: left_input,
            right: left_input,
        }
    } else if compatible_policy {
        AbstractOperation::WrappingIntegerAdd {
            psi_operation: id(1_715, OperationId::new),
            result: left_leader,
            scalar_type: integer,
            left: left_input,
            right: left_input,
        }
    } else {
        AbstractOperation::IntegerBitwiseNot {
            psi_operation: id(1_715, OperationId::new),
            result: left_leader,
            scalar_type: integer,
            operand: left_input,
        }
    };
    let right_expression = if proof_certified {
        AbstractOperation::ExactIntegerAdd {
            psi_operation: id(1_716, OperationId::new),
            obligation: id(1_723, ObligationId::new),
            result: right_leader,
            scalar_type: integer,
            left: right_input,
            right: right_input,
        }
    } else if compatible_policy {
        match right_arm {
            PhiTranslatedRightArm::Matching => AbstractOperation::SaturatingIntegerAdd {
                psi_operation: id(1_716, OperationId::new),
                result: right_leader,
                scalar_type: integer,
                left: right_input,
                right: right_input,
            },
            PhiTranslatedRightArm::Missing => AbstractOperation::WrappingIntegerSubtract {
                psi_operation: id(1_716, OperationId::new),
                result: right_leader,
                scalar_type: integer,
                left: right_input,
                right: right_input,
            },
            PhiTranslatedRightArm::MismatchedType => AbstractOperation::IntegerWiden {
                psi_operation: id(1_716, OperationId::new),
                result: right_leader,
                source_type: integer,
                target_type: wide,
                operand: right_input,
            },
        }
    } else {
        match right_arm {
            PhiTranslatedRightArm::Matching => AbstractOperation::IntegerBitwiseNot {
                psi_operation: id(1_716, OperationId::new),
                result: right_leader,
                scalar_type: integer,
                operand: right_input,
            },
            PhiTranslatedRightArm::Missing => AbstractOperation::WrappingIntegerAdd {
                psi_operation: id(1_716, OperationId::new),
                result: right_leader,
                scalar_type: integer,
                left: right_input,
                right: right_input,
            },
            PhiTranslatedRightArm::MismatchedType => AbstractOperation::IntegerWiden {
                psi_operation: id(1_716, OperationId::new),
                result: right_leader,
                source_type: integer,
                target_type: wide,
                operand: right_input,
            },
        }
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([47; 32]),
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
                        value: left_input,
                        scalar_type: ScalarType::Integer(integer),
                    },
                    AbstractParameter {
                        value: right_input,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: redundant,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: join,
                        parameters: vec![AbstractParameter {
                            value: join_input,
                            scalar_type: ScalarType::Integer(integer),
                        }],
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 5,
                    },
                ],
                operations: vec![
                    redundant_expression,
                    AbstractOperation::Return {
                        psi_edge: id(1_714, EdgeId::new),
                        result: redundant,
                        value: redundant,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                    left_expression,
                    AbstractOperation::Jump {
                        psi_edge: id(1_720, EdgeId::new),
                        target: join,
                        bindings: vec![ValueBinding {
                            parameter: join_input,
                            argument: left_input,
                            scalar_type: ScalarType::Integer(integer),
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_718, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_719, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    right_expression,
                    AbstractOperation::Jump {
                        psi_edge: id(1_717, EdgeId::new),
                        target: join,
                        bindings: vec![ValueBinding {
                            parameter: join_input,
                            argument: right_input,
                            scalar_type: ScalarType::Integer(integer),
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    if proof_certified || compatible_policy {
        with_synthetic_accepted_obligations(unit)
    } else {
        unit
    }
}
