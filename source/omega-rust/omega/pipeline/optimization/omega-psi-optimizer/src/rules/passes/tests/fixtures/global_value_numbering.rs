//! GVN fixture programs.

use super::*;

#[path = "global_value_numbering/identities.rs"]
mod identities;
pub(crate) use identities::*;

pub(crate) fn local_cse_unit() -> PsiOptimizationUnit {
    scalar_local_cse_unit(false)
}

pub(crate) fn proof_certified_local_cse_unit() -> PsiOptimizationUnit {
    scalar_local_cse_unit(true)
}

pub(crate) fn compatible_policy_local_cse_unit() -> PsiOptimizationUnit {
    let mut unit = proof_certified_local_cse_unit();
    let node = &mut unit.functions[0].blocks[0].nodes[0];
    let O::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = node.operation
    else {
        unreachable!("proof CSE leader is exact add")
    };
    node.operation = O::WrappingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    };
    unit.functions[0].facts.retain(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == psi_operation)
    });
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn scalar_local_cse_unit(proof_certified: bool) -> PsiOptimizationUnit {
    let machine = id(1_301, MachineId::new);
    let block = id(1_302, BlockId::new);
    let left = id(1_303, ValueId::new);
    let right = id(1_304, ValueId::new);
    let leader = id(1_305, ValueId::new);
    let redundant = id(1_306, ValueId::new);
    let equal = id(1_307, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let binary = |psi_operation, obligation, result, left, right| {
        if proof_certified {
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        } else {
            AbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        }
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([41; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![
                    AbstractParameter {
                        value: left,
                        scalar_type: ScalarType::Integer(integer),
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: equal,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    binary(
                        id(1_308, OperationId::new),
                        id(1_312, ObligationId::new),
                        leader,
                        left,
                        right,
                    ),
                    binary(
                        id(1_309, OperationId::new),
                        id(1_313, ObligationId::new),
                        redundant,
                        right,
                        left,
                    ),
                    AbstractOperation::IntegerEqual {
                        psi_operation: id(1_310, OperationId::new),
                        result: equal,
                        left: leader,
                        right: redundant,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_311, EdgeId::new),
                        result: equal,
                        value: equal,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    if proof_certified {
        with_synthetic_accepted_obligations(unit)
    } else {
        unit
    }
}

pub(crate) fn dominator_gvn_unit() -> PsiOptimizationUnit {
    scalar_dominator_gvn_unit(false)
}

pub(crate) fn proof_certified_dominator_gvn_unit() -> PsiOptimizationUnit {
    scalar_dominator_gvn_unit(true)
}

pub(crate) fn compatible_policy_dominator_gvn_unit() -> PsiOptimizationUnit {
    let mut unit = proof_certified_dominator_gvn_unit();
    let node = &mut unit.functions[0].blocks[1].nodes[0];
    let O::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = node.operation
    else {
        unreachable!("proof GVN leader is exact add")
    };
    node.operation = O::SaturatingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    };
    unit.functions[0].facts.retain(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == psi_operation)
    });
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn scalar_dominator_gvn_unit(proof_certified: bool) -> PsiOptimizationUnit {
    let machine = id(1_341, MachineId::new);
    let dominated = id(1_342, BlockId::new);
    let entry = id(1_343, BlockId::new);
    let left = id(1_344, ValueId::new);
    let right = id(1_345, ValueId::new);
    let leader = id(1_346, ValueId::new);
    let redundant = id(1_347, ValueId::new);
    let equal = id(1_348, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let binary = |psi_operation, obligation, result, left, right| {
        if proof_certified {
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        } else {
            AbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        }
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([42; 32]),
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
                        value: left,
                        scalar_type: ScalarType::Integer(integer),
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: equal,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: dominated,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                operations: vec![
                    binary(
                        id(1_351, OperationId::new),
                        id(1_354, ObligationId::new),
                        redundant,
                        right,
                        left,
                    ),
                    AbstractOperation::IntegerEqual {
                        psi_operation: id(1_352, OperationId::new),
                        result: equal,
                        left: leader,
                        right: redundant,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_353, EdgeId::new),
                        result: equal,
                        value: equal,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                    binary(
                        id(1_349, OperationId::new),
                        id(1_355, ObligationId::new),
                        leader,
                        left,
                        right,
                    ),
                    AbstractOperation::Jump {
                        psi_edge: id(1_350, EdgeId::new),
                        target: dominated,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    if proof_certified {
        with_synthetic_accepted_obligations(unit)
    } else {
        unit
    }
}

pub(crate) fn diamond_dominator_gvn_unit() -> PsiOptimizationUnit {
    let machine = id(1_401, MachineId::new);
    let join = id(1_402, BlockId::new);
    let left_block = id(1_403, BlockId::new);
    let entry = id(1_404, BlockId::new);
    let right_block = id(1_405, BlockId::new);
    let condition = id(1_406, ValueId::new);
    let operand = id(1_407, ValueId::new);
    let outer_first = id(1_408, ValueId::new);
    let outer_second = id(1_409, ValueId::new);
    let inner_first = id(1_410, ValueId::new);
    let inner_second = id(1_411, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
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
                        value: operand,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: inner_second,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: join,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 7,
                    },
                ],
                operations: vec![
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_412, OperationId::new),
                        result: inner_first,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_413, OperationId::new),
                        result: inner_second,
                        scalar_type: integer,
                        operand: inner_first,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_414, EdgeId::new),
                        result: inner_second,
                        value: inner_second,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_415, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_416, OperationId::new),
                        result: outer_first,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_417, OperationId::new),
                        result: outer_second,
                        scalar_type: integer,
                        operand: outer_first,
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_418, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_419, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_420, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn sibling_only_gvn_unit() -> PsiOptimizationUnit {
    let machine = id(1_441, MachineId::new);
    let join = id(1_442, BlockId::new);
    let left_block = id(1_443, BlockId::new);
    let entry = id(1_444, BlockId::new);
    let right_block = id(1_445, BlockId::new);
    let condition = id(1_446, ValueId::new);
    let operand = id(1_447, ValueId::new);
    let sibling = id(1_448, ValueId::new);
    let redundant = id(1_449, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
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
                        value: operand,
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
                        parameters: Vec::new(),
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
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_450, OperationId::new),
                        result: redundant,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_451, EdgeId::new),
                        result: redundant,
                        value: redundant,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_452, OperationId::new),
                        result: sibling,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_453, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_454, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_455, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_456, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

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
